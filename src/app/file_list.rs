use super::{Context, GitPanel};
use crate::git::ChangedFile;
use std::sync::Arc;

pub(super) struct FileRows {
    pub files: Arc<[ChangedFile]>,
    pub indices: Vec<usize>,
    pub counts: [usize; 2],
    pub header_row: usize,
    collapsed: [bool; 2],
}

impl GitPanel {
    pub(super) fn working_rows(
        &mut self,
        files: &Arc<[ChangedFile]>,
        cx: &Context<Self>,
    ) -> Arc<FileRows> {
        let collapsed = self.collapsed_change_groups;
        if let Some(rows) = &self.file_rows
            && Arc::ptr_eq(files, &rows.files)
            && rows.collapsed == collapsed
        {
            return rows.clone();
        }
        let [mut indices, untracked] =
            grouped_file_indices(files, &self.file_search.read(cx).value());
        let counts = [indices.len(), untracked.len()];
        if collapsed[0] {
            indices.clear();
        }
        let header_row = indices.len();
        if !collapsed[1] {
            indices.extend(untracked);
        }
        let rows = Arc::new(FileRows {
            files: files.clone(),
            indices,
            counts,
            header_row,
            collapsed,
        });
        self.file_rows = Some(rows.clone());
        rows
    }
}

pub(super) fn grouped_file_indices(files: &[ChangedFile], query: &str) -> [Vec<usize>; 2] {
    let query = query.to_lowercase();
    let mut groups = [Vec::new(), Vec::new()];
    for (index, file) in files.iter().enumerate() {
        if query.is_empty() || super::matches_query(&file.display_path(), &query) {
            groups[usize::from(file.status == "??")].push(index);
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substring_matching_preserves_unicode_lowercase_semantics() {
        for text in ["README.md", "中文/ReadMe", "ΣΟΣ", "İSTANBUL", "Key", "Été"] {
            for query in ["", "readme", "中文", "ος", "οσ", "i", "i̇s", "key", "été"] {
                assert_eq!(
                    super::super::matches_query(text, query),
                    text.to_lowercase().contains(query)
                );
            }
        }
    }

    #[test]
    fn filtering_and_grouping_keep_original_file_indices() {
        let files = [
            ChangedFile {
                path: "new/README.md".into(),
                original_path: None,
                status: "??".into(),
            },
            ChangedFile {
                path: "src/main.rs".into(),
                original_path: None,
                status: " M".into(),
            },
            ChangedFile {
                path: "README.md".into(),
                original_path: None,
                status: "M ".into(),
            },
            ChangedFile {
                path: "docs/README.md".into(),
                original_path: None,
                status: "??".into(),
            },
        ];
        assert_eq!(
            grouped_file_indices(&files, "readme"),
            [vec![2], vec![0, 3]]
        );
        assert_eq!(grouped_file_indices(&files, ""), [vec![1, 2], vec![0, 3]]);
        assert!(
            grouped_file_indices(&files, "absent")
                .iter()
                .all(Vec::is_empty)
        );
    }
}
