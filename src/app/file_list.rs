use crate::git::{ChangeScope, ChangedFile};

pub(super) fn grouped_file_indices(
    files: &[ChangedFile],
    query: &str,
    scope: ChangeScope,
) -> [Vec<usize>; 2] {
    let query = query.to_lowercase();
    let mut groups = [Vec::new(), Vec::new()];
    for (index, file) in files.iter().enumerate() {
        if scope.includes(file) && (query.is_empty() || file.path.to_lowercase().contains(&query)) {
            groups[usize::from(file.status == "??")].push(index);
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

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
            grouped_file_indices(&files, "readme", ChangeScope::All),
            [vec![2], vec![0, 3]]
        );
        assert_eq!(
            grouped_file_indices(&files, "readme", ChangeScope::Staged),
            [vec![2], vec![]]
        );
        assert!(
            grouped_file_indices(&files, "absent", ChangeScope::All)
                .iter()
                .all(Vec::is_empty)
        );
    }
}
