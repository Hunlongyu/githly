use crate::git::Commit;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraphRow {
    pub node: usize,
    pub incoming: usize,
    pub edges: Vec<(usize, usize)>,
    pub width: usize,
}

/// Topological rows keep the active parent lanes across virtualized list items.
#[cfg(test)]
pub fn layout(commits: &[Commit]) -> Vec<GraphRow> {
    extend(commits, &mut Vec::new())
}

pub fn extend<'a>(
    commits: impl IntoIterator<Item = &'a Commit>,
    lanes: &mut Vec<String>,
) -> Vec<GraphRow> {
    // Borrow IDs from the input for this batch. Only surviving lanes need to
    // own their IDs when the next page of history is loaded.
    let mut active: Vec<&str> = lanes.iter().map(String::as_str).collect();
    let mut parent_columns = Vec::new();
    let rows = commits
        .into_iter()
        .map(|commit| {
            let incoming = active.len();
            let node = active
                .iter()
                .position(|id| *id == commit.id)
                .unwrap_or(incoming);
            let before = incoming.max(node + 1);
            // The other lanes keep their relative order. Only the columns after
            // the node shift when we replace it with its previously unseen parents.
            parent_columns.clear();
            parent_columns.resize(commit.parents.len(), None);
            if let [parent] = commit.parents.as_slice() {
                parent_columns[0] = active.iter().position(|id| *id == parent);
            } else if !commit.parents.is_empty() {
                let positions: std::collections::HashMap<_, _> = active
                    .iter()
                    .enumerate()
                    .map(|(column, &id)| (id, column))
                    .collect();
                for (column, parent) in parent_columns.iter_mut().zip(&commit.parents) {
                    *column = positions.get(parent.as_str()).copied();
                }
            }
            let added = parent_columns
                .iter()
                .filter(|column| column.is_none())
                .count();
            let shifted = |column| {
                if column > node {
                    column - 1 + added
                } else {
                    column
                }
            };
            let mut edges = Vec::with_capacity(before - 1 + commit.parents.len());
            for column in (0..incoming).filter(|&column| column != node) {
                edges.push((column, shifted(column)));
            }
            let new_parents = commit
                .parents
                .iter()
                .zip(&parent_columns)
                .filter(|(_, column)| column.is_none())
                .map(|(id, _)| id.as_str());
            active.splice(node..(node + 1).min(incoming), new_parents);
            let mut next_column = node;
            for &column in &parent_columns {
                let to = column.map(shifted).unwrap_or_else(|| {
                    next_column += 1;
                    next_column - 1
                });
                edges.push((node, to));
            }
            GraphRow {
                node,
                incoming,
                width: before.max(active.len()),
                edges,
            }
        })
        .collect();
    *lanes = active.into_iter().map(str::to_owned).collect();
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    fn commit(id: &str, parents: &[&str]) -> Commit {
        Commit {
            id: id.into(),
            parents: parents.iter().map(|s| (*s).into()).collect(),
            short_id: id.into(),
            subject: String::new(),
            author: String::new(),
            date: String::new(),
            references: Vec::new(),
        }
    }
    #[test]
    fn octopus_merge_shifts_surviving_lanes_and_keeps_parent_order() {
        let mut lanes = vec!["a".into(), "merge".into(), "b".into(), "c".into()];
        let rows = extend(&[commit("merge", &["c", "new", "a", "other"])], &mut lanes);
        assert_eq!(lanes, ["a", "new", "other", "b", "c"]);
        assert_eq!(
            rows[0],
            GraphRow {
                node: 1,
                incoming: 4,
                width: 5,
                edges: vec![(0, 0), (2, 3), (3, 4), (1, 4), (1, 1), (1, 0), (1, 2)]
            }
        );
    }

    #[test]
    fn merge_lanes_converge_on_common_parent() {
        let rows = layout(&[
            commit("merge", &["a", "b"]),
            commit("a", &["root"]),
            commit("b", &["root"]),
            commit("root", &[]),
        ]);
        assert_eq!(rows[0].edges, [(0, 0), (0, 1)]);
        assert!(rows[2].edges.contains(&(1, 0)));
        assert_eq!(rows[3].incoming, 1);
        assert!(rows[3].edges.is_empty());
    }
}
