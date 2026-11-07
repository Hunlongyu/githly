use crate::i18n::messages;
use anyhow::{Context, Result, bail};
use futures_lite::io::{AsyncRead, AsyncReadExt};
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

pub const IMAGE_FILE_LIMIT: u64 = 8 * 1024 * 1024;
mod history;
pub use history::{History, HistoryStream};
mod stash;
mod status;
pub use stash::{Stash, StashDetail};
mod statistics;
pub use statistics::LineStatistics;
mod worktrees;
pub use worktrees::Worktree;
#[cfg(test)]
pub(crate) mod history_tests;

#[derive(Clone, Debug)]
pub struct Git {
    pub executable: PathBuf,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    pub original_path: Option<String>,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub id: String,
    pub short_id: String,
    pub author: String,
    pub date: String,
    pub subject: String,
    pub parents: Vec<String>,
    pub references: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repository {
    pub root: PathBuf,
    pub has_other_worktrees: bool,
    pub worktree_error: Option<String>,
    git_dir: PathBuf,
    pub operation_states: Vec<String>,
    pub branch: String,
    pub head: Option<String>,
    pub tracking: Option<BranchTracking>,
    pub changes: Vec<ChangedFile>,
    pub commits: Arc<History<Commit>>,
    pub stashes: Vec<Stash>,
    pub branches: Arc<[Branch]>,
    pub graph: Arc<History<crate::graph::GraphRow>>,
    pub graph_width: usize,
    pub history_scope: HistoryScope,
    history_roots: Arc<[String]>,
    pub history_complete: bool,
    pub history_pending: bool,
    graph_lanes: Arc<[String]>,
    history_key: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    pub remote: bool,
    pub current: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchTracking {
    pub upstream: String,
    /// HEAD-only (ahead) and upstream-only (behind) commits. None means the
    /// configured upstream ref is unavailable locally, rather than zero.
    pub counts: Option<(usize, usize)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum HistoryScope {
    #[default]
    Head,
    Branch(String),
    All,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChangeScope {
    #[default]
    All,
    Staged,
    Working,
}
impl ChangeScope {
    pub fn includes(self, file: &ChangedFile) -> bool {
        match self {
            Self::All => true,
            Self::Staged => {
                file.status != "??" && file.status.as_bytes().first().is_some_and(|b| *b != b' ')
            }
            Self::Working => {
                file.status == "??" || file.status.as_bytes().get(1).is_some_and(|b| *b != b' ')
            }
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::All => messages().head_to_worktree(),
            Self::Staged => messages().head_to_index(),
            Self::Working => messages().index_to_worktree(),
        }
    }
}

impl HistoryScope {
    pub fn label(&self) -> &str {
        match self {
            Self::Head => messages().current_branch(),
            Self::All => messages().all_branches(),
            Self::Branch(name) => name
                .strip_prefix("refs/heads/")
                .or_else(|| name.strip_prefix("refs/remotes/"))
                .unwrap_or(name),
        }
    }
}

impl Git {
    pub async fn detect(configured: Option<PathBuf>) -> Result<Self> {
        let executable = if let Some(path) = configured {
            anyhow::ensure!(
                path.is_absolute() && path.is_file(),
                messages().git_path_missing(path.display())
            );
            path
        } else {
            find_on_path().context(messages().git_not_on_path())?
        };
        let name = executable
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or_default();
        anyhow::ensure!(
            name.eq_ignore_ascii_case(if cfg!(windows) { "git.exe" } else { "git" }),
            messages().select_git_binary()
        );
        let mut git = Self {
            executable,
            version: String::new(),
        };
        let output = git.run(None, ["--version"]).await?;
        let version = String::from_utf8_lossy(&output).trim().to_string();
        anyhow::ensure!(
            version.starts_with("git version "),
            messages().invalid_git_version()
        );
        git.version = version;
        Ok(git)
    }

    async fn run<I, S>(&self, root: Option<&Path>, args: I) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.run_with_stdout(root, args, |stdout| read_limited(stdout, 8 * 1024 * 1024))
            .await
    }

    async fn run_with_stdout<I, S, T, F>(
        &self,
        root: Option<&Path>,
        args: I,
        read_stdout: impl FnOnce(async_process::ChildStdout) -> F,
    ) -> Result<T>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
        F: Future<Output = Result<T>>,
    {
        let mut command = std::process::Command::new(&self.executable);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        command
            .arg("--no-pager")
            .arg("--literal-pathspecs")
            .args(args)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("LC_ALL", "C");
        if let Some(root) = root {
            command.current_dir(root);
        }
        let mut command = async_process::Command::from(command);
        command
            .kill_on_drop(true)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = command.spawn().context(messages().start_git())?;
        let stdout = child.stdout.take().context(messages().read_git_output())?;
        let stderr = child.stderr.take().context(messages().read_git_stderr())?;
        let (status, stdout, stderr) = futures_lite::future::race(
            async {
                let (stdout, stderr) = futures_lite::future::try_zip(
                    read_stdout(stdout),
                    read_limited(stderr, 64 * 1024),
                )
                .await?;
                Ok((child.status().await?, stdout, stderr))
            },
            async {
                async_io::Timer::after(Duration::from_secs(20)).await;
                bail!(messages().git_timeout())
            },
        )
        .await?;
        if !status.success() {
            bail!("{}", String::from_utf8_lossy(&stderr).trim());
        }
        Ok(stdout)
    }

    pub async fn open(&self, path: &Path, initial_limit: usize) -> Result<Repository> {
        let root = self
            .run(Some(path), ["rev-parse", "--show-toplevel"])
            .await
            .context(messages().invalid_repository())?;
        let root = PathBuf::from(String::from_utf8_lossy(&root).trim());
        self.read_repository(root, None, true, initial_limit).await
    }

    /// Called by the single background polling task. Unchanged history shares
    /// its allocation; unchanged snapshots are discarded off the UI thread.
    pub async fn refresh(
        &self,
        previous: &Repository,
        update_history: bool,
        initial_limit: usize,
    ) -> Result<Option<Repository>> {
        let next = self
            .read_repository(
                previous.root.clone(),
                Some(previous),
                update_history,
                initial_limit,
            )
            .await?;
        Ok((next != *previous).then_some(next))
    }

    pub async fn browse(
        &self,
        previous: &Repository,
        scope: HistoryScope,
        initial_limit: usize,
    ) -> Result<Repository> {
        let mut previous = previous.clone();
        previous.history_scope = scope;
        previous.history_key.clear();
        self.read_repository(previous.root.clone(), Some(&previous), true, initial_limit)
            .await
    }

    pub async fn switch_branch(&self, root: &Path, name: &str) -> Result<()> {
        let status = self
            .run(
                Some(root),
                ["status", "--porcelain=v1", "-z", "--untracked-files=normal"],
            )
            .await?;
        anyhow::ensure!(status.is_empty(), messages().dirty_checkout());
        let git_dir = self.git_dir(root).await?;
        anyhow::ensure!(
            operation_states(&git_dir).is_empty(),
            messages().busy_checkout()
        );
        let reference = format!("refs/heads/{name}");
        let local = self
            .run(
                Some(root),
                [
                    "for-each-ref",
                    "--count=1",
                    "--format=%(refname)%00%(symref)",
                    "--",
                    &reference,
                ],
            )
            .await?;
        anyhow::ensure!(
            local.strip_suffix(b"\0\n") == Some(reference.as_bytes()),
            messages().local_checkout_only()
        );
        // Writes are not subject to the read timeout/cancellation policy.
        let mut command = std::process::Command::new(&self.executable);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        command
            .current_dir(root)
            .args(["switch", "--no-guess", "--", name])
            .env("GIT_TERMINAL_PROMPT", "0");
        let mut command = async_process::Command::from(command);
        let mut child = command
            .kill_on_drop(false)
            .reap_on_drop(true)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        let stdout = child.stdout.take().context(messages().read_git_output())?;
        let stderr = child.stderr.take().context(messages().read_git_error())?;
        let (_, stderr) =
            futures_lite::future::try_zip(drain_capped(stdout), drain_capped(stderr)).await?;
        let status = child.status().await?;
        anyhow::ensure!(
            status.success(),
            "{}",
            String::from_utf8_lossy(&stderr).trim()
        );
        Ok(())
    }

    async fn git_dir(&self, root: &Path) -> Result<PathBuf> {
        let output = self
            .run(Some(root), ["rev-parse", "--absolute-git-dir"])
            .await?;
        Ok(PathBuf::from(String::from_utf8_lossy(&output).trim()))
    }

    async fn read_repository(
        &self,
        root: PathBuf,
        previous: Option<&Repository>,
        update_history: bool,
        initial_limit: usize,
    ) -> Result<Repository> {
        let git_dir = if let Some(previous) = previous {
            previous.git_dir.clone()
        } else {
            self.git_dir(&root).await?
        };
        let status = self
            .run(
                Some(&root),
                [
                    "status",
                    "--porcelain=v2",
                    "--branch",
                    "--ahead-behind",
                    "-z",
                    "--untracked-files=all",
                ],
            )
            .await?;
        let status::Status {
            head,
            branch,
            tracking,
            changes,
        } = status::parse(&status)?;
        // status distinguishes an unborn branch from a failed Git query.
        // Include tags and object IDs so external ref moves invalidate decorations.
        let (refs, refs_digest) = self
            .run_with_stdout(
                Some(&root),
                [
                    "for-each-ref",
                    "--format=%(refname)%00%(symref)%00%(HEAD)%00%(objectname)",
                    "refs/heads",
                    "refs/remotes",
                    "refs/tags",
                ],
                read_refs,
            )
            .await?;
        let history_scope = previous
            .map(|repo| repo.history_scope.clone())
            .unwrap_or_default();
        let mut roots = Vec::new();
        match &history_scope {
            HistoryScope::Head => {
                if let Some(head) = &head {
                    roots.push(head.clone());
                }
            }
            HistoryScope::Branch(reference) => {
                let expression = format!("{reference}^{{commit}}");
                let object = self
                    .run(Some(&root), ["rev-parse", "--verify", &expression])
                    .await?;
                roots.push(String::from_utf8_lossy(&object).trim().to_string());
            }
            HistoryScope::All => {
                for line in String::from_utf8_lossy(&refs).lines() {
                    let fields: Vec<_> = line.split('\0').collect();
                    if fields.len() == 4
                        && fields[1].is_empty()
                        && (fields[0].starts_with("refs/heads/")
                            || fields[0].starts_with("refs/remotes/"))
                    {
                        roots.push(fields[3].to_string());
                    }
                }
                if let Some(head) = &head {
                    roots.push(head.clone());
                }
                roots.sort();
                roots.dedup();
            }
        }
        let mut history_key = head.as_deref().unwrap_or_default().as_bytes().to_vec();
        history_key.push(0);
        history_key.extend_from_slice(&refs_digest);
        history_key.extend_from_slice(format!("{history_scope:?}").as_bytes());
        let history_pending =
            !update_history && previous.is_some_and(|repo| repo.history_key != history_key);
        let unchanged_history =
            previous.filter(|repo| history_pending || repo.history_key == history_key);
        if history_pending {
            history_key = previous.unwrap().history_key.clone();
        }
        let initial_limit = initial_limit.max(1);
        let mut graph_lanes = Vec::new();
        let commits = if let Some(previous) = unchanged_history {
            previous.commits.clone()
        } else if !roots.is_empty() {
            let mut args: Vec<String> = [
                "log",
                "-z",
                "--date=format-local:%Y-%m-%d %H:%M",
                "--format=%H%x00%h%x00%an%x00%ad%x00%s%x00%P%x00%D",
                "--topo-order",
            ]
            .into_iter()
            .map(String::from)
            .collect();
            args.push(format!("--max-count={initial_limit}"));
            args.extend(roots.clone());
            args.push("--".into());
            let log = self.run(Some(&root), args).await?;
            Arc::new(History::new(parse_log(&log)))
        } else {
            Arc::new(History::new(Vec::new()))
        };
        let stashes = self
            .run(
                Some(&root),
                [
                    "stash",
                    "list",
                    "-z",
                    "--date=format-local:%Y-%m-%d %H:%M",
                    "--format=%H%x00%cd%x00%s",
                ],
            )
            .await?;
        let branches: Arc<[Branch]> = parse_branches(&refs).into();
        let branch = if branch == "(detached)"
            && head.is_some()
            && !branches.iter().any(|branch| branch.current)
        {
            messages().detached_head().into()
        } else {
            branch
        };
        let graph = unchanged_history
            .map(|repo| repo.graph.clone())
            .unwrap_or_else(|| {
                Arc::new(History::new(crate::graph::extend(
                    commits.iter(),
                    &mut graph_lanes,
                )))
            });
        let history_complete = unchanged_history
            .map(|repo| repo.history_complete)
            .unwrap_or(commits.len() < initial_limit);
        let graph_lanes = unchanged_history
            .map(|repo| repo.graph_lanes.clone())
            .unwrap_or_else(|| graph_lanes.into());
        let graph_width = unchanged_history
            .map(|repo| repo.graph_width)
            .unwrap_or_else(|| graph.iter().map(|row| row.width).max().unwrap_or(1));
        let mut operation_states = operation_states(&git_dir);
        let conflicts = changes
            .iter()
            .filter(|file| file.status.contains('U') || file.status == "AA" || file.status == "DD")
            .count();
        if conflicts > 0 {
            operation_states.push(messages().conflicts(conflicts));
        }
        let (has_other_worktrees, worktree_error) = match self.worktree_entries(&root).await {
            Ok(entries) => (
                entries
                    .iter()
                    .filter(|entry| entry.available)
                    .nth(1)
                    .is_some(),
                None,
            ),
            Err(error) => (
                previous.is_some_and(|repo| repo.has_other_worktrees),
                Some(format!("{error:#}")),
            ),
        };
        Ok(Repository {
            root,
            has_other_worktrees,
            worktree_error,
            head,
            git_dir,
            operation_states,
            tracking,
            branch,
            changes,
            graph,
            commits,
            stashes: stash::parse_stashes(&stashes),
            branches,
            history_key,
            history_complete,
            history_pending,
            graph_lanes,
            graph_width,
            history_scope,
            history_roots: unchanged_history
                .map(|repo| repo.history_roots.clone())
                .unwrap_or_else(|| roots.into()),
        })
    }

    pub async fn commit_files(&self, root: &Path, id: &str) -> Result<Vec<ChangedFile>> {
        let output = self
            .run(
                Some(root),
                [
                    "show",
                    "--format=",
                    "--diff-merges=first-parent",
                    "--name-status",
                    "--no-renames",
                    "-z",
                    id,
                    "--",
                ],
            )
            .await?;
        let fields: Vec<_> = output.split(|b| *b == 0).collect();
        Ok(fields
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| ChangedFile {
                original_path: None,
                status: String::from_utf8_lossy(pair[0]).trim().into(),
                path: String::from_utf8_lossy(pair[1]).into(),
            })
            .collect())
    }

    pub async fn commit_body(&self, root: &Path, id: &str) -> Result<String> {
        let output = self
            .run(Some(root), ["show", "-s", "--format=%B", id, "--"])
            .await?;
        let message = String::from_utf8_lossy(&output);
        if message.len() > 64 * 1024 {
            let mut end = 64 * 1024;
            while !message.is_char_boundary(end) {
                end -= 1;
            }
            Ok(messages().truncated_message(&message[..end]))
        } else {
            Ok(message.trim().to_string())
        }
    }

    /// Read original bytes without text conversion. None selects the worktree;
    /// a missing file is distinct from a failed or oversized read.
    pub async fn image_file(
        &self,
        root: &Path,
        revision: Option<&str>,
        path: &str,
    ) -> Result<Option<Vec<u8>>> {
        if revision == Some(":") {
            let entry = self
                .run(Some(root), ["ls-files", "--stage", "-z", "--", path])
                .await?;
            if entry.is_empty() {
                return Ok(None);
            }
            let header = entry.split(|b| *b == b'\t').next().unwrap_or_default();
            let fields: Vec<_> = std::str::from_utf8(header)?.split_whitespace().collect();
            anyhow::ensure!(
                fields.len() == 3 && fields[2] == "0" && fields[0].starts_with("100"),
                messages().invalid_index_image()
            );
            let size = self.run(Some(root), ["cat-file", "-s", fields[1]]).await?;
            anyhow::ensure!(
                std::str::from_utf8(&size)?.trim().parse::<u64>()? <= IMAGE_FILE_LIMIT,
                messages().image_too_large()
            );
            return self
                .run(Some(root), ["cat-file", "blob", fields[1]])
                .await
                .map(Some);
        }
        if let Some(revision) = revision {
            let entry = self
                .run(Some(root), ["ls-tree", "-l", "-z", revision, "--", path])
                .await?;
            if entry.is_empty() {
                return Ok(None);
            }
            let header = entry
                .split(|byte| *byte == b'\t')
                .next()
                .unwrap_or_default();
            let header = std::str::from_utf8(header)?;
            let fields: Vec<_> = header.split_whitespace().collect();
            anyhow::ensure!(
                fields.len() == 4 && fields[0].starts_with("100") && fields[1] == "blob",
                messages().invalid_image_version()
            );
            anyhow::ensure!(
                fields[3].parse::<u64>()? <= IMAGE_FILE_LIMIT,
                messages().image_too_large()
            );
            self.run(Some(root), ["cat-file", "blob", fields[2]])
                .await
                .map(Some)
        } else {
            use std::io::Read;
            let path = root.join(path);
            let metadata = match std::fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error).context(messages().read_image()),
            };
            anyhow::ensure!(metadata.is_file(), messages().invalid_image_path());
            anyhow::ensure!(
                metadata.len() <= IMAGE_FILE_LIMIT,
                messages().image_too_large()
            );
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take(IMAGE_FILE_LIMIT + 1)
                .read_to_end(&mut bytes)?;
            anyhow::ensure!(
                bytes.len() as u64 <= IMAGE_FILE_LIMIT,
                messages().image_too_large()
            );
            Ok(Some(bytes))
        }
    }

    pub async fn diff(
        &self,
        root: &Path,
        commit: Option<&str>,
        file: &ChangedFile,
    ) -> Result<String> {
        self.diff_scoped(root, commit, file, ChangeScope::All).await
    }

    pub async fn diff_scoped(
        &self,
        root: &Path,
        commit: Option<&str>,
        file: &ChangedFile,
        scope: ChangeScope,
    ) -> Result<String> {
        let output = if let Some(id) = commit {
            self.run(
                Some(root),
                [
                    "show",
                    "--format=",
                    "--diff-merges=first-parent",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--no-renames",
                    id,
                    "--",
                    &file.path,
                ],
            )
            .await?
        } else if file.status == "??" {
            let path = root.join(&file.path);
            let metadata = std::fs::metadata(&path)?;
            anyhow::ensure!(metadata.len() <= 1024 * 1024, messages().file_too_large());
            let text = std::fs::read_to_string(&path).context(messages().non_text())?;
            let count = text.lines().count();
            return Ok(format!(
                "diff --git a/{0} b/{0}\nnew file mode 100644\n--- /dev/null\n+++ b/{0}\n@@ -0,0 +1,{1} @@\n{2}",
                file.path,
                count,
                text.lines()
                    .map(|line| format!("+{line}\n"))
                    .collect::<String>()
            ));
        } else {
            let mut args: Vec<OsString> =
                ["diff", "--no-ext-diff", "--no-textconv", "--no-renames"]
                    .into_iter()
                    .map(OsString::from)
                    .collect();
            match scope {
                ChangeScope::Staged => args.push("--cached".into()),
                ChangeScope::Working => {}
                ChangeScope::All => {
                    let head = self
                        .run(Some(root), ["rev-parse", "--verify", "HEAD"])
                        .await;
                    if let Ok(head) = head {
                        args.push(String::from_utf8_lossy(&head).trim().into());
                    } else {
                        args.push(self.empty_tree(root).await?.into());
                    }
                }
            }
            args.push("--".into());
            args.push(file.path.clone().into());
            if let Some(original) = &file.original_path {
                args.push(original.into());
            }
            self.run(Some(root), args).await?
        };
        anyhow::ensure!(output.len() <= 2 * 1024 * 1024, messages().diff_too_large());
        Ok(String::from_utf8_lossy(&output).into())
    }

    async fn empty_tree(&self, root: &Path) -> Result<String> {
        let output = self
            .run(Some(root), ["hash-object", "-t", "tree", "--stdin"])
            .await?;
        Ok(String::from_utf8_lossy(&output).trim().into())
    }
}

async fn drain_capped(mut reader: impl AsyncRead + Unpin) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            return Ok(output);
        }
        let keep = count.min((64 * 1024_usize).saturating_sub(output.len()));
        output.extend_from_slice(&buffer[..keep]);
    }
}

async fn read_limited(reader: impl AsyncRead + Unpin, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes).await?;
    anyhow::ensure!(bytes.len() as u64 <= limit, messages().output_too_large());
    Ok(bytes)
}

async fn read_refs(reader: impl AsyncRead + Unpin) -> Result<(Vec<u8>, [u8; 32])> {
    use futures_lite::io::{AsyncBufReadExt, BufReader};
    use sha2::{Digest, Sha256};

    const LIMIT: usize = 8 * 1024 * 1024;
    let mut reader = BufReader::new(reader);
    let mut record = Vec::new();
    let mut branches = Vec::new();
    let mut digest = Sha256::new();
    loop {
        record.clear();
        let count = (&mut reader)
            .take(LIMIT as u64 + 1)
            .read_until(b'\n', &mut record)
            .await?;
        if count == 0 {
            break;
        }
        anyhow::ensure!(count <= LIMIT, messages().ref_too_large());
        digest.update(&record);
        // Tags affect log decorations, but never appear in the branch picker or roots.
        if !record.starts_with(b"refs/tags/") {
            anyhow::ensure!(
                branches.len() + count <= LIMIT,
                messages().branches_too_large()
            );
            branches.extend_from_slice(&record);
        }
    }
    Ok((branches, digest.finalize().into()))
}

fn operation_states(git_dir: &Path) -> Vec<String> {
    [
        ("MERGE_HEAD", messages().merging()),
        ("rebase-merge", messages().rebasing()),
        ("rebase-apply", messages().applying_patch()),
        ("CHERRY_PICK_HEAD", messages().cherry_picking()),
        ("REVERT_HEAD", messages().reverting()),
        ("BISECT_LOG", messages().bisecting()),
    ]
    .into_iter()
    .filter(|(marker, _)| git_dir.join(marker).exists())
    .map(|(_, label)| label.to_string())
    .collect()
}

fn find_on_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(if cfg!(windows) { "git.exe" } else { "git" }))
        .find(|file| {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                file.metadata()
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            }
            #[cfg(windows)]
            {
                file.is_file()
            }
        })
}
fn parse_log(bytes: &[u8]) -> Vec<Commit> {
    let fields: Vec<_> = bytes.split(|b| *b == 0).collect();
    fields
        .as_chunks::<7>()
        .0
        .iter()
        .map(|f| Commit {
            id: String::from_utf8_lossy(f[0]).into(),
            short_id: String::from_utf8_lossy(f[1]).into(),
            author: String::from_utf8_lossy(f[2]).into(),
            date: String::from_utf8_lossy(f[3]).into(),
            subject: String::from_utf8_lossy(f[4]).into(),
            parents: String::from_utf8_lossy(f[5])
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
            references: String::from_utf8_lossy(f[6])
                .split(", ")
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
        })
        .collect()
}
fn parse_status(bytes: &[u8]) -> Vec<ChangedFile> {
    let mut records = bytes.split(|b| *b == 0);
    let mut files = Vec::new();
    while let Some(record) = records.next() {
        if record.len() < 4 {
            continue;
        }
        let status = String::from_utf8_lossy(&record[..2]).into_owned();
        let path = String::from_utf8_lossy(&record[3..]).into_owned();
        let original_path = if status.contains('R') || status.contains('C') {
            records
                .next()
                .map(|path| String::from_utf8_lossy(path).into_owned())
        } else {
            None
        };
        files.push(ChangedFile {
            path,
            original_path,
            status,
        });
    }
    files
}

fn parse_branches(bytes: &[u8]) -> Vec<Branch> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\0');
            let reference = fields.next()?;
            // origin/HEAD is a symbolic alias, not a separate remote branch.
            if !fields.next()?.is_empty() {
                return None;
            }
            let current = fields.next()? == "*";
            let (name, remote) = if let Some(name) = reference.strip_prefix("refs/heads/") {
                (name, false)
            } else {
                (reference.strip_prefix("refs/remotes/")?, true)
            };
            Some(Branch {
                name: name.into(),
                remote,
                current,
            })
        })
        .collect()
}

fn parse_tracking(bytes: &[u8]) -> Option<BranchTracking> {
    let mut upstream = None;
    let mut counts = None;
    for header in bytes
        .split(|b| *b == 0)
        .take_while(|line| line.starts_with(b"# "))
    {
        let header = String::from_utf8_lossy(header);
        if let Some(value) = header.strip_prefix("# branch.upstream ") {
            upstream = Some(value.to_string());
        } else if let Some(value) = header.strip_prefix("# branch.ab ") {
            let mut parts = value.split_whitespace();
            counts = parts
                .next()
                .and_then(|n| n.strip_prefix('+'))
                .and_then(|n| n.parse().ok())
                .zip(
                    parts
                        .next()
                        .and_then(|n| n.strip_prefix('-'))
                        .and_then(|n| n.parse().ok()),
                );
        }
    }
    upstream.map(|upstream| BranchTracking { upstream, counts })
}

#[cfg(test)]
mod tests {
    #[test]
    fn branches_use_ref_namespaces_and_skip_symbolic_aliases() {
        let branches = super::parse_branches(
            b"refs/heads/origin/local\0\0 \nrefs/heads/main\0\0*\nrefs/remotes/origin/HEAD\0refs/remotes/origin/main\0 \nrefs/remotes/origin/main\0\0 \n",
        );
        assert_eq!(branches.len(), 3);
        assert_eq!(branches[0].name, "origin/local");
        assert!(!branches[0].remote);
        assert!(branches[1].current);
        assert!(branches[2].remote);
        assert_eq!(branches[2].name, "origin/main");
    }

    use super::*;
    #[test]
    fn tags_beyond_eight_mib_still_invalidate_history() {
        let branch = b"refs/heads/main\0\0*\0abc\n";
        let tag = b"refs/tags/a-long-tag-name\0\0 \0abc\n";
        let mut output = branch.to_vec();
        output.extend(tag.repeat(8 * 1024 * 1024 / tag.len() + 1));
        futures_lite::future::block_on(async {
            let (kept, first) = read_refs(output.as_slice()).await.unwrap();
            assert_eq!(kept, branch);
            output.extend_from_slice(b"refs/tags/last\0\0 \0def\n");
            let (kept, second) = read_refs(output.as_slice()).await.unwrap();
            assert_eq!(kept, branch);
            assert_ne!(first, second);
        });
    }
    #[test]
    fn tracking_counts_keep_push_pull_order_and_unknown_state() {
        assert_eq!(
            parse_tracking(b"# branch.upstream origin/main\0# branch.ab +3 -5\0"),
            Some(BranchTracking {
                upstream: "origin/main".into(),
                counts: Some((3, 5))
            })
        );
        assert_eq!(
            parse_tracking(b"# branch.upstream origin/main\0# branch.ab +0 -0\0")
                .unwrap()
                .counts,
            Some((0, 0))
        );
        assert_eq!(
            parse_tracking(b"# branch.upstream origin/gone\0")
                .unwrap()
                .counts,
            None
        );
        assert!(parse_tracking(b"# branch.head (detached)\0").is_none());
    }
    #[test]
    fn nul_status_preserves_paths_and_rename_boundaries() {
        let files =
            parse_status(" M src/a b.rs\0R  新文件.rs\0旧文件.rs\0?? :special.txt\0".as_bytes());
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].path, "src/a b.rs");
        assert_eq!(files[1].path, "新文件.rs");
        assert_eq!(files[1].original_path.as_deref(), Some("旧文件.rs"));
        assert_eq!(files[2].path, ":special.txt");
    }
    #[test]
    fn log_preserves_subject_and_author_fields() {
        let commits = parse_log(
            b"abc\0abc\0Author\x002026-11-06 12:00\0hello | world\0parent\0HEAD -> main, tag: v1\0",
        );
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].subject, "hello | world");
        assert_eq!(commits[0].parents, ["parent"]);
        assert_eq!(commits[0].references.len(), 2);
    }

    #[test]
    fn image_versions_read_raw_bytes_across_commits_renames_and_deletion() {
        futures_lite::future::block_on(async {
            let git = Git::detect(None).await.expect("测试需要系统 Git");
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path();
            let old = b"\x89PNG\0old\xff";
            let staged = b"\x89PNG\0staged\xfe";
            let modified = b"\x89PNG\0worktree\xfd";
            git.run(Some(root), ["init", "-b", "main"]).await.unwrap();
            git.run(Some(root), ["config", "user.name", "githly test"])
                .await
                .unwrap();
            git.run(Some(root), ["config", "user.email", "test@example.invalid"])
                .await
                .unwrap();
            let commit = [
                "-c",
                crate::git::history_tests::NO_HOOKS,
                "commit",
                "--no-gpg-sign",
                "-m",
                "image",
            ];
            std::fs::write(root.join("旧 图片.png"), old).unwrap();
            assert_eq!(
                git.image_file(root, None, "旧 图片.png")
                    .await
                    .unwrap()
                    .unwrap(),
                old
            );
            git.run(Some(root), ["add", "--", "."]).await.unwrap();
            git.run(Some(root), commit).await.unwrap();
            let initial = git.open(root, 46).await.unwrap().commits[0].clone();
            assert!(initial.parents.is_empty());
            assert!(
                git.image_file(root, Some(&initial.id), "missing.png")
                    .await
                    .unwrap()
                    .is_none()
            );

            std::fs::write(root.join("旧 图片.png"), staged).unwrap();
            git.run(Some(root), ["add", "--", "."]).await.unwrap();
            std::fs::write(root.join("旧 图片.png"), modified).unwrap();
            assert_eq!(
                git.image_file(root, Some("HEAD"), "旧 图片.png")
                    .await
                    .unwrap()
                    .unwrap(),
                old
            );
            assert_eq!(
                git.image_file(root, None, "旧 图片.png")
                    .await
                    .unwrap()
                    .unwrap(),
                modified
            );
            git.run(Some(root), commit).await.unwrap();
            assert_eq!(
                git.image_file(root, Some("HEAD"), "旧 图片.png")
                    .await
                    .unwrap()
                    .unwrap(),
                staged
            );
            assert_eq!(
                git.image_file(root, Some("HEAD^"), "旧 图片.png")
                    .await
                    .unwrap()
                    .unwrap(),
                old
            );

            std::fs::write(root.join("旧 图片.png"), staged).unwrap();
            git.run(Some(root), ["mv", "--", "旧 图片.png", "新 图片.png"])
                .await
                .unwrap();
            let repository = git.open(root, 46).await.unwrap();
            let renamed = &repository.changes[0];
            assert_eq!(renamed.path, "新 图片.png");
            assert_eq!(renamed.original_path.as_deref(), Some("旧 图片.png"));
            assert_eq!(
                git.image_file(
                    root,
                    Some("HEAD"),
                    renamed.original_path.as_deref().unwrap()
                )
                .await
                .unwrap()
                .unwrap(),
                staged
            );
            assert_eq!(
                git.image_file(root, None, &renamed.path)
                    .await
                    .unwrap()
                    .unwrap(),
                staged
            );
            git.run(Some(root), commit).await.unwrap();
            git.run(Some(root), ["rm", "--", "新 图片.png"])
                .await
                .unwrap();
            assert!(
                git.image_file(root, None, "新 图片.png")
                    .await
                    .unwrap()
                    .is_none()
            );
            git.run(Some(root), commit).await.unwrap();
            assert!(
                git.image_file(root, Some("HEAD"), "新 图片.png")
                    .await
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                git.image_file(root, Some("HEAD^"), "新 图片.png")
                    .await
                    .unwrap()
                    .unwrap(),
                staged
            );

            std::fs::File::create(root.join("large.png"))
                .unwrap()
                .set_len(IMAGE_FILE_LIMIT + 1)
                .unwrap();
            assert!(
                git.image_file(root, None, "large.png")
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("8 MiB")
            );
            git.run(Some(root), ["add", "--", "large.png"])
                .await
                .unwrap();
            git.run(Some(root), commit).await.unwrap();
            assert!(
                git.image_file(root, Some("HEAD"), "large.png")
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("8 MiB")
            );
        });
    }

    #[test]
    fn reads_real_repository_and_keeps_file_diffs_distinct() {
        futures_lite::future::block_on(async {
            let git = Git::detect(None).await.expect("测试需要系统 Git");
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path();
            let command = |args: &[&str]| {
                let mut command = std::process::Command::new(&git.executable);
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    command.creation_flags(0x08000000);
                }
                let output = command.current_dir(root).args(args).output().unwrap();
                assert!(
                    output.status.success(),
                    "git {args:?} failed ({}): {} {}",
                    output.status,
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            };
            command(&["init", "-b", "main"]);
            command(&["config", "user.name", "githly test"]);
            command(&["config", "user.email", "test@example.invalid"]);
            std::fs::write(root.join("文件 a.txt"), "first file\n").unwrap();
            std::fs::write(root.join("b.txt"), "second file\n").unwrap();
            command(&["add", "--", "."]);
            command(&[
                "-c",
                crate::git::history_tests::NO_HOOKS,
                "commit",
                "-m",
                "初次提交 | test",
            ]);
            let repo = git.open(root, 46).await.unwrap();
            assert_eq!(repo.branch, "main");
            assert!(repo.tracking.is_none());
            assert_eq!(repo.commits.len(), 1);
            assert_eq!(repo.commits[0].subject, "初次提交 | test");
            assert!(repo.changes.is_empty());
            let files = git.commit_files(root, &repo.commits[0].id).await.unwrap();
            assert_eq!(files.len(), 2);
            let a = files.iter().find(|f| f.path == "文件 a.txt").unwrap();
            let b = files.iter().find(|f| f.path == "b.txt").unwrap();
            let patch_a = git.diff(root, Some(&repo.commits[0].id), a).await.unwrap();
            let patch_b = git.diff(root, Some(&repo.commits[0].id), b).await.unwrap();
            assert!(patch_a.contains("+first file"));
            assert!(!patch_b.contains("first file"));
            assert!(patch_b.contains("+second file"));
            assert_eq!(
                gpui_kit::component::diff::DiffFile::parse(&patch_a)
                    .unwrap()
                    .len(),
                1
            );
            std::fs::write(root.join("b.txt"), "changed second file\n").unwrap();
            let repo = git.open(root, 46).await.unwrap();
            assert_eq!(repo.changes.len(), 1);
            let patch = git.diff(root, None, &repo.changes[0]).await.unwrap();
            assert!(patch.contains("+changed second file"));
            // A local upstream exercises ahead/behind without any network access.
            command(&["branch", "upstream"]);
            command(&["branch", "--set-upstream-to=upstream", "main"]);
            assert_eq!(
                git.open(root, 46).await.unwrap().tracking.unwrap().counts,
                Some((0, 0))
            );
            command(&[
                "-c",
                crate::git::history_tests::NO_HOOKS,
                "commit",
                "--allow-empty",
                "-m",
                "local only",
            ]);
            assert_eq!(
                git.open(root, 46).await.unwrap().tracking.unwrap().counts,
                Some((1, 0))
            );
            command(&["switch", "upstream"]);
            command(&[
                "-c",
                crate::git::history_tests::NO_HOOKS,
                "commit",
                "--allow-empty",
                "-m",
                "upstream only",
            ]);
            command(&["switch", "main"]);
            assert_eq!(
                git.open(root, 46).await.unwrap().tracking.unwrap().counts,
                Some((1, 1))
            );
        });
    }

    #[test]
    fn missing_configured_git_is_reported_without_falling_back() {
        let directory = tempfile::tempdir().unwrap();
        assert!(
            futures_lite::future::block_on(Git::detect(Some(directory.path().join("git.exe"))))
                .is_err()
        );
    }

    #[test]
    fn oversized_output_is_rejected_at_the_reader() {
        let input = futures_lite::io::Cursor::new(vec![b'x'; 128]);
        assert!(futures_lite::future::block_on(read_limited(input, 32)).is_err());
    }
}
