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

pub fn extend(commits: &[Commit], lanes: &mut Vec<String>) -> Vec<GraphRow> {
    commits
        .iter()
        .map(|commit| {
            let incoming = lanes.len();
            let node = lanes
                .iter()
                .position(|id| id == &commit.id)
                .unwrap_or_else(|| {
                    lanes.push(commit.id.clone());
                    lanes.len() - 1
                });
            let before = lanes.clone();
            lanes.remove(node);
            for (offset, parent) in commit
                .parents
                .iter()
                .filter(|parent| !before.contains(parent))
                .enumerate()
            {
                lanes.insert((node + offset).min(lanes.len()), parent.clone());
            }
            let mut edges = Vec::new();
            for (column, id) in before.iter().enumerate() {
                if column != node
                    && let Some(to) = lanes.iter().position(|p| p == id)
                {
                    edges.push((column, to));
                }
            }
            for parent in &commit.parents {
                if let Some(to) = lanes.iter().position(|p| p == parent) {
                    edges.push((node, to));
                }
            }
            GraphRow {
                node,
                incoming,
                width: before.len().max(lanes.len()),
                edges,
            }
        })
        .collect()
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
