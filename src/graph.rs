use std::collections::HashSet;

use crate::git::Commit;

#[derive(Clone, Debug, Default)]
pub struct GraphLayout {
    pub rows: Vec<GraphRow>,
    pub lanes: usize,
}

#[derive(Clone, Debug, Default)]
pub struct GraphRow {
    pub lane: usize,
    pub before_lanes: Vec<usize>,
    pub after_lanes: Vec<usize>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Clone, Debug)]
pub struct GraphEdge {
    pub from_lane: usize,
    pub to_lane: usize,
    pub kind: EdgeKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EdgeKind {
    Continue,
    Branch,
    Merge,
}

pub fn layout(commits: &[Commit]) -> GraphLayout {
    let known_hashes = commits
        .iter()
        .map(|commit| commit.hash.as_str())
        .collect::<HashSet<_>>();
    let mut active: Vec<Option<String>> = Vec::new();
    let mut rows = Vec::with_capacity(commits.len());
    let mut max_lanes = 1;

    for commit in commits {
        let lane = if let Some(index) = active
            .iter()
            .position(|hash| hash.as_deref() == Some(commit.hash.as_str()))
        {
            index
        } else {
            first_free_lane(&mut active)
        };
        let before_lanes = active
            .iter()
            .enumerate()
            .filter_map(|(index, hash)| hash.is_some().then_some(index))
            .collect::<Vec<_>>();

        let visible_parents = commit
            .parents
            .iter()
            .filter(|parent| known_hashes.contains(parent.as_str()))
            .cloned()
            .collect::<Vec<_>>();

        let mut parent_lanes = Vec::new();
        if visible_parents.is_empty() {
            active[lane] = None;
        } else {
            active[lane] = Some(visible_parents[0].clone());
            parent_lanes.push(lane);

            for parent in visible_parents.iter().skip(1) {
                let parent_lane = find_or_allocate_lane(&mut active, parent);
                parent_lanes.push(parent_lane);
            }
        }

        let edges = parent_lanes
            .into_iter()
            .enumerate()
            .map(|(index, to_lane)| GraphEdge {
                from_lane: lane,
                to_lane,
                kind: if index == 0 {
                    EdgeKind::Continue
                } else if to_lane < lane {
                    EdgeKind::Merge
                } else {
                    EdgeKind::Branch
                },
            })
            .collect();

        max_lanes = max_lanes.max(active.len());
        let mut after_lanes = active
            .iter()
            .enumerate()
            .filter_map(|(index, hash)| hash.is_some().then_some(index))
            .collect::<Vec<_>>();
        after_lanes.sort_unstable();
        rows.push(GraphRow {
            lane,
            before_lanes,
            after_lanes,
            edges,
        });
        trim_trailing_free_lanes(&mut active);
    }

    GraphLayout {
        rows,
        lanes: max_lanes,
    }
}

fn first_free_lane(active: &mut Vec<Option<String>>) -> usize {
    if let Some(index) = active.iter().position(Option::is_none) {
        index
    } else {
        active.push(None);
        active.len() - 1
    }
}

fn find_or_allocate_lane(active: &mut Vec<Option<String>>, hash: &str) -> usize {
    if let Some(index) = active
        .iter()
        .position(|candidate| candidate.as_deref() == Some(hash))
    {
        return index;
    }

    let index = first_free_lane(active);
    active[index] = Some(hash.to_owned());
    index
}

fn trim_trailing_free_lanes(active: &mut Vec<Option<String>>) {
    while active.last().is_some_and(Option::is_none) {
        active.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(hash: &str, parents: &[&str]) -> Commit {
        Commit {
            hash: hash.to_owned(),
            short_hash: hash.to_owned(),
            parents: parents.iter().map(|parent| parent.to_string()).collect(),
            author: "Ada".to_owned(),
            author_email: "ada@example.com".to_owned(),
            date: "2026-06-23 09:51".to_owned(),
            relative_time: "now".to_owned(),
            subject: hash.to_owned(),
            refs: Vec::new(),
        }
    }

    #[test]
    fn lays_out_linear_history_in_one_lane() {
        let graph = layout(&[commit("c", &["b"]), commit("b", &["a"]), commit("a", &[])]);

        assert_eq!(graph.lanes, 1);
        assert_eq!(
            graph.rows.iter().map(|row| row.lane).collect::<Vec<_>>(),
            vec![0, 0, 0]
        );
    }

    #[test]
    fn keeps_merge_parent_on_separate_lane() {
        let graph = layout(&[
            commit("m", &["b", "f"]),
            commit("b", &["a"]),
            commit("f", &["e"]),
            commit("e", &["a"]),
            commit("a", &[]),
        ]);

        assert!(graph.lanes >= 2);
        assert_eq!(graph.rows[0].edges.len(), 2);
        assert!(
            graph.rows[0]
                .edges
                .iter()
                .any(|edge| edge.kind != EdgeKind::Continue)
        );
        assert!(graph.rows[1].before_lanes.contains(&0));
        assert!(graph.rows[1].after_lanes.contains(&0));
        assert!(graph.rows[2].before_lanes.contains(&1));
        assert!(graph.rows[2].after_lanes.contains(&1));
    }

    #[test]
    fn separates_lane_entry_and_exit_segments() {
        let graph = layout(&[
            commit("m", &["b", "f"]),
            commit("b", &["a"]),
            commit("f", &["e"]),
            commit("e", &["a"]),
            commit("a", &[]),
        ]);

        assert!(graph.rows[0].before_lanes.is_empty());
        assert!(graph.rows[0].after_lanes.contains(&0));
        assert!(graph.rows[0].after_lanes.contains(&1));
        assert!(!graph.rows[0].before_lanes.contains(&1));
        assert!(!graph.rows[4].after_lanes.contains(&0));
    }

    #[test]
    fn lays_out_large_history_with_periodic_merges() {
        let commits = (0..5_000)
            .map(|index| {
                let hash = format!("c{index}");
                let mut parents = Vec::new();
                if index + 1 < 5_000 {
                    parents.push(format!("c{}", index + 1));
                }
                if index % 50 == 0 && index + 25 < 5_000 {
                    parents.push(format!("c{}", index + 25));
                }

                Commit {
                    hash: hash.clone(),
                    short_hash: hash,
                    parents,
                    author: "Ada".to_owned(),
                    author_email: "ada@example.com".to_owned(),
                    date: "2026-06-23 09:51".to_owned(),
                    relative_time: "now".to_owned(),
                    subject: format!("commit {index}"),
                    refs: Vec::new(),
                }
            })
            .collect::<Vec<_>>();

        let graph = layout(&commits);

        assert_eq!(graph.rows.len(), commits.len());
        assert!(graph.lanes >= 2);
        assert!(graph.rows.iter().any(|row| row.edges.len() > 1));
    }
}
