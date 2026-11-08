use super::*;
use std::collections::HashMap;

#[derive(Default)]
pub struct LineStatistics {
    pub files: HashMap<BString, Option<(u64, u64)>>,
    pub added: u64,
    pub removed: u64,
    pub incomplete: bool,
}

impl LineStatistics {
    fn insert(&mut self, path: BString, counts: Option<(u64, u64)>) {
        if let Some((added, removed)) = counts {
            self.added += added;
            self.removed += removed;
        } else {
            self.incomplete = true;
        }
        self.files.insert(path, counts);
    }
}

impl Git {
    pub async fn line_statistics(
        &self,
        root: &Path,
        target: DiffTarget<'_>,
        changes: &[ChangedFile],
    ) -> Result<LineStatistics> {
        let mut args: Vec<String> = match target {
            DiffTarget::Commit(commit) => {
                ["show", "--format=", "--diff-merges=first-parent", commit]
                    .into_iter()
                    .map(String::from)
                    .collect()
            }
            DiffTarget::WorkingTree { head } => {
                vec!["diff".to_string(), self.worktree_base(root, head).await?]
            }
        };
        args.extend(
            [
                "--numstat",
                "-z",
                "--no-renames",
                "--no-ext-diff",
                "--no-textconv",
                "--",
            ]
            .into_iter()
            .map(String::from),
        );
        let output = self.run(Some(root), args).await?;
        let mut stats = LineStatistics::default();
        for record in output
            .split(|b| *b == 0)
            .filter(|record| !record.is_empty())
        {
            let mut fields = record.splitn(3, |b| *b == b'\t');
            let added = fields
                .next()
                .and_then(|v| std::str::from_utf8(v).ok())
                .and_then(|v| v.parse().ok());
            let removed = fields
                .next()
                .and_then(|v| std::str::from_utf8(v).ok())
                .and_then(|v| v.parse().ok());
            if let Some(path) = fields.next() {
                stats.insert(path.into(), added.zip(removed));
            }
        }
        // Bound untracked-file I/O. Large/binary/remaining files keep an explicit
        // unavailable value; their line count is never reported as zero.
        if matches!(target, DiffTarget::WorkingTree { .. }) {
            let mut files = changes.iter().filter(|file| file.status == "??");
            let sample: Vec<_> = files
                .by_ref()
                .take(256)
                .map(|file| file.path.clone())
                .collect();
            for file in files {
                stats.insert(file.path.clone(), None);
            }
            if !sample.is_empty() {
                let root = root.to_owned();
                stats = blocking::unblock(move || {
                    let mut budget = 8 * 1024 * 1024_u64;
                    for path in sample {
                        let counts = if budget == 0 {
                            None
                        } else {
                            path.to_path()
                                .ok()
                                .and_then(|name| untracked_lines(&root.join(name), &mut budget))
                                .map(|lines| (lines, 0))
                        };
                        stats.insert(path, counts);
                    }
                    stats
                })
                .await;
            }
        }
        Ok(stats)
    }
}

fn untracked_lines(path: &Path, budget: &mut u64) -> Option<u64> {
    use std::io::Read;

    const FILE_LIMIT: u64 = 1024 * 1024;
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > FILE_LIMIT.min(*budget) {
        return None;
    }
    let mut file = std::fs::File::open(path).ok()?;
    let limit = (FILE_LIMIT + 1).min(*budget);
    let mut bytes = Vec::new();
    let result = file.by_ref().take(limit).read_to_end(&mut bytes);
    let size = bytes.len() as u64;
    *budget -= size;
    result.ok()?;
    if size > FILE_LIMIT
        || (size == limit && file.metadata().ok()?.len() > size)
        || bytes.contains(&0)
    {
        return None;
    }
    Some(std::str::from_utf8(&bytes).ok()?.lines().count() as u64)
}
