use super::*;
use crate::i18n::messages;

#[derive(Clone)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: String,
    pub status: String,
    pub available: bool,
}

impl Git {
    pub(super) async fn worktree_entries(&self, root: &Path) -> Result<Vec<Worktree>> {
        let output = self
            .run(Some(root), ["worktree", "list", "--porcelain", "-z"])
            .await?;
        let mut entries = Vec::<Worktree>::new();
        for field in output.split(|b| *b == 0) {
            let text = String::from_utf8_lossy(field);
            if let Some(path) = text.strip_prefix("worktree ") {
                entries.push(Worktree {
                    path: PathBuf::from(path),
                    branch: messages().detached_head().into(),
                    status: messages().status_unchecked().into(),
                    available: Path::new(path).is_dir(),
                });
            } else if let Some(entry) = entries.last_mut() {
                if let Some(branch) = text.strip_prefix("branch refs/heads/") {
                    entry.branch = branch.into();
                }
                if text == "bare" {
                    entry.available = false;
                    entry.branch = messages().bare_repository().into();
                }
                if text.starts_with("locked") {
                    entry.status = messages().locked().into();
                }
                if text.starts_with("prunable") {
                    entry.available = false;
                    entry.status = messages().directory_missing().into();
                }
            }
        }
        Ok(entries)
    }

    pub async fn worktrees(&self, root: &Path) -> Result<Vec<Worktree>> {
        let mut entries = self.worktree_entries(root).await?;
        // Only read status when the menu is opened, never every poll for
        // all worktrees. Keep serial I/O and a bounded number of queries.
        for entry in entries.iter_mut().filter(|entry| entry.available).take(64) {
            let note =
                (entry.status != messages().status_unchecked()).then(|| entry.status.clone());
            let status = match self
                .run(
                    Some(&entry.path),
                    ["status", "--porcelain=v1", "-z", "--untracked-files=normal"],
                )
                .await
            {
                Ok(output) => {
                    let count = parse_status(&output).len();
                    if count == 0 {
                        messages().clean_worktree().into()
                    } else {
                        messages().worktree_changes(count)
                    }
                }
                Err(error) => messages().unreadable(error),
            };
            entry.status = note.map_or_else(|| status.clone(), |note| format!("{note} · {status}"));
        }
        Ok(entries)
    }
}
