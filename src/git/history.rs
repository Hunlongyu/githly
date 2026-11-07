use super::*;
use crate::i18n::messages;
use futures_lite::io::{AsyncBufRead, BufReader};
use std::{
    ops::Index,
    pin::Pin,
    task::{Poll, ready},
};

pub const HISTORY_BATCH: usize = 100;

/// Shared batches: extending history copies a small index, not all old strings
/// and graph edges. The entire session is released when its project is closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct History<T> {
    batches: Vec<Arc<[T]>>,
    len: usize,
}

impl<T> History<T> {
    pub fn new(items: Vec<T>) -> Self {
        let len = items.len();
        Self {
            batches: vec![items.into()],
            len,
        }
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn get(&self, index: usize) -> Option<&T> {
        let first = &self.batches[0];
        if index < first.len() {
            return first.get(index);
        }
        // Only continuation batches have a fixed size.
        let index = index - first.len();
        self.batches
            .get(1 + index / HISTORY_BATCH)?
            .get(index % HISTORY_BATCH)
    }
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.batches.iter().flat_map(|b| b.iter())
    }
    fn append(&mut self, items: Vec<T>) {
        debug_assert_eq!((self.len - self.batches[0].len()) % HISTORY_BATCH, 0);
        self.len += items.len();
        self.batches.push(items.into());
    }
}
impl<T> Index<usize> for History<T> {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        self.get(index).expect("history index")
    }
}

pub struct HistoryStream {
    child: async_process::Child,
    stdout: BufReader<async_process::ChildStdout>,
}

impl HistoryStream {
    pub fn start(git: &Git, repository: &Repository) -> Result<Self> {
        anyhow::ensure!(
            !repository.history_roots.is_empty(),
            messages().no_more_history()
        );
        let mut command = std::process::Command::new(&git.executable);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        command
            .current_dir(&repository.root)
            .args([
                "--no-pager",
                "log",
                "-z",
                "--topo-order",
                "--date=format-local:%Y-%m-%d %H:%M",
                "--format=%H%x00%h%x00%an%x00%ad%x00%s%x00%P%x00%D",
            ])
            .arg(format!("--skip={}", repository.commits.len()))
            .args(repository.history_roots.iter())
            .arg("--")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("LC_ALL", "C");
        let mut command = async_process::Command::from(command);
        // No unread error pipe can block the stream. Nonzero exit is surfaced
        // below; cancellation kills and reaps the owned child.
        let mut child = command
            .kill_on_drop(true)
            .reap_on_drop(true)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        let stdout = BufReader::new(child.stdout.take().context(messages().read_history())?);
        Ok(Self { child, stdout })
    }

    pub async fn extend(&mut self, previous: &Repository) -> Result<Repository> {
        futures_lite::future::race(self.read_batch(previous), async {
            async_io::Timer::after(Duration::from_secs(20)).await;
            bail!(messages().history_timeout())
        })
        .await
    }

    async fn read_batch(&mut self, previous: &Repository) -> Result<Repository> {
        let mut bytes = Vec::new();
        let mut complete = false;
        // Every record has seven NUL-terminated fields. Limit allocation while
        // reading, even if a malicious commit contains a huge subject.
        for field in 0..HISTORY_BATCH * 7 {
            let mut terminated = false;
            loop {
                // Copy within the poll: futures-lite's fill_buf future polls
                // twice, which is not reliable at EOF on Windows async pipes.
                let part: Result<Option<bool>> = futures_lite::future::poll_fn(|cx| {
                    let buffer = ready!(Pin::new(&mut self.stdout).poll_fill_buf(cx))?;
                    if buffer.is_empty() {
                        return Poll::Ready(Ok(None));
                    }
                    let count = buffer
                        .iter()
                        .position(|b| *b == 0)
                        .map_or(buffer.len(), |i| i + 1);
                    if bytes.len() + count > 8 * 1024 * 1024 {
                        return Poll::Ready(Err(anyhow::anyhow!(messages().history_limit())));
                    }
                    let terminated = buffer[count - 1] == 0;
                    bytes.extend_from_slice(&buffer[..count]);
                    Pin::new(&mut self.stdout).consume(count);
                    Poll::Ready(Ok(Some(terminated)))
                })
                .await;
                let Some(part) = part? else { break };
                terminated = part;
                if terminated {
                    break;
                }
            }
            if !terminated {
                let status = self.child.status().await?;
                anyhow::ensure!(status.success(), messages().history_failed(status));
                anyhow::ensure!(field % 7 == 0, messages().incomplete_history());
                complete = true;
                break;
            }
        }
        let commits = parse_log(&bytes);
        let mut lanes = previous.graph_lanes.to_vec();
        let graph = crate::graph::extend(&commits, &mut lanes);
        let mut next = previous.clone();
        next.graph_width = previous
            .graph_width
            .max(graph.iter().map(|row| row.width).max().unwrap_or(1));
        let mut history = (*previous.commits).clone();
        history.append(commits);
        next.commits = Arc::new(history);
        let mut rows = (*previous.graph).clone();
        rows.append(graph);
        next.graph = Arc::new(rows);
        next.graph_lanes = lanes.into();
        next.history_complete = complete;
        Ok(next)
    }
}
