//! zm-mux — 멀티플렉서 코어(순수 모델): pane 분할 트리 + 탭 + 포커스 + 기하 계산.
//!
//! OS/UI 비의존. Term/PTY/스레드 같은 런타임 자원은 보유하지 않고 **PaneId**만 다룬다.
//! zm-app이 PaneId ↔ 실제 자원(PTY+Term+리더 스레드)을 매핑한다.
//!
//! 분할 방향:
//!  - `Orientation::LeftRight` : 세로 분할선 → 좌/우 pane
//!  - `Orientation::TopBottom` : 가로 분할선 → 상/하 pane

#![allow(clippy::needless_range_loop)]

pub type PaneId = u64;

/// 분할 방향.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    LeftRight,
    TopBottom,
}

/// 픽셀 사각형.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// 분할 트리 노드.
#[derive(Debug, Clone)]
enum Node {
    Leaf(PaneId),
    Split {
        orient: Orientation,
        ratio: f32, // 첫 자식이 차지하는 비율
        first: Box<Node>,
        second: Box<Node>,
    },
}

impl Node {
    fn first_leaf(&self) -> PaneId {
        match self {
            Node::Leaf(id) => *id,
            Node::Split { first, .. } => first.first_leaf(),
        }
    }

    fn collect_leaves(&self, out: &mut Vec<PaneId>) {
        match self {
            Node::Leaf(id) => out.push(*id),
            Node::Split { first, second, .. } => {
                first.collect_leaves(out);
                second.collect_leaves(out);
            }
        }
    }

    /// active 리프를 split으로 교체(split_active).
    fn split_leaf(&mut self, target: PaneId, orient: Orientation, new_id: PaneId) -> bool {
        match self {
            Node::Leaf(id) if *id == target => {
                let old = *id;
                *self = Node::Split {
                    orient,
                    ratio: 0.5,
                    first: Box::new(Node::Leaf(old)),
                    second: Box::new(Node::Leaf(new_id)),
                };
                true
            }
            Node::Leaf(_) => false,
            Node::Split { first, second, .. } => {
                first.split_leaf(target, orient, new_id)
                    || second.split_leaf(target, orient, new_id)
            }
        }
    }
}

/// 리프 제거 결과.
enum Rm {
    IsTarget,
    Kept(Node),
}

fn remove_leaf(node: Node, target: PaneId) -> Rm {
    match node {
        Node::Leaf(id) if id == target => Rm::IsTarget,
        Node::Leaf(id) => Rm::Kept(Node::Leaf(id)),
        Node::Split {
            orient,
            ratio,
            first,
            second,
        } => match remove_leaf(*first, target) {
            Rm::IsTarget => Rm::Kept(*second),
            Rm::Kept(nf) => match remove_leaf(*second, target) {
                Rm::IsTarget => Rm::Kept(nf),
                Rm::Kept(ns) => Rm::Kept(Node::Split {
                    orient,
                    ratio,
                    first: Box::new(nf),
                    second: Box::new(ns),
                }),
            },
        },
    }
}

fn layout(node: &Node, rect: Rect, border: f32, out: &mut Vec<PaneGeom>) {
    match node {
        Node::Leaf(id) => out.push(PaneGeom { id: *id, rect }),
        Node::Split {
            orient,
            ratio,
            first,
            second,
        } => match orient {
            Orientation::LeftRight => {
                let avail = (rect.w - border).max(0.0);
                let fw = (avail * ratio).floor().max(0.0);
                let sw = (avail - fw).max(0.0);
                layout(
                    first,
                    Rect { x: rect.x, y: rect.y, w: fw, h: rect.h },
                    border,
                    out,
                );
                layout(
                    second,
                    Rect { x: rect.x + fw + border, y: rect.y, w: sw, h: rect.h },
                    border,
                    out,
                );
            }
            Orientation::TopBottom => {
                let avail = (rect.h - border).max(0.0);
                let fh = (avail * ratio).floor().max(0.0);
                let sh = (avail - fh).max(0.0);
                layout(
                    first,
                    Rect { x: rect.x, y: rect.y, w: rect.w, h: fh },
                    border,
                    out,
                );
                layout(
                    second,
                    Rect { x: rect.x, y: rect.y + fh + border, w: rect.w, h: sh },
                    border,
                    out,
                );
            }
        },
    }
}

/// pane 한 개의 기하(픽셀 사각형).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaneGeom {
    pub id: PaneId,
    pub rect: Rect,
}

struct Tab {
    root: Node,
    active: PaneId,
    /// 줌된 pane(전체화면). Some이면 그 pane만 렌더.
    zoomed: Option<PaneId>,
}

/// 멀티플렉서 모델.
pub struct Mux {
    tabs: Vec<Tab>,
    active_tab: usize,
    next_id: PaneId,
}

/// close 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseResult {
    /// pane만 닫힘(닫힌 id). 같은 탭의 다른 pane으로 포커스 이동.
    Pane(PaneId),
    /// 탭 전체가 닫힘(닫힌 pane id). 다른 탭으로 전환됨.
    Tab(PaneId),
    /// 마지막 pane → 앱 종료 신호(닫힌 pane id).
    LastPane(PaneId),
}

impl Mux {
    /// 첫 탭 + 첫 pane(id=1) 생성. (Mux, 첫 PaneId).
    pub fn new() -> (Self, PaneId) {
        let first = 1;
        let mux = Self {
            tabs: vec![Tab {
                root: Node::Leaf(first),
                active: first,
                zoomed: None,
            }],
            active_tab: 0,
            next_id: first + 1,
        };
        (mux, first)
    }

    fn tab(&self) -> &Tab {
        &self.tabs[self.active_tab]
    }
    fn tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active_tab]
    }

    pub fn active_pane(&self) -> PaneId {
        self.tab().active
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn active_tab_index(&self) -> usize {
        self.active_tab
    }

    /// 활성 탭의 모든 pane id(렌더 대상).
    pub fn active_tab_panes(&self) -> Vec<PaneId> {
        let mut v = Vec::new();
        self.tab().root.collect_leaves(&mut v);
        v
    }

    /// 모든 탭의 모든 pane id(자원 정리용).
    pub fn all_panes(&self) -> Vec<PaneId> {
        let mut v = Vec::new();
        for t in &self.tabs {
            t.root.collect_leaves(&mut v);
        }
        v
    }

    /// 활성 pane을 분할하고 새 pane을 활성화. 새 PaneId 반환.
    pub fn split_active(&mut self, orient: Orientation) -> PaneId {
        let new_id = self.next_id;
        self.next_id += 1;
        let active = self.tab().active;
        let t = self.tab_mut();
        t.root.split_leaf(active, orient, new_id);
        t.active = new_id;
        t.zoomed = None; // 구조 변경 시 줌 해제
        new_id
    }

    /// 활성 pane 줌 토글(전체화면). 줌 중이면 해제.
    pub fn toggle_zoom(&mut self) {
        let active = self.tab().active;
        let t = self.tab_mut();
        t.zoomed = if t.zoomed == Some(active) {
            None
        } else {
            Some(active)
        };
    }

    pub fn is_zoomed(&self) -> bool {
        self.tab().zoomed.is_some()
    }

    /// 새 탭(+새 pane) 생성 후 전환. 새 PaneId 반환.
    pub fn new_tab(&mut self) -> PaneId {
        let new_id = self.next_id;
        self.next_id += 1;
        self.tabs.push(Tab {
            root: Node::Leaf(new_id),
            active: new_id,
            zoomed: None,
        });
        self.active_tab = self.tabs.len() - 1;
        new_id
    }

    pub fn next_tab(&mut self) {
        if self.tabs.len() > 1 {
            self.active_tab = (self.active_tab + 1) % self.tabs.len();
        }
    }

    pub fn prev_tab(&mut self) {
        if self.tabs.len() > 1 {
            self.active_tab = (self.active_tab + self.tabs.len() - 1) % self.tabs.len();
        }
    }

    /// 활성 탭 내 다음 pane으로 포커스 순환.
    pub fn focus_next(&mut self) {
        let leaves = self.active_tab_panes();
        if leaves.len() < 2 {
            return;
        }
        let cur = self.tab().active;
        let idx = leaves.iter().position(|&p| p == cur).unwrap_or(0);
        let next = leaves[(idx + 1) % leaves.len()];
        self.tab_mut().active = next;
    }

    /// 특정 pane으로 포커스(클릭 등).
    pub fn focus(&mut self, id: PaneId) {
        if self.active_tab_panes().contains(&id) {
            self.tab_mut().active = id;
        }
    }

    /// 활성 pane을 닫는다.
    pub fn close_active(&mut self) -> CloseResult {
        self.close_pane(self.active_pane())
    }

    /// 특정 pane을 닫는다(어느 탭이든). 셸 종료(PtyExited) 처리에 사용.
    pub fn close_pane(&mut self, id: PaneId) -> CloseResult {
        // id가 속한 탭 찾기.
        let tab_idx = self.tabs.iter().position(|t| {
            let mut v = Vec::new();
            t.root.collect_leaves(&mut v);
            v.contains(&id)
        });
        let Some(ti) = tab_idx else {
            return CloseResult::Pane(id); // 알 수 없는 id → no-op
        };

        let root = std::mem::replace(&mut self.tabs[ti].root, Node::Leaf(0));
        match remove_leaf(root, id) {
            Rm::IsTarget => {
                // 해당 탭의 유일 pane → 탭 제거.
                self.tabs.remove(ti);
                if self.tabs.is_empty() {
                    return CloseResult::LastPane(id);
                }
                if self.active_tab > ti {
                    self.active_tab -= 1;
                } else if self.active_tab == ti && self.active_tab >= self.tabs.len() {
                    self.active_tab = self.tabs.len() - 1;
                }
                CloseResult::Tab(id)
            }
            Rm::Kept(new_root) => {
                let new_active = new_root.first_leaf();
                let t = &mut self.tabs[ti];
                t.root = new_root;
                if t.active == id {
                    t.active = new_active;
                }
                t.zoomed = None;
                CloseResult::Pane(id)
            }
        }
    }

    /// 활성 탭 전체를 닫는다. (닫힌 pane id들, 마지막 탭이었는지=앱 종료).
    pub fn close_tab(&mut self) -> (Vec<PaneId>, bool) {
        let ids = self.active_tab_panes();
        self.tabs.remove(self.active_tab);
        let last = self.tabs.is_empty();
        if !last && self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
        (ids, last)
    }

    /// 활성 탭의 pane 기하 계산(픽셀). `border`는 분할 사이 간격.
    pub fn compute(&self, window: Rect, border: f32) -> Vec<PaneGeom> {
        // 줌 중이고 그 pane이 활성 탭에 있으면 그것만 전체화면.
        if let Some(z) = self.tab().zoomed {
            if self.active_tab_panes().contains(&z) {
                return vec![PaneGeom { id: z, rect: window }];
            }
        }
        let mut out = Vec::new();
        layout(&self.tab().root, window, border, &mut out);
        out
    }

    /// `point` 근처의 분할 divider를 `delta`만큼 이동(드래그 리사이즈). 조정되면 true.
    pub fn resize_split(
        &mut self,
        window: Rect,
        border: f32,
        point: (f32, f32),
        delta: (f32, f32),
    ) -> bool {
        let root = &mut self.tabs[self.active_tab].root;
        adjust_split(root, window, border, point, delta)
    }
}

fn adjust_split(node: &mut Node, rect: Rect, border: f32, p: (f32, f32), d: (f32, f32)) -> bool {
    let Node::Split {
        orient,
        ratio,
        first,
        second,
    } = node
    else {
        return false;
    };
    match orient {
        Orientation::LeftRight => {
            let avail = (rect.w - border).max(1.0);
            let fw = (avail * *ratio).floor().max(0.0);
            let dv0 = rect.x + fw;
            let on_div = p.0 >= dv0 - 4.0
                && p.0 <= dv0 + border + 4.0
                && p.1 >= rect.y
                && p.1 <= rect.y + rect.h;
            if on_div {
                *ratio = ((fw + d.0) / avail).clamp(0.05, 0.95);
                return true;
            }
            let fr = Rect { x: rect.x, y: rect.y, w: fw, h: rect.h };
            let sr = Rect {
                x: rect.x + fw + border,
                y: rect.y,
                w: (avail - fw).max(0.0),
                h: rect.h,
            };
            if p.0 < dv0 {
                adjust_split(first, fr, border, p, d)
            } else {
                adjust_split(second, sr, border, p, d)
            }
        }
        Orientation::TopBottom => {
            let avail = (rect.h - border).max(1.0);
            let fh = (avail * *ratio).floor().max(0.0);
            let dv0 = rect.y + fh;
            let on_div = p.1 >= dv0 - 4.0
                && p.1 <= dv0 + border + 4.0
                && p.0 >= rect.x
                && p.0 <= rect.x + rect.w;
            if on_div {
                *ratio = ((fh + d.1) / avail).clamp(0.05, 0.95);
                return true;
            }
            let fr = Rect { x: rect.x, y: rect.y, w: rect.w, h: fh };
            let sr = Rect {
                x: rect.x,
                y: rect.y + fh + border,
                w: rect.w,
                h: (avail - fh).max(0.0),
            };
            if p.1 < dv0 {
                adjust_split(first, fr, border, p, d)
            } else {
                adjust_split(second, sr, border, p, d)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(geoms: &[PaneGeom]) -> Vec<PaneId> {
        geoms.iter().map(|g| g.id).collect()
    }

    #[test]
    fn new_has_one_pane() {
        let (mux, first) = Mux::new();
        assert_eq!(first, 1);
        assert_eq!(mux.active_pane(), 1);
        assert_eq!(mux.active_tab_panes(), vec![1]);
    }

    #[test]
    fn split_and_geometry() {
        let (mut mux, _) = Mux::new();
        let p2 = mux.split_active(Orientation::LeftRight);
        assert_eq!(p2, 2);
        assert_eq!(mux.active_pane(), 2);
        let win = Rect { x: 0.0, y: 0.0, w: 100.0, h: 50.0 };
        let g = mux.compute(win, 0.0);
        assert_eq!(ids(&g), vec![1, 2]);
        // 좌/우 분할: 폭 절반.
        assert_eq!(g[0].rect.w, 50.0);
        assert_eq!(g[1].rect.x, 50.0);
        assert_eq!(g[1].rect.h, 50.0);
    }

    #[test]
    fn close_collapses_split() {
        let (mut mux, _) = Mux::new();
        mux.split_active(Orientation::TopBottom); // p2 active
        let r = mux.close_active();
        assert_eq!(r, CloseResult::Pane(2));
        assert_eq!(mux.active_pane(), 1);
        assert_eq!(mux.active_tab_panes(), vec![1]);
    }

    #[test]
    fn close_last_pane_signals_exit() {
        let (mut mux, _) = Mux::new();
        let r = mux.close_active();
        assert_eq!(r, CloseResult::LastPane(1));
    }

    #[test]
    fn resize_split_adjusts_ratio() {
        let (mut mux, _) = Mux::new();
        mux.split_active(Orientation::LeftRight); // 좌/우, 비율 0.5
        let win = Rect { x: 0.0, y: 0.0, w: 100.0, h: 50.0 };
        // divider는 border=0일 때 x=50 근처. +10 드래그 → 첫 pane 폭 증가.
        let g0 = mux.compute(win, 0.0);
        assert_eq!(g0[0].rect.w, 50.0);
        let ok = mux.resize_split(win, 0.0, (50.0, 25.0), (10.0, 0.0));
        assert!(ok);
        let g1 = mux.compute(win, 0.0);
        assert_eq!(g1[0].rect.w, 60.0);
        assert_eq!(g1[1].rect.x, 60.0);
    }

    #[test]
    fn zoom_shows_single_pane() {
        let (mut mux, _) = Mux::new();
        mux.split_active(Orientation::LeftRight); // 2 panes, active=2
        mux.toggle_zoom();
        assert!(mux.is_zoomed());
        let win = Rect { x: 0.0, y: 0.0, w: 100.0, h: 50.0 };
        let g = mux.compute(win, 0.0);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].id, 2);
        assert_eq!(g[0].rect.w, 100.0);
        mux.toggle_zoom();
        assert!(!mux.is_zoomed());
        assert_eq!(mux.compute(win, 0.0).len(), 2);
    }

    #[test]
    fn tabs_and_focus() {
        let (mut mux, _) = Mux::new();
        mux.split_active(Orientation::LeftRight); // tab0: panes 1,2 active=2
        let p3 = mux.new_tab(); // tab1: pane 3
        assert_eq!(p3, 3);
        assert_eq!(mux.tab_count(), 2);
        assert_eq!(mux.active_pane(), 3);
        mux.prev_tab();
        assert_eq!(mux.active_tab_index(), 0);
        assert_eq!(mux.active_pane(), 2);
        mux.focus_next();
        assert_eq!(mux.active_pane(), 1);
        mux.focus_next();
        assert_eq!(mux.active_pane(), 2);
        // 모든 pane.
        let mut all = mux.all_panes();
        all.sort();
        assert_eq!(all, vec![1, 2, 3]);
    }
}
