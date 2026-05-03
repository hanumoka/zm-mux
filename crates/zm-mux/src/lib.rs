pub mod session;
pub mod tabs;

pub use tabs::{Tab, TabId, TabSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaneId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PaneNode {
    Leaf(PaneId),
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

#[derive(Debug)]
pub struct PaneTree {
    root: PaneNode,
}

impl PaneTree {
    pub fn root_node(&self) -> &PaneNode {
        &self.root
    }

    pub fn from_node(root: PaneNode) -> Self {
        Self { root }
    }
}

impl PaneTree {
    pub fn with_initial_pane(initial: PaneId) -> Self {
        Self {
            root: PaneNode::Leaf(initial),
        }
    }

    pub fn root_pane(&self) -> PaneId {
        Self::find_first_leaf(&self.root)
    }

    pub fn split(
        &mut self,
        target: PaneId,
        direction: SplitDirection,
        new_id: PaneId,
    ) -> bool {
        Self::split_node(&mut self.root, target, direction, new_id)
    }

    pub fn remove(&mut self, target: PaneId) -> bool {
        if self.pane_count() <= 1 {
            return false;
        }
        Self::remove_node(&mut self.root, target)
    }

    pub fn layout(&self, width: usize, height: usize) -> Vec<(PaneId, Rect)> {
        let mut result = Vec::new();
        let root_rect = Rect {
            x: 0,
            y: 0,
            width,
            height,
        };
        Self::layout_node(&self.root, root_rect, &mut result);
        result
    }

    pub fn pane_ids(&self) -> Vec<PaneId> {
        let mut ids = Vec::new();
        Self::collect_ids(&self.root, &mut ids);
        ids
    }

    pub fn pane_count(&self) -> usize {
        self.pane_ids().len()
    }

    pub fn find_adjacent(
        &self,
        current: PaneId,
        direction: SplitDirection,
        forward: bool,
        total_width: usize,
        total_height: usize,
    ) -> Option<PaneId> {
        let layouts = self.layout(total_width, total_height);
        let current_rect = layouts.iter().find(|(id, _)| *id == current)?.1;

        let cx = current_rect.x + current_rect.width / 2;
        let cy = current_rect.y + current_rect.height / 2;

        let mut best: Option<(PaneId, usize)> = None;

        for (id, rect) in &layouts {
            if *id == current {
                continue;
            }
            let rx = rect.x + rect.width / 2;
            let ry = rect.y + rect.height / 2;

            let is_candidate = match (direction, forward) {
                (SplitDirection::Horizontal, true) => rx > cx,
                (SplitDirection::Horizontal, false) => rx < cx,
                (SplitDirection::Vertical, true) => ry > cy,
                (SplitDirection::Vertical, false) => ry < cy,
            };

            if !is_candidate {
                continue;
            }

            let dist = cx.abs_diff(rx) + cy.abs_diff(ry);
            if best.is_none() || dist < best.unwrap().1 {
                best = Some((*id, dist));
            }
        }

        best.map(|(id, _)| id)
    }

    fn split_node(
        node: &mut PaneNode,
        target: PaneId,
        direction: SplitDirection,
        new_id: PaneId,
    ) -> bool {
        match node {
            PaneNode::Leaf(id) if *id == target => {
                let old = std::mem::replace(node, PaneNode::Leaf(new_id));
                *node = PaneNode::Split {
                    direction,
                    ratio: 0.5,
                    first: Box::new(old),
                    second: Box::new(PaneNode::Leaf(new_id)),
                };
                true
            }
            PaneNode::Leaf(_) => false,
            PaneNode::Split { first, second, .. } => {
                Self::split_node(first, target, direction, new_id)
                    || Self::split_node(second, target, direction, new_id)
            }
        }
    }

    fn remove_node(node: &mut PaneNode, target: PaneId) -> bool {
        match node {
            PaneNode::Leaf(_) => false,
            PaneNode::Split { first, second, .. } => {
                if matches!(first.as_ref(), PaneNode::Leaf(id) if *id == target) {
                    let replacement = std::mem::replace(second.as_mut(), PaneNode::Leaf(PaneId(0)));
                    *node = replacement;
                    return true;
                }
                if matches!(second.as_ref(), PaneNode::Leaf(id) if *id == target) {
                    let replacement = std::mem::replace(first.as_mut(), PaneNode::Leaf(PaneId(0)));
                    *node = replacement;
                    return true;
                }
                Self::remove_node(first, target) || Self::remove_node(second, target)
            }
        }
    }

    fn layout_node(node: &PaneNode, rect: Rect, result: &mut Vec<(PaneId, Rect)>) {
        const BORDER: usize = 1;

        match node {
            PaneNode::Leaf(id) => {
                result.push((*id, rect));
            }
            PaneNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let (r1, r2) = match direction {
                    SplitDirection::Vertical => {
                        let split = ((rect.height as f32) * ratio) as usize;
                        let split = split.max(1).min(rect.height.saturating_sub(BORDER + 1));
                        (
                            Rect {
                                x: rect.x,
                                y: rect.y,
                                width: rect.width,
                                height: split,
                            },
                            Rect {
                                x: rect.x,
                                y: rect.y + split + BORDER,
                                width: rect.width,
                                height: rect.height.saturating_sub(split + BORDER),
                            },
                        )
                    }
                    SplitDirection::Horizontal => {
                        let split = ((rect.width as f32) * ratio) as usize;
                        let split = split.max(1).min(rect.width.saturating_sub(BORDER + 1));
                        (
                            Rect {
                                x: rect.x,
                                y: rect.y,
                                width: split,
                                height: rect.height,
                            },
                            Rect {
                                x: rect.x + split + BORDER,
                                y: rect.y,
                                width: rect.width.saturating_sub(split + BORDER),
                                height: rect.height,
                            },
                        )
                    }
                };
                Self::layout_node(first, r1, result);
                Self::layout_node(second, r2, result);
            }
        }
    }

    fn collect_ids(node: &PaneNode, ids: &mut Vec<PaneId>) {
        match node {
            PaneNode::Leaf(id) => ids.push(*id),
            PaneNode::Split { first, second, .. } => {
                Self::collect_ids(first, ids);
                Self::collect_ids(second, ids);
            }
        }
    }

    fn find_first_leaf(node: &PaneNode) -> PaneId {
        match node {
            PaneNode::Leaf(id) => *id,
            PaneNode::Split { first, .. } => Self::find_first_leaf(first),
        }
    }

    pub fn border_hit_test(
        &self,
        px_x: usize,
        px_y: usize,
        width: usize,
        height: usize,
        tolerance: usize,
    ) -> Option<BorderHit> {
        let root_rect = Rect { x: 0, y: 0, width, height };
        Self::border_hit_node(&self.root, root_rect, px_x, px_y, tolerance)
    }

    fn border_hit_node(
        node: &PaneNode,
        rect: Rect,
        px_x: usize,
        px_y: usize,
        tolerance: usize,
    ) -> Option<BorderHit> {
        const BORDER: usize = 1;
        match node {
            PaneNode::Leaf(_) => None,
            PaneNode::Split { direction, ratio, first, second } => {
                match direction {
                    SplitDirection::Horizontal => {
                        let split = ((rect.width as f32) * ratio) as usize;
                        let split = split.max(1).min(rect.width.saturating_sub(BORDER + 1));
                        let border_x = rect.x + split;
                        if px_y >= rect.y
                            && px_y < rect.y + rect.height
                            && px_x.abs_diff(border_x) <= tolerance
                        {
                            return Some(BorderHit { direction: *direction, border_pos: border_x });
                        }
                        let r1 = Rect { x: rect.x, y: rect.y, width: split, height: rect.height };
                        let r2 = Rect {
                            x: rect.x + split + BORDER, y: rect.y,
                            width: rect.width.saturating_sub(split + BORDER), height: rect.height,
                        };
                        Self::border_hit_node(first, r1, px_x, px_y, tolerance)
                            .or_else(|| Self::border_hit_node(second, r2, px_x, px_y, tolerance))
                    }
                    SplitDirection::Vertical => {
                        let split = ((rect.height as f32) * ratio) as usize;
                        let split = split.max(1).min(rect.height.saturating_sub(BORDER + 1));
                        let border_y = rect.y + split;
                        if px_x >= rect.x
                            && px_x < rect.x + rect.width
                            && px_y.abs_diff(border_y) <= tolerance
                        {
                            return Some(BorderHit { direction: *direction, border_pos: border_y });
                        }
                        let r1 = Rect { x: rect.x, y: rect.y, width: rect.width, height: split };
                        let r2 = Rect {
                            x: rect.x, y: rect.y + split + BORDER,
                            width: rect.width, height: rect.height.saturating_sub(split + BORDER),
                        };
                        Self::border_hit_node(first, r1, px_x, px_y, tolerance)
                            .or_else(|| Self::border_hit_node(second, r2, px_x, px_y, tolerance))
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn adjust_border(
        &mut self,
        start_x: usize,
        start_y: usize,
        drag_pos: usize,
        width: usize,
        height: usize,
        tolerance: usize,
        min_size: usize,
    ) -> bool {
        let root_rect = Rect { x: 0, y: 0, width, height };
        Self::adjust_border_node(
            &mut self.root, root_rect, start_x, start_y,
            drag_pos, tolerance, min_size,
        )
    }

    fn adjust_border_node(
        node: &mut PaneNode,
        rect: Rect,
        start_x: usize,
        start_y: usize,
        drag_pos: usize,
        tolerance: usize,
        min_size: usize,
    ) -> bool {
        const BORDER: usize = 1;
        match node {
            PaneNode::Leaf(_) => false,
            PaneNode::Split { direction, ratio, first, second } => {
                match direction {
                    SplitDirection::Horizontal => {
                        let split = ((rect.width as f32) * *ratio) as usize;
                        let split = split.max(1).min(rect.width.saturating_sub(BORDER + 1));
                        let border_x = rect.x + split;
                        if start_y >= rect.y
                            && start_y < rect.y + rect.height
                            && start_x.abs_diff(border_x) <= tolerance
                        {
                            let new_split = drag_pos.saturating_sub(rect.x);
                            let dim = rect.width;
                            let min_r = min_size as f32 / dim as f32;
                            let max_r = 1.0 - (min_size + BORDER) as f32 / dim as f32;
                            let new_ratio = (new_split as f32 / dim as f32).clamp(min_r, max_r);
                            *ratio = new_ratio;
                            return true;
                        }
                        let r1 = Rect { x: rect.x, y: rect.y, width: split, height: rect.height };
                        let r2 = Rect {
                            x: rect.x + split + BORDER, y: rect.y,
                            width: rect.width.saturating_sub(split + BORDER), height: rect.height,
                        };
                        Self::adjust_border_node(first, r1, start_x, start_y, drag_pos, tolerance, min_size)
                            || Self::adjust_border_node(second, r2, start_x, start_y, drag_pos, tolerance, min_size)
                    }
                    SplitDirection::Vertical => {
                        let split = ((rect.height as f32) * *ratio) as usize;
                        let split = split.max(1).min(rect.height.saturating_sub(BORDER + 1));
                        let border_y = rect.y + split;
                        if start_x >= rect.x
                            && start_x < rect.x + rect.width
                            && start_y.abs_diff(border_y) <= tolerance
                        {
                            let new_split = drag_pos.saturating_sub(rect.y);
                            let dim = rect.height;
                            let min_r = min_size as f32 / dim as f32;
                            let max_r = 1.0 - (min_size + BORDER) as f32 / dim as f32;
                            let new_ratio = (new_split as f32 / dim as f32).clamp(min_r, max_r);
                            *ratio = new_ratio;
                            return true;
                        }
                        let r1 = Rect { x: rect.x, y: rect.y, width: rect.width, height: split };
                        let r2 = Rect {
                            x: rect.x, y: rect.y + split + BORDER,
                            width: rect.width, height: rect.height.saturating_sub(split + BORDER),
                        };
                        Self::adjust_border_node(first, r1, start_x, start_y, drag_pos, tolerance, min_size)
                            || Self::adjust_border_node(second, r2, start_x, start_y, drag_pos, tolerance, min_size)
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BorderHit {
    pub direction: SplitDirection,
    pub border_pos: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> PaneTree {
        PaneTree::with_initial_pane(PaneId(0))
    }

    #[test]
    fn new_tree_has_one_pane() {
        let tree = fresh();
        assert_eq!(tree.pane_count(), 1);
        assert_eq!(tree.root_pane(), PaneId(0));
    }

    #[test]
    fn split_creates_two_panes() {
        let mut tree = fresh();
        let root = tree.root_pane();
        assert!(tree.split(root, SplitDirection::Horizontal, PaneId(1)));
        assert_eq!(tree.pane_count(), 2);
    }

    #[test]
    fn split_invalid_id_returns_false() {
        let mut tree = fresh();
        assert!(!tree.split(PaneId(999), SplitDirection::Horizontal, PaneId(1)));
        assert_eq!(tree.pane_count(), 1);
    }

    #[test]
    fn remove_pane() {
        let mut tree = fresh();
        let root = tree.root_pane();
        assert!(tree.split(root, SplitDirection::Horizontal, PaneId(1)));
        assert_eq!(tree.pane_count(), 2);

        tree.remove(PaneId(1));
        assert_eq!(tree.pane_count(), 1);
    }

    #[test]
    fn cannot_remove_last_pane() {
        let mut tree = fresh();
        let root = tree.root_pane();
        assert!(!tree.remove(root));
        assert_eq!(tree.pane_count(), 1);
    }

    #[test]
    fn layout_single_pane() {
        let tree = fresh();
        let layouts = tree.layout(800, 600);
        assert_eq!(layouts.len(), 1);
        assert_eq!(layouts[0].1.x, 0);
        assert_eq!(layouts[0].1.y, 0);
        assert_eq!(layouts[0].1.width, 800);
        assert_eq!(layouts[0].1.height, 600);
    }

    #[test]
    fn layout_horizontal_split() {
        let mut tree = fresh();
        let root = tree.root_pane();
        tree.split(root, SplitDirection::Horizontal, PaneId(1));

        let layouts = tree.layout(800, 600);
        assert_eq!(layouts.len(), 2);

        let (_, r1) = &layouts[0];
        let (_, r2) = &layouts[1];

        assert_eq!(r1.y, 0);
        assert_eq!(r2.y, 0);
        assert!(r1.width + r2.width < 800);
        assert_eq!(r1.height, 600);
        assert_eq!(r2.height, 600);
    }

    #[test]
    fn layout_vertical_split() {
        let mut tree = fresh();
        let root = tree.root_pane();
        tree.split(root, SplitDirection::Vertical, PaneId(1));

        let layouts = tree.layout(800, 600);
        assert_eq!(layouts.len(), 2);

        let (_, r1) = &layouts[0];
        let (_, r2) = &layouts[1];

        assert_eq!(r1.x, 0);
        assert_eq!(r2.x, 0);
        assert!(r1.height + r2.height < 600);
        assert_eq!(r1.width, 800);
        assert_eq!(r2.width, 800);
    }

    #[test]
    fn layout_four_panes() {
        let mut tree = fresh();
        let p0 = tree.root_pane();
        tree.split(p0, SplitDirection::Horizontal, PaneId(1));
        tree.split(p0, SplitDirection::Vertical, PaneId(2));
        tree.split(PaneId(1), SplitDirection::Vertical, PaneId(3));

        let layouts = tree.layout(800, 600);
        assert_eq!(layouts.len(), 4);

        for (_, rect) in &layouts {
            assert!(rect.width > 0);
            assert!(rect.height > 0);
        }
    }

    #[test]
    fn find_adjacent_horizontal() {
        let mut tree = fresh();
        let p0 = tree.root_pane();
        tree.split(p0, SplitDirection::Horizontal, PaneId(1));

        let right = tree.find_adjacent(p0, SplitDirection::Horizontal, true, 800, 600);
        assert_eq!(right, Some(PaneId(1)));

        let left = tree.find_adjacent(PaneId(1), SplitDirection::Horizontal, false, 800, 600);
        assert_eq!(left, Some(p0));
    }

    #[test]
    fn border_hit_horizontal_split() {
        let mut tree = fresh();
        let root = tree.root_pane();
        tree.split(root, SplitDirection::Horizontal, PaneId(1));
        let layouts = tree.layout(800, 600);
        let border_x = layouts[0].1.width;
        let hit = tree.border_hit_test(border_x, 300, 800, 600, 4);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().direction, SplitDirection::Horizontal);
    }

    #[test]
    fn border_hit_vertical_split() {
        let mut tree = fresh();
        let root = tree.root_pane();
        tree.split(root, SplitDirection::Vertical, PaneId(1));
        let layouts = tree.layout(800, 600);
        let border_y = layouts[0].1.height;
        let hit = tree.border_hit_test(400, border_y, 800, 600, 4);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().direction, SplitDirection::Vertical);
    }

    #[test]
    fn border_hit_miss_inside_pane() {
        let mut tree = fresh();
        let root = tree.root_pane();
        tree.split(root, SplitDirection::Horizontal, PaneId(1));
        let hit = tree.border_hit_test(100, 300, 800, 600, 4);
        assert!(hit.is_none());
    }

    #[test]
    fn adjust_border_changes_ratio() {
        let mut tree = fresh();
        let root = tree.root_pane();
        tree.split(root, SplitDirection::Horizontal, PaneId(1));
        let layouts = tree.layout(800, 600);
        let border_x = layouts[0].1.width;
        let ok = tree.adjust_border(border_x, 300, 600, 800, 600, 4, 20);
        assert!(ok);
        let new_layouts = tree.layout(800, 600);
        assert!(new_layouts[0].1.width > layouts[0].1.width);
    }

    #[test]
    fn adjust_border_clamps_min_size() {
        let mut tree = fresh();
        let root = tree.root_pane();
        tree.split(root, SplitDirection::Horizontal, PaneId(1));
        let layouts = tree.layout(800, 600);
        let border_x = layouts[0].1.width;
        let ok = tree.adjust_border(border_x, 300, 5, 800, 600, 4, 100);
        assert!(ok);
        let new_layouts = tree.layout(800, 600);
        assert!(new_layouts[0].1.width >= 100);
    }
}
