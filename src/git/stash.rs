use super::*;
use crate::i18n::messages;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stash {
    pub id: String,
    pub selector: String,
    pub date: String,
    pub subject: String,
}

pub struct StashDetail {
    pub commit: Commit,
    pub files: Vec<ChangedFile>,
    pub untracked_revision: Option<String>,
    pub untracked_paths: std::collections::HashSet<String>,
    pub body: String,
}

pub(super) fn parse_stashes(bytes: &[u8]) -> Vec<Stash> {
    bytes
        .split(|b| *b == 0)
        .collect::<Vec<_>>()
        .as_chunks::<3>()
        .0
        .iter()
        .enumerate()
        .map(|(index, fields)| Stash {
            id: String::from_utf8_lossy(fields[0]).into_owned(),
            date: String::from_utf8_lossy(fields[1]).into_owned(),
            subject: String::from_utf8_lossy(fields[2]).into_owned(),
            selector: format!("stash@{{{index}}}"),
        })
        .collect()
}

impl Git {
    pub async fn stash_detail(&self, root: &Path, id: &str) -> Result<StashDetail> {
        let log = self.run(
            Some(root),
            [
                "log",
                "-1",
                "-z",
                "--date=format-local:%Y-%m-%d %H:%M",
                "--format=%H%x00%h%x00%an%x00%ad%x00%s%x00%P%x00%D",
                id,
                "--",
            ],
        );
        let (output, mut files, body) =
            futures_util::try_join!(log, self.commit_files(root, id), self.commit_body(root, id))?;
        let commit = parse_log(&output)
            .into_iter()
            .next()
            .context(messages().stash_missing())?;
        let untracked_revision = commit.parents.get(2).cloned();
        let mut untracked_paths = std::collections::HashSet::new();
        if let Some(revision) = &untracked_revision {
            for file in self.commit_files(root, revision).await? {
                if !files.iter().any(|tracked| tracked.path == file.path) {
                    untracked_paths.insert(file.path.clone());
                    files.push(file);
                }
            }
        }
        Ok(StashDetail {
            commit,
            files,
            untracked_revision,
            untracked_paths,
            body,
        })
    }
}
