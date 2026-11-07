use super::controls;
use crate::git::{Branch, HistoryScope};
use crate::i18n::messages;
use gpui_kit::component::{
    ActiveTheme, Icon, IconName, StyledExt, h_flex,
    input::{InputEvent, InputState},
    scroll::{Scrollbar, ScrollbarMode},
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::sync::Arc;

const ROW_HEIGHT: f32 = 30.;
const MAX_LIST_HEIGHT: f32 = 420.;

pub(super) enum BranchEvent {
    Browse(HistoryScope),
    Switch(String),
}
impl EventEmitter<BranchEvent> for BranchPicker {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BranchRow {
    Group { remote: bool, count: usize },
    Branch(usize),
    Empty,
}

fn branch_rows(branches: &[Branch], query: &str, collapsed: [bool; 2]) -> Vec<BranchRow> {
    let query = query.trim().to_lowercase();
    let mut rows = Vec::new();
    let mut total_matches = 0;
    for remote in [false, true] {
        let matching: Vec<_> = branches
            .iter()
            .enumerate()
            .filter(|(_, branch)| {
                branch.remote == remote && super::matches_query(&branch.name, &query)
            })
            .map(|(index, _)| index)
            .collect();
        total_matches += matching.len();
        rows.push(BranchRow::Group {
            remote,
            count: matching.len(),
        });
        if !collapsed[usize::from(remote)] {
            rows.extend(matching.into_iter().map(BranchRow::Branch));
        }
    }
    if total_matches == 0 {
        rows.push(BranchRow::Empty);
    }
    rows
}

pub(super) struct BranchPicker {
    branches: Arc<[Branch]>,
    search: Entity<InputState>,
    collapsed: [bool; 2],
    rows: Vec<BranchRow>,
    scroll: UniformListScrollHandle,
}

impl BranchPicker {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder(messages().search_branches()));
        cx.subscribe(&search, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.filter(cx);
            }
        })
        .detach();
        Self {
            branches: Arc::from([]),
            search,
            collapsed: [false; 2],
            rows: Vec::new(),
            scroll: UniformListScrollHandle::new(),
        }
    }

    pub(super) fn set_branches(
        &mut self,
        branches: impl Into<Arc<[Branch]>>,
        cx: &mut Context<Self>,
    ) {
        let offset = self.scroll.0.borrow().base_handle.offset();
        let branches = branches.into();
        self.branches = branches;
        self.filter(cx);
        self.scroll.0.borrow_mut().deferred_scroll_to_item = None;
        self.scroll.0.borrow().base_handle.set_offset(offset);
    }

    pub(super) fn clear(&mut self, cx: &mut Context<Self>) {
        self.branches = Arc::from([]);
        self.rows = Vec::new();
        self.scroll = UniformListScrollHandle::new();
        cx.notify();
    }

    pub(super) fn update_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |input, cx| {
            input.set_placeholder(messages().search_branches(), window, cx)
        });
        cx.notify();
    }

    pub(super) fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, |search, cx| {
            search.set_value("", window, cx);
            search.focus(window, cx);
        });
        self.filter(cx);
    }

    fn filter(&mut self, cx: &mut Context<Self>) {
        self.rows = branch_rows(
            &self.branches,
            &self.search.read(cx).value(),
            self.collapsed,
        );
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    fn row(&self, row: BranchRow, cx: &mut Context<Self>) -> AnyElement {
        match row {
            BranchRow::Group { remote, count } => {
                let group = usize::from(remote);
                let collapsed = self.collapsed[group];
                let name = if remote {
                    messages().remote_branches()
                } else {
                    messages().local_branches()
                };
                gpui_kit::base::Button::new(("branch-group", group))
                    .w_full()
                    .h(px(ROW_HEIGHT))
                    .flex_shrink_0()
                    .px(px(8.))
                    .gap(px(8.))
                    .items_center()
                    .justify_start()
                    .text_size(px(12.))
                    .font_semibold()
                    .text_color(cx.theme().muted_foreground)
                    .hover(|style| style.bg(cx.theme().list_hover))
                    .accessibility_label(messages().branch_group(
                        name,
                        count,
                        if collapsed {
                            messages().collapsed()
                        } else {
                            messages().expanded()
                        },
                    ))
                    .child(
                        Icon::new(if collapsed {
                            IconName::ChevronRight
                        } else {
                            IconName::ChevronDown
                        })
                        .size(px(12.)),
                    )
                    .child(name)
                    .child(div().font_normal().child(count.to_string()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.collapsed[group] = !this.collapsed[group];
                        this.filter(cx);
                    }))
                    .into_any_element()
            }
            BranchRow::Branch(index) => {
                let branch = &self.branches[index];
                let name = branch.name.clone();
                let reference = format!(
                    "{}{}",
                    if branch.remote {
                        "refs/remotes/"
                    } else {
                        "refs/heads/"
                    },
                    name
                );
                let switch_name = name.clone();
                h_flex()
                    .id(("branch-row", index))
                    .w_full()
                    .h(px(ROW_HEIGHT))
                    .flex_shrink_0()
                    .px(px(8.))
                    .pr_4()
                    .gap(px(8.))
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .hover(|style| style.bg(cx.theme().list_hover))
                    .child(
                        div()
                            .w(px(12.))
                            .flex_shrink_0()
                            .when(branch.current, |view| {
                                view.child(
                                    Icon::new(IconName::Check)
                                        .size(px(12.))
                                        .text_color(cx.theme().ring),
                                )
                            }),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(name.clone()))
                    .when(branch.current, |view| {
                        view.child(
                            div()
                                .text_size(px(11.))
                                .flex_shrink_0()
                                .text_color(cx.theme().muted_foreground)
                                .child(messages().current()),
                        )
                    })
                    .when(!branch.remote && !branch.current, |view| {
                        view.child(
                            controls::button(("checkout", index))
                                .label(messages().switch())
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.stop_propagation();
                                    cx.emit(BranchEvent::Switch(switch_name.clone()));
                                })),
                        )
                    })
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(BranchEvent::Browse(HistoryScope::Branch(reference.clone())))
                    }))
                    .tooltip(move |window, cx| Tooltip::new(name.clone()).build(window, cx))
                    .into_any_element()
            }
            BranchRow::Empty => h_flex()
                .h(px(ROW_HEIGHT))
                .px(px(12.))
                .text_size(px(12.))
                .text_color(cx.theme().muted_foreground)
                .child(if self.branches.is_empty() {
                    messages().no_branches()
                } else {
                    messages().no_matching_branches()
                })
                .into_any_element(),
        }
    }
}

impl Render for BranchPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Leave room for the title bar, search field, popup margins and borders.
        let available = (f32::from(window.viewport_size().height) - 130.).max(ROW_HEIGHT);
        let height = (self.rows.len() as f32 * ROW_HEIGHT)
            .min(MAX_LIST_HEIGHT)
            .min(available);
        v_flex()
            .w(px(340.))
            .min_w_0()
            .child(
                h_flex()
                    .p_1()
                    .gap_1()
                    .child(
                        controls::button("browse-head")
                            .label(messages().current_branch())
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(BranchEvent::Browse(HistoryScope::Head))
                            })),
                    )
                    .child(
                        controls::button("browse-all")
                            .label(messages().all_branches())
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(BranchEvent::Browse(HistoryScope::All))
                            })),
                    ),
            )
            .child(div().p(px(8.)).child(controls::search_input(&self.search)))
            .child(
                div()
                    .relative()
                    .h(px(height))
                    .w_full()
                    .overflow_hidden()
                    .child(
                        uniform_list(
                            "branch-list",
                            self.rows.len(),
                            cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                                range
                                    .map(|index| this.row(this.rows[index], cx))
                                    .collect::<Vec<_>>()
                            }),
                        )
                        .size_full()
                        .track_scroll(&self.scroll),
                    )
                    .child(
                        Scrollbar::vertical(&self.scroll)
                            .mode(ScrollbarMode::Always)
                            .viewport_from_layout(),
                    ),
            )
            .pb_1()
    }
}

#[cfg(test)]
mod tests {
    use super::{Branch, BranchRow, branch_rows};

    #[test]
    fn search_and_collapse_preserve_group_counts_and_source_indices() {
        let branches = vec![
            Branch {
                name: "origin/local".into(),
                remote: false,
                current: false,
            },
            Branch {
                name: "main".into(),
                remote: false,
                current: true,
            },
            Branch {
                name: "origin/main".into(),
                remote: true,
                current: false,
            },
        ];
        assert_eq!(
            branch_rows(&branches, " MAIN ", [false; 2]),
            vec![
                BranchRow::Group {
                    remote: false,
                    count: 1
                },
                BranchRow::Branch(1),
                BranchRow::Group {
                    remote: true,
                    count: 1
                },
                BranchRow::Branch(2),
            ]
        );
        assert_eq!(
            branch_rows(&branches, "main", [true; 2]),
            vec![
                BranchRow::Group {
                    remote: false,
                    count: 1
                },
                BranchRow::Group {
                    remote: true,
                    count: 1
                },
            ]
        );
        assert_eq!(
            branch_rows(&branches, "missing", [false; 2]).last(),
            Some(&BranchRow::Empty)
        );
    }
}

#[cfg(all(test, feature = "ui-tests"))]
mod layout_tests {
    use super::{Branch, BranchPicker};
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AppContext, Bounds, ScrollStrategy, TestAppContext, WindowBounds, WindowOptions, px, size,
    };

    #[gpui_kit::test]
    fn many_branches_stay_bounded_and_scroll_to_last_item(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (window, picker) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(640.), px(600.)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        let mut picker = BranchPicker::new(window, cx);
                        picker.set_branches(
                            (0..500)
                                .map(|index| Branch {
                                    name: format!("origin/feature-{index:03}"),
                                    remote: true,
                                    current: false,
                                })
                                .collect::<Vec<_>>(),
                            cx,
                        );
                        picker
                    })
                },
            )
            .unwrap()
        });
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            for _ in 0..3 {
                window.render_frame(cx);
            }
            let scroll = &picker.read(cx).scroll;
            let layout = scroll
                .0
                .borrow()
                .last_item_size
                .expect("list must be laid out");
            assert_eq!(layout.item.height, px(420.));
            assert_eq!(layout.contents.height, px(502. * 30.));
            assert_eq!(scroll.logical_scroll_top_index(), 0);
            assert_eq!(scroll.is_scrolled_to_end(), Some(false));
            picker.update(cx, |picker, cx| {
                picker
                    .scroll
                    .scroll_to_item(picker.rows.len() - 1, ScrollStrategy::Bottom);
                cx.notify();
            });
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            for _ in 0..3 {
                window.render_frame(cx);
            }
            let scroll = &picker.read(cx).scroll;
            assert!(scroll.0.borrow().base_handle.offset().y < px(-480. * 30.));
            assert_eq!(
                scroll.is_scrolled_to_end(),
                Some(true),
                "last branch must be reachable"
            );
        })
        .unwrap();
    }
}
