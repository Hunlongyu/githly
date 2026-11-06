use super::*;
use std::collections::HashMap;

#[derive(Default)]
pub struct LineStatistics {
    pub files: HashMap<String, Option<(u64, u64)>>,
    pub added: u64,
    pub removed: u64,
    pub incomplete: bool,
}

impl LineStatistics {
    fn insert(&mut self, path: String, counts: Option<(u64, u64)>) {
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
        commit: Option<&str>,
        scope: ChangeScope,
        changes: &[ChangedFile],
    ) -> Result<LineStatistics> {
        let mut args: Vec<String> = if let Some(commit) = commit {
            ["show", "--format=", "--diff-merges=first-parent", commit]
                .into_iter()
                .map(String::from)
                .collect()
        } else {
            let mut args = vec!["diff".to_string()];
            match scope {
                ChangeScope::Staged => args.push("--cached".into()),
                ChangeScope::Working => {}
                ChangeScope::All => {
                    let head = self
                        .run(Some(root), ["rev-parse", "--verify", "HEAD"])
                        .await;
                    args.push(if let Ok(head) = head {
                        String::from_utf8_lossy(&head).trim().into()
                    } else {
                        self.empty_tree(root).await?
                    });
                }
            }
            args
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
                stats.insert(
                    String::from_utf8_lossy(path).into_owned(),
                    added.zip(removed),
                );
            }
        }
        // Bound untracked-file I/O. Large/binary/remaining files keep an explicit
        // unavailable value; their line count is never reported as zero.
        let mut budget = 8 * 1024 * 1024_u64;
        let mut read_count = 0;
        if commit.is_none() && scope != ChangeScope::Staged {
            for file in changes.iter().filter(|file| file.status == "??") {
                let path = root.join(&file.path);
                let counts = if read_count < 256 {
                    if let Ok(metadata) = std::fs::symlink_metadata(&path) {
                        if metadata.is_file()
                            && metadata.len() <= 1024 * 1024
                            && metadata.len() <= budget
                        {
                            budget -= metadata.len();
                            read_count += 1;
                            // Cap the read as the file can grow after metadata.
                            use std::io::Read;
                            let mut bytes = Vec::new();
                            let result = std::fs::File::open(path).and_then(|file| {
                                file.take(1024 * 1024 + 1).read_to_end(&mut bytes)
                            });
                            if result.is_ok() && bytes.len() <= 1024 * 1024 && !bytes.contains(&0) {
                                std::str::from_utf8(&bytes)
                                    .ok()
                                    .map(|text| (text.lines().count() as u64, 0))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };
                stats.insert(file.path.clone(), counts);
            }
        }
        Ok(stats)
    }
}
