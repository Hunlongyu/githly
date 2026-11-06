use crate::git::ChangedFile;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ChangeRow {
    Header { untracked: bool, count: usize },
    File(usize),
}

// Only lightweight source indices are flattened here. GPUI creates elements
// for the visible range, including headers, in one shared scroll viewport.
#[cfg(test)]
pub(super) fn change_rows(
    files: &[ChangedFile],
    query: &str,
    collapsed: [bool; 2],
) -> Vec<ChangeRow> {
    scoped_change_rows(files, query, collapsed, crate::git::ChangeScope::All)
}

pub(super) fn scoped_change_rows(
    files: &[ChangedFile],
    query: &str,
    collapsed: [bool; 2],
    scope: crate::git::ChangeScope,
) -> Vec<ChangeRow> {
    let query = query.to_lowercase();
    let mut groups = [Vec::new(), Vec::new()];
    for (index, file) in files.iter().enumerate() {
        if scope.includes(file) && file.path.to_lowercase().contains(&query) {
            groups[usize::from(file.status == "??")].push(index);
        }
    }
    let mut rows = Vec::new();
    for (group, indices) in groups.into_iter().enumerate() {
        rows.push(ChangeRow::Header {
            untracked: group == 1,
            count: indices.len(),
        });
        if !collapsed[group] {
            rows.extend(indices.into_iter().map(ChangeRow::File));
        }
    }
    rows
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
            change_rows(&files, "readme", [false; 2]),
            vec![
                ChangeRow::Header {
                    untracked: false,
                    count: 1
                },
                ChangeRow::File(2),
                ChangeRow::Header {
                    untracked: true,
                    count: 2
                },
                ChangeRow::File(0),
                ChangeRow::File(3),
            ]
        );
        assert_eq!(
            change_rows(&files, "readme", [true, false]),
            vec![
                ChangeRow::Header {
                    untracked: false,
                    count: 1
                },
                ChangeRow::Header {
                    untracked: true,
                    count: 2
                },
                ChangeRow::File(0),
                ChangeRow::File(3),
            ]
        );
        assert_eq!(
            change_rows(&files, "absent", [true; 2]),
            vec![
                ChangeRow::Header {
                    untracked: false,
                    count: 0
                },
                ChangeRow::Header {
                    untracked: true,
                    count: 0
                },
            ]
        );
    }
}
