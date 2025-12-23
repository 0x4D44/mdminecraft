use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

/// A 2D grid position used by the deterministic pathfinder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GridPos {
    pub x: i32,
    pub z: i32,
}

impl GridPos {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpenNode {
    /// Total estimated cost.
    f: i32,
    /// Cost so far.
    g: i32,
    pos: GridPos,
}

impl Ord for OpenNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap; invert comparisons so the smallest (f, g, pos) is popped first.
        other
            .f
            .cmp(&self.f)
            .then_with(|| other.g.cmp(&self.g))
            .then_with(|| other.pos.cmp(&self.pos))
    }
}

impl PartialOrd for OpenNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Deterministic 4-neighbor A* on an implicit grid.
///
/// This pathfinder is intended for **simulation use**, so it enforces deterministic tie-breaking:
/// - open-set priority is `(f, g, pos)` where `pos` provides a total ordering
/// - neighbor expansion order is fixed
///
/// `is_walkable` must be deterministic for a given `GridPos`.
pub fn astar_path_4dir(
    start: GridPos,
    goal: GridPos,
    mut is_walkable: impl FnMut(GridPos) -> bool,
    max_expansions: usize,
) -> Option<Vec<GridPos>> {
    if start == goal {
        return Some(vec![start]);
    }
    if !is_walkable(start) || !is_walkable(goal) {
        return None;
    }

    fn heuristic(a: GridPos, b: GridPos) -> i32 {
        (a.x - b.x).abs() + (a.z - b.z).abs()
    }

    const NEIGHBORS: [GridPos; 4] = [
        GridPos { x: -1, z: 0 },
        GridPos { x: 1, z: 0 },
        GridPos { x: 0, z: -1 },
        GridPos { x: 0, z: 1 },
    ];

    let mut open = BinaryHeap::new();
    open.push(OpenNode {
        g: 0,
        f: heuristic(start, goal),
        pos: start,
    });

    let mut came_from: BTreeMap<GridPos, GridPos> = BTreeMap::new();
    let mut g_score: BTreeMap<GridPos, i32> = BTreeMap::new();
    g_score.insert(start, 0);

    let mut closed: BTreeSet<GridPos> = BTreeSet::new();

    let mut expansions = 0usize;
    while let Some(node) = open.pop() {
        if closed.contains(&node.pos) {
            continue;
        }
        closed.insert(node.pos);

        if node.pos == goal {
            let mut path = vec![goal];
            let mut cur = goal;
            while let Some(prev) = came_from.get(&cur).copied() {
                path.push(prev);
                if prev == start {
                    break;
                }
                cur = prev;
            }
            path.reverse();
            return Some(path);
        }

        expansions += 1;
        if expansions > max_expansions {
            return None;
        }

        let current_g = *g_score.get(&node.pos).unwrap_or(&i32::MAX);
        for offset in NEIGHBORS {
            let neighbor = GridPos::new(node.pos.x + offset.x, node.pos.z + offset.z);
            if closed.contains(&neighbor) || !is_walkable(neighbor) {
                continue;
            }

            let tentative_g = current_g.saturating_add(1);
            let best_g = g_score.get(&neighbor).copied().unwrap_or(i32::MAX);
            if tentative_g >= best_g {
                continue;
            }

            came_from.insert(neighbor, node.pos);
            g_score.insert(neighbor, tentative_g);
            open.push(OpenNode {
                g: tentative_g,
                f: tentative_g.saturating_add(heuristic(neighbor, goal)),
                pos: neighbor,
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{astar_path_4dir, GridPos};
    use std::collections::BTreeSet;

    #[test]
    fn astar_returns_straight_path_in_open_space() {
        let start = GridPos::new(0, 0);
        let goal = GridPos::new(3, 0);
        let path = astar_path_4dir(start, goal, |_| true, 1024).expect("path should exist");
        assert_eq!(
            path,
            vec![
                GridPos::new(0, 0),
                GridPos::new(1, 0),
                GridPos::new(2, 0),
                GridPos::new(3, 0),
            ]
        );
    }

    #[test]
    fn astar_tie_breaking_is_deterministic_around_symmetric_obstacle() {
        let start = GridPos::new(0, 0);
        let goal = GridPos::new(2, 0);
        let blocked = BTreeSet::from([GridPos::new(1, 0)]);
        let path = astar_path_4dir(
            start,
            goal,
            |p| !blocked.contains(&p) && (-8..=8).contains(&p.x) && (-8..=8).contains(&p.z),
            4096,
        )
        .expect("path should exist");

        // Two shortest paths exist (detouring north or south); the deterministic ordering chooses one.
        assert_eq!(
            path,
            vec![
                GridPos::new(0, 0),
                GridPos::new(0, -1),
                GridPos::new(1, -1),
                GridPos::new(2, -1),
                GridPos::new(2, 0),
            ]
        );
    }
}
