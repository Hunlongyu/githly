use super::*;

#[derive(Clone)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: LocalizedText,
    status: Option<LocalizedText>,
    locked: bool,
    pub available: bool,
}

impl Worktree {
    pub fn status_label(&self) -> String {
        match (&self.status, self.locked) {
            (Some(status), true) => format!("{} · {status}", messages().locked()),
            (Some(status), false) => status.to_string(),
            (None, true) => messages().locked().into(),
            (None, false) => messages().status_unchecked().into(),
        }
    }
}

impl Git {
    pub(super) async fn worktree_entries(&self, root: &Path) -> Result<Vec<Worktree>> {
        let output = self
            .run(Some(root), ["worktree", "list", "--porcelain", "-z"])
            .await?;
        let mut entries = Vec::<Worktree>::new();
        for field in output.split(|b| *b == 0) {
            if let Some(path) = field.strip_prefix(b"worktree ") {
                let path = path.to_path()?;
                entries.push(Worktree {
                    path: PathBuf::from(path),
                    branch: LocalizedText::new(Locale::detached_head),
                    status: None,
                    locked: false,
                    available: path.is_dir(),
                });
            } else if let Some(entry) = entries.last_mut() {
                let text = String::from_utf8_lossy(field);
                if let Some(branch) = text.strip_prefix("branch refs/heads/") {
                    entry.branch = LocalizedText::Verbatim(branch.into());
                }
                if text == "bare" {
                    entry.available = false;
                    entry.branch = LocalizedText::new(Locale::bare_repository);
                }
                if text.starts_with("locked") {
                    entry.locked = true;
                }
                if text.starts_with("prunable") {
                    entry.available = false;
                    entry.status = Some(LocalizedText::new(Locale::directory_missing));
                }
            }
        }
        Ok(entries)
    }

    pub async fn worktrees(&self, root: &Path) -> Result<Vec<Worktree>> {
        let mut entries = self.worktree_entries(root).await?;
        use futures_util::stream::{self, StreamExt};
        // Preserve Git's menu order while at most four worktrees run status.
        let available = entries.iter_mut().filter(|entry| entry.available).take(64);
        stream::iter(available)
            .for_each_concurrent(4, |entry| async move {
                entry.status = Some(
                    match self
                        .run(
                            Some(&entry.path),
                            ["status", "--porcelain=v1", "-z", "--untracked-files=normal"],
                        )
                        .await
                    {
                        Ok(output) => {
                            let count = parse_status(&output).len();
                            if count == 0 {
                                LocalizedText::new(Locale::clean_worktree)
                            } else {
                                LocalizedText::new(|l| l.worktree_changes(count))
                            }
                        }
                        Err(error) => LocalizedText::with_error(error, |l, e| l.unreadable(e)),
                    },
                );
            })
            .await;
        Ok(entries)
    }
}
