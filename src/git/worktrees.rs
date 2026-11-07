use super::*;

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
                    branch: "分离 HEAD".into(),
                    status: "未核对状态".into(),
                    available: Path::new(path).is_dir(),
                });
            } else if let Some(entry) = entries.last_mut() {
                if let Some(branch) = text.strip_prefix("branch refs/heads/") {
                    entry.branch = branch.into();
                }
                if text == "bare" {
                    entry.available = false;
                    entry.branch = "裸仓库".into();
                }
                if text.starts_with("locked") {
                    entry.status = "已锁定".into();
                }
                if text.starts_with("prunable") {
                    entry.available = false;
                    entry.status = "目录不可用".into();
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
            let note = (entry.status != "未核对状态").then(|| entry.status.clone());
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
                        "工作区干净".into()
                    } else {
                        format!("{count} 项更改")
                    }
                }
                Err(error) => format!("不可读取：{error}"),
            };
            entry.status = note.map_or_else(|| status.clone(), |note| format!("{note} · {status}"));
        }
        Ok(entries)
    }
}
