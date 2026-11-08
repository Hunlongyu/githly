use super::controls;
use super::*;
use crate::i18n::messages;
use gpui_kit::component::{
    Icon,
    resizable::{h_resizable, resizable_panel, v_resizable},
    scroll::{Scrollbar, ScrollbarMode},
    tab::Tab,
    tooltip::Tooltip,
};

pub(super) const FILE_ROW_HEIGHT: f32 = 28.;

pub(super) const HISTORY_ROW_HEIGHT: f32 = 32.;
const HISTORY_HEADER_HEIGHT: f32 = 28.;

pub(super) fn initial_history_limit(window: &Window) -> usize {
    // Project and branch loads start with the detail pane closed.
    let list_height = f32::from(window.viewport_size().height)
        - title_bar::TITLE_BAR_HEIGHT
        - controls::TOOLBAR_HEIGHT
        - HISTORY_HEADER_HEIGHT;
    2 * (list_height / HISTORY_ROW_HEIGHT).ceil().max(1.) as usize
}

struct UntrackedHeader {
    panel: Entity<GitPanel>,
    row: usize,
    count: usize,
}

impl UniformListDecoration for UntrackedHeader {
    fn compute(
        &self,
        _: std::ops::Range<usize>,
        bounds: Bounds<Pixels>,
        offset: Point<Pixels>,
        row_height: Pixels,
        _: usize,
        _: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        // Decorations are positioned in content coordinates. Keep the header
        // on its own row until that row crosses either edge of the viewport.
        let viewport_top = -offset.y;
        let last_top = viewport_top + (bounds.size.height - row_height).max(px(0.));
        let top = (row_height * self.row).clamp(viewport_top, last_top);
        let header = self.panel.update(cx, |panel, cx| {
            panel.file_group_header(true, self.count, cx)
        });
        div()
            .size_full()
            .child(
                div()
                    .absolute()
                    .top(top)
                    .left_0()
                    .right_0()
                    .block_mouse_except_scroll()
                    .child(header),
            )
            .into_any_element()
    }
}

impl GitPanel {
    pub(super) fn cancel_history_filter(&mut self) {
        self.history_filter_generation
            .fetch_add(1, Ordering::Relaxed);
        self.history_filter_task = None;
        self.history_filtering = false;
    }

    pub(super) fn filter_history(&mut self, cx: &mut Context<Self>) {
        let query = self.history_search.read(cx).value().to_lowercase();
        self.cancel_history_filter();
        let Some(repository) = self.repository.clone() else {
            self.history_indices = Vec::new();
            return;
        };
        if query.is_empty() {
            self.history_indices = (0..repository.commits.len()).collect();
            return;
        }
        self.history_filtering = true;
        // Existing indices may belong to the old history after a ref change.
        self.history_indices.clear();
        let cancellation = self.history_filter_generation.clone();
        let generation = cancellation.load(Ordering::Relaxed);
        let executor = cx.background_executor().clone();
        self.history_filter_task = Some(cx.spawn(async move |this, cx| {
            executor.timer(std::time::Duration::from_millis(150)).await;
            let indices = executor
                .spawn(async move {
                    let mut indices = Vec::new();
                    for (index, commit) in repository.commits.iter().enumerate() {
                        if index % 256 == 0 {
                            // Let a replaced search task stop before visiting more commits.
                            if cancellation.load(Ordering::Relaxed) != generation {
                                return None;
                            }
                            futures_lite::future::yield_now().await;
                        }
                        if commit.id.contains(&query)
                            || matches_query(&commit.subject, &query)
                            || matches_query(&commit.author, &query)
                        {
                            indices.push(index);
                        }
                    }
                    Some(indices)
                })
                .await;
            let Some(indices) = indices else { return };
            let _ = this.update(cx, |this, cx| {
                if this.history_filter_generation.load(Ordering::Relaxed) == generation {
                    this.history_indices = indices;
                    this.history_filtering = false;
                    cx.notify();
                }
            });
        }));
    }

    pub(super) fn welcome(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut recent = v_flex()
            .w(px(260.))
            .h(px(350.))
            .pl_6()
            .gap(px(12.))
            .border_l_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .w_full()
                    .h(px(controls::CONTROL_HEIGHT))
                    .flex_shrink_0()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(messages().recent_projects()),
                    )
                    .when(!self.config.recent_projects.is_empty(), |header| {
                        header.child(
                            controls::icon_button(
                                "clear-recent",
                                Icon::new(gpui_kit::assets::IconName::ListX)
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .accessibility_label(messages().clear_recent())
                            .tooltip(messages().clear_recent())
                            .on_click(cx.listener(|this, _, _, cx| this.clear_recent(cx))),
                        )
                    }),
            );
        for (index, path) in self.config.recent_projects.iter().enumerate() {
            let target = path.clone();
            recent = recent.child(
                v_flex()
                    .id(("recent", index))
                    .p(px(8.))
                    .gap(px(4.))
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|style| style.bg(cx.theme().list_hover))
                    .child(
                        div().text_sm().font_medium().child(
                            path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                        ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(path.display().to_string()),
                    )
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.load_project(target.clone(), cx)),
                    ),
            );
        }
        if self.config.recent_projects.is_empty() {
            recent = recent.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(messages().no_recent()),
            );
        }
        h_flex()
            .flex_1()
            .min_h_0()
            .justify_center()
            .gap_8()
            .p_8()
            .child(
                v_flex()
                    .w(px(450.))
                    .items_center()
                    .gap(px(16.))
                    .child(crate::app_assets::logo(px(64.)))
                    .child(div().text_2xl().font_semibold().child("Githly"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(messages().welcome()),
                    )
                    .when(self.detecting, |view| {
                        view.child(messages().detect_system_git())
                    })
                    .when(self.git.is_none() && !self.detecting, |view| {
                        view.child(
                            self.git_error.as_ref().map_or_else(
                                || messages().no_git().to_owned(),
                                ToString::to_string,
                            ),
                        )
                        .child(self.git_controls(cx))
                    })
                    .when(self.git.is_some(), |view| {
                        view.child(
                            v_flex()
                                .id("load-project-zone")
                                .test_support()
                                .w_full()
                                .h(px(150.))
                                .mt_3()
                                .items_center()
                                .justify_center()
                                .gap(px(12.))
                                .rounded_lg()
                                .border_1()
                                .border_dashed()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().muted)
                                .cursor_pointer()
                                .hover(|style| style.bg(cx.theme().list_hover))
                                .child(Icon::new(IconName::FolderOpen).size_6())
                                .child(div().text_sm().font_medium().child(messages().open_local()))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(messages().drop_repository()),
                                )
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.choose_path(false, cx)),
                                ),
                        )
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(messages().open_shortcut()),
                    ),
            )
            .child(recent)
            .into_any_element()
    }

    fn file_row(
        &self,
        file: &ChangedFile,
        index: usize,
        working: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let label = file.display_path();
        let (directory, name) = label.rsplit_once('/').unwrap_or(("", label.as_ref()));
        let selected = self.detail_open
            && self
                .selected_file
                .and_then(|i| self.files.get(i))
                .is_some_and(|selected| selected.path == file.path)
            && self.selected_commit.is_none() == working;
        let full_path = self
            .repository
            .as_ref()
            .map(|repo| repo.root.join(label.as_ref()).display().to_string())
            .unwrap_or_else(|| label.to_string());
        h_flex()
            .id((
                if working {
                    "working-file"
                } else {
                    "detail-file"
                },
                index,
            ))
            .w_full()
            .h(px(FILE_ROW_HEIGHT))
            .flex_shrink_0()
            .line_height(px(18.))
            .px(px(12.))
            .gap(px(8.))
            .border_l_2()
            .border_color(gpui_kit::transparent_black())
            .when(selected, |row| {
                row.bg(cx.theme().list_active).border_color(cx.theme().ring)
            })
            .when(!selected, |row| {
                row.hover(|style| style.bg(cx.theme().list_hover))
            })
            .child(
                Icon::new(IconName::File)
                    .size_4()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .flex_shrink_0()
                            .max_w(relative(0.65))
                            .truncate()
                            .child(name.to_string()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .truncate()
                            .child(if directory.is_empty() { "/" } else { directory }.to_string()),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(12.))
                    .text_color(if file.status.contains('D') {
                        cx.theme().danger
                    } else if file.status.contains('A') || file.status == "??" {
                        cx.theme().success
                    } else {
                        cx.theme().warning
                    })
                    .child(file.status.trim().replace("??", "U")),
            )
            .tooltip(move |window, cx| Tooltip::new(full_path.clone()).build(window, cx))
            .when(working, |view| {
                let counts = self.statistics.files.get(&file.path);
                view.child(
                    div()
                        .flex_shrink_0()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(if self.statistics_pending {
                            "…".to_string()
                        } else if let Some(Some((added, removed))) = counts {
                            format!("+{added} −{removed}")
                        } else {
                            "—".into()
                        }),
                )
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                if working {
                    this.changes_focus.focus(window, cx);
                    this.select_change(index, cx);
                } else {
                    this.files_focus.focus(window, cx);
                    this.select_file(index, cx);
                }
            }))
            .into_any_element()
    }

    fn history_options(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().downgrade();
        let display = self.config.history_display.clone();
        controls::icon_button("history-options", IconName::Ellipsis)
            .tooltip(messages().history_display())
            .dropdown_menu(move |mut menu, _, _| {
                for (key, label, checked) in [
                    (0, messages().show_hash(), display.hash),
                    (1, messages().show_tags(), display.tags),
                    (2, messages().show_branches(), display.branches),
                    (3, messages().metadata_after(), display.metadata_after),
                ] {
                    let view = view.clone();
                    if key == 3 {
                        menu = menu.separator();
                    }
                    menu = menu.item(PopupMenuItem::new(label).checked(checked).on_click(
                        move |_, _, cx| {
                            let _ = view.update(cx, |this, cx| {
                                let d = &mut this.config.history_display;
                                match key {
                                    0 => d.hash = !d.hash,
                                    1 => d.tags = !d.tags,
                                    2 => d.branches = !d.branches,
                                    _ => d.metadata_after = !d.metadata_after,
                                }
                                this.save(cx);
                                cx.notify();
                            });
                        },
                    ));
                }
                menu
            })
    }

    fn history_metadata(
        &self,
        commit: &crate::git::Commit,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let display = &self.config.history_display;
        h_flex()
            .gap(px(4.))
            .min_w_0()
            .max_w(px(270.))
            .children(
                commit
                    .references
                    .iter()
                    .filter(|r| {
                        if r.starts_with("tag: ") {
                            display.tags
                        } else {
                            display.branches
                        }
                    })
                    .map(|reference| {
                        let label = match reference.strip_prefix("tag: ") {
                            Some(name) => messages().tag_label(name),
                            None => reference.trim_start_matches("HEAD -> ").to_owned(),
                        };
                        div()
                            .px_1()
                            .rounded_sm()
                            .bg(cx.theme().muted)
                            .text_color(cx.theme().muted_foreground)
                            .text_size(px(10.))
                            .truncate()
                            .child(label)
                    }),
            )
            .when(display.hash, |row| {
                row.child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(11.))
                        .text_color(cx.theme().muted_foreground)
                        .child(commit.short_id.clone()),
                )
            })
    }

    fn file_group_header(
        &self,
        untracked: bool,
        count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let group = usize::from(untracked);
        let collapsed = self.collapsed_change_groups[group];
        let heading = if untracked {
            messages().untracked()
        } else {
            messages().tracked()
        };
        gpui_kit::base::Button::new(("file-group", group))
            .accessibility_label(messages().file_group(
                heading,
                count,
                if collapsed {
                    messages().collapsed()
                } else {
                    messages().expanded()
                },
            ))
            .flex()
            .items_center()
            .w_full()
            .h(px(FILE_ROW_HEIGHT))
            .flex_shrink_0()
            .px(px(12.))
            .gap(px(8.))
            .line_height(px(18.))
            .bg(cx.theme().muted)
            .border_b_1()
            .border_color(cx.theme().border)
            .text_color(cx.theme().foreground)
            .hover(|style| style.bg(cx.theme().sidebar_accent))
            .focus_visible(|style| style.border_color(cx.theme().ring))
            .child(
                Icon::new(if collapsed {
                    IconName::ChevronRight
                } else {
                    IconName::ChevronDown
                })
                .size_4()
                .text_color(cx.theme().sidebar_primary),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(13.))
                    .font_semibold()
                    .child(heading),
            )
            .child(
                div()
                    .min_w(px(22.))
                    .px_1p5()
                    .text_center()
                    .rounded_full()
                    .bg(cx.theme().sidebar_accent)
                    .text_size(px(12.))
                    .font_semibold()
                    .text_color(cx.theme().sidebar_accent_foreground)
                    .child(count.to_string()),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.collapsed_change_groups[group] = !this.collapsed_change_groups[group];
                cx.notify();
            }))
            .into_any_element()
    }

    fn working_file_groups(&mut self, files: &Arc<[ChangedFile]>, cx: &mut Context<Self>) -> Div {
        let rows = self.working_rows(files, cx);
        let [tracked_count, untracked_count] = rows.counts;
        let header_row = rows.header_row;
        let list = uniform_list(
            "working-files",
            rows.indices.len() + 1,
            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                range
                    .map(|row| {
                        if row == header_row {
                            return div().h(px(FILE_ROW_HEIGHT)).into_any_element();
                        }
                        let index = rows.indices[row - usize::from(row > header_row)];
                        this.file_row(&rows.files[index], index, true, cx)
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .size_full()
        .track_scroll(&self.changes_scroll)
        .with_decoration(UntrackedHeader {
            panel: cx.entity().clone(),
            row: header_row,
            count: untracked_count,
        });
        v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_hidden()
            .child(self.file_group_header(false, tracked_count, cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .child(list)
                    .child(Scrollbar::vertical(&self.changes_scroll).viewport_from_layout()),
            )
    }

    pub(super) fn workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let repo = self.repository.as_ref().unwrap().clone();
        let left = v_flex()
            .track_focus(&self.changes_focus)
            .size_full()
            .min_w_0()
            .bg(cx.theme().sidebar)
            .when(!repo.operation_states.is_empty(), |view| {
                view.child(
                    div()
                        .px_3()
                        .py_2()
                        .text_xs()
                        .text_color(cx.theme().warning)
                        .child(
                            repo.operation_states
                                .iter()
                                .map(|state| state.get(messages()))
                                .collect::<Vec<_>>()
                                .join(" · "),
                        ),
                )
            })
            .child(
                h_flex()
                    .h(px(controls::TOOLBAR_HEIGHT))
                    .flex_shrink_0()
                    .px(px(8.))
                    .gap(px(4.))
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        controls::segmented("changes-stash-switch")
                            .w(px(190.))
                            .flex_shrink_0()
                            .selected_index(usize::from(self.stash_tab))
                            .child(
                                Tab::new()
                                    .flex_1()
                                    .label(messages().changes_tab(repo.changes.len())),
                            )
                            .child(
                                Tab::new()
                                    .flex_1()
                                    .label(messages().stash_tab(repo.stashes.len())),
                            )
                            .on_click(cx.listener(|this, index: &usize, _, cx| {
                                let stash = *index == 1;
                                if this.stash_tab != stash {
                                    this.stash_tab = stash;
                                    cx.notify();
                                }
                            })),
                    ),
            )
            .when(!self.stash_tab, |view| {
                let stats = &self.statistics;
                let summary = if repo.changes.is_empty() {
                    None
                } else if self.statistics_pending {
                    Some(messages().calculating_stats().to_string())
                } else if self.statistics_error.is_some() {
                    Some(messages().stats_unavailable().into())
                } else if stats.added == 0 && stats.removed == 0 {
                    stats.incomplete.then(|| messages().partial_stats().into())
                } else {
                    let note = if stats.incomplete {
                        messages().partial_stats_suffix()
                    } else {
                        ""
                    };
                    Some(format!("+{} −{}{note}", stats.added, stats.removed))
                };
                view.child(
                    div()
                        .px(px(8.))
                        .py(px(6.))
                        .child(controls::search_input(&self.file_search)),
                )
                .when_some(summary, |view, summary| {
                    view.child(
                        div()
                            .px_3()
                            .pb_1()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(summary),
                    )
                })
            })
            .child(if self.stash_tab && repo.stashes.is_empty() {
                v_flex()
                    .id("stash-empty")
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(Icon::new(gpui_kit::assets::IconName::Archive).size(px(22.)))
                    .child(
                        div()
                            .id("stash-empty-label")
                            .test_support()
                            .child(messages().no_stashes()),
                    )
                    .into_any_element()
            } else if self.stash_tab {
                v_flex()
                    .id("stash-list")
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .text_sm()
                    .child(
                        uniform_list(
                            "stash-items",
                            repo.stashes.len(),
                            cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                                range
                                    .map(|index| {
                                        let stash =
                                            &this.repository.as_ref().unwrap().stashes[index];
                                        let selected = this
                                            .selected_commit
                                            .as_ref()
                                            .is_some_and(|commit| commit.id == stash.id);
                                        v_flex()
                                            .id(("stash", index))
                                            .h(px(56.))
                                            .px_3()
                                            .py_2()
                                            .gap_1()
                                            .when(selected, |view| view.bg(cx.theme().list_active))
                                            .hover(|view| view.bg(cx.theme().list_hover))
                                            .child(div().truncate().child(stash.subject.clone()))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(format!(
                                                        "{} · {}",
                                                        stash.selector, stash.date
                                                    )),
                                            )
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.select_stash(index, cx)
                                            }))
                                    })
                                    .collect::<Vec<_>>()
                            }),
                        )
                        .flex_1()
                        .w_full()
                        .min_h_0(),
                    )
                    .into_any_element()
            } else {
                self.working_file_groups(&repo.changes, cx)
                    .into_any_element()
            });
        let graph = self.config.history_display.graph;
        let query_empty = self.history_search.read(cx).text().len() == 0;
        let history_summary = if self.history_filtering {
            messages().searching().to_owned()
        } else {
            let status = if self.history_loading {
                messages().loading()
            } else if repo.history_pending {
                messages().new_history()
            } else if repo.history_complete {
                messages().end_of_history()
            } else if !query_empty {
                messages().search_loaded()
            } else {
                ""
            };
            messages().shown_commits(self.history_indices.len(), status)
        };
        // Keep the graph mounted through its exit. A single transition drives
        // every visible row and the header, so scrolling cannot desynchronize it.
        let graph_progress = gpui_kit::base::motion::transition(
            "history-graph-visibility",
            if graph { 1_f32 } else { 0_f32 },
            gpui_kit::base::motion::Transition::new(std::time::Duration::from_millis(180)),
            window,
            cx,
        );
        // Filtered rows are not adjacent commits: never draw misleading edges.
        let graph_progress = if query_empty { graph_progress } else { 0. };
        let graph_width = (repo.graph_width as f32 * 14. + 16.).max(40.);
        let graph_slot_width = (graph_width + 12.) * graph_progress;
        let history = v_flex()
            .track_focus(&self.history_focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if !this.history_focus.is_focused(window) {
                    return;
                }
                let current = this.selected_commit.as_ref().and_then(|selected| {
                    this.history_indices.iter().position(|index| {
                        this.repository.as_ref().unwrap().commits[*index].id == selected.id
                    })
                });
                let count = this.history_indices.len();
                if count == 0 {
                    return;
                }
                let position = match event.keystroke.key.as_str() {
                    "down" => current.map_or(0, |i| (i + 1).min(count - 1)),
                    "up" => current.unwrap_or(0).saturating_sub(1),
                    "home" => 0,
                    "end" => count - 1,
                    _ => return,
                };
                this.select_commit(this.history_indices[position], cx);
                this.history_scroll
                    .scroll_to_item(position, ScrollStrategy::Nearest);
                cx.stop_propagation();
            }))
            .size_full()
            .min_h_0()
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .h(px(controls::TOOLBAR_HEIGHT))
                    .flex_shrink_0()
                    .px(px(12.))
                    .gap(px(8.))
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().sidebar)
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .child(messages().history_scope(repo.history_scope.label())),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(cx.theme().muted_foreground)
                            .child(history_summary),
                    )
                    .child(div().flex_1())
                    .child(
                        controls::segmented("history-view-switch")
                            .w(px(138.))
                            .flex_shrink_0()
                            .selected_index(usize::from(!graph))
                            .child(Tab::new().flex_1().label(messages().graph()))
                            .child(Tab::new().flex_1().label(messages().list()))
                            .on_click(cx.listener(|this, index: &usize, _, cx| {
                                let graph = *index == 0;
                                if this.config.history_display.graph != graph {
                                    this.config.history_display.graph = graph;
                                    this.save(cx);
                                    cx.notify();
                                }
                            })),
                    )
                    .child(self.history_options(cx))
                    .child(
                        div()
                            .w(px(225.))
                            .child(controls::search_input(&self.history_search)),
                    ),
            )
            .child(
                h_flex()
                    .h(px(HISTORY_HEADER_HEIGHT))
                    .flex_shrink_0()
                    .px(px(12.))
                    .text_size(px(11.))
                    .text_color(cx.theme().muted_foreground)
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().w(px(graph_slot_width)).flex_shrink_0())
                    .child(div().flex_1().mr_3().child(messages().commit_message()))
                    .child(
                        div()
                            .w(px(100.))
                            .mr_3()
                            .flex_shrink_0()
                            .child(messages().author()),
                    )
                    .child(
                        div()
                            .w(px(130.))
                            .flex_shrink_0()
                            .child(messages().commit_time()),
                    ),
            )
            .child(
                uniform_list(
                    "history",
                    self.history_indices.len(),
                    cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                        this.history_near_end = range.end.saturating_add(range.len().div_ceil(2))
                            >= this.history_indices.len();
                        if this.history_near_end
                            && !this.history_load_requested
                            && this.can_load_more_history(cx)
                        {
                            this.history_load_requested = true;
                            let entity = cx.entity().downgrade();
                            cx.defer(move |cx| {
                                let _ = entity.update(cx, |this, cx| {
                                    if std::mem::take(&mut this.history_load_requested)
                                        && this.history_near_end
                                    {
                                        this.load_more_history(cx);
                                    }
                                });
                            });
                        }
                        let repo = this.repository.as_ref().unwrap();
                        range
                            .map(|position| {
                                let index = this.history_indices[position];
                                let commit = &repo.commits[index];
                                let mut title = h_flex().flex_1().min_w_0().mr_3().gap(px(8.));
                                if !this.config.history_display.metadata_after {
                                    title = title.child(this.history_metadata(commit, cx));
                                }
                                title = title.child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_size(px(13.))
                                        .truncate()
                                        .child(commit.subject.clone()),
                                );
                                if this.config.history_display.metadata_after {
                                    title = title.child(this.history_metadata(commit, cx));
                                }
                                h_flex()
                                    .id(("commit", index))
                                    .w_full()
                                    .h(px(HISTORY_ROW_HEIGHT))
                                    .px(px(12.))
                                    .when(
                                        this.selected_commit
                                            .as_ref()
                                            .is_some_and(|c| c.id == commit.id)
                                            && this.detail_open,
                                        |row| row.bg(cx.theme().list_active),
                                    )
                                    .when(
                                        !this
                                            .selected_commit
                                            .as_ref()
                                            .is_some_and(|c| c.id == commit.id)
                                            || !this.detail_open,
                                        |row| row.hover(|style| style.bg(cx.theme().list_hover)),
                                    )
                                    .child(
                                        div()
                                            .w(px(graph_slot_width))
                                            .h_full()
                                            .flex_shrink_0()
                                            .overflow_hidden()
                                            .when(graph_progress > 0., |slot| {
                                                slot.child(graph_cell(
                                                    repo.graph[index].clone(),
                                                    graph_width,
                                                    graph_progress,
                                                    cx,
                                                ))
                                            }),
                                    )
                                    .child(title)
                                    .child(
                                        div()
                                            .w(px(100.))
                                            .mr_3()
                                            .flex_shrink_0()
                                            .text_size(px(11.))
                                            .text_color(cx.theme().muted_foreground)
                                            .truncate()
                                            .child(commit.author.clone()),
                                    )
                                    .child(
                                        div()
                                            .w(px(130.))
                                            .flex_shrink_0()
                                            .text_size(px(11.))
                                            .text_color(cx.theme().muted_foreground)
                                            .child(commit.date.clone()),
                                    )
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.history_focus.focus(window, cx);
                                        this.select_commit(index, cx)
                                    }))
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .track_scroll(&self.history_scroll)
                .flex_1()
                .w_full()
                .min_h_0(),
            )
            .when_some(self.history_error.clone(), |view, error| {
                view.child(
                    div()
                        .px_3()
                        .py_1()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(messages().history_retry(error)),
                )
            });
        let selected = self.selected_commit.as_ref();
        let heading = selected
            .map(|c| c.subject.clone())
            .unwrap_or_else(|| messages().local_changes().into());
        let meta = selected
            .map(|c| {
                messages().parent_comparison(
                    &c.short_id,
                    &c.author,
                    &c.date,
                    c.parents
                        .first()
                        .map(|s| &s[..7])
                        .unwrap_or(messages().empty_tree()),
                )
            })
            .unwrap_or_else(|| messages().worktree_snapshot().to_owned());
        let detail = v_flex()
            .size_full()
            .min_h_0()
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .h(px(56.))
                    .flex_shrink_0()
                    .px(px(12.))
                    .gap(px(12.))
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().sidebar)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_medium()
                                    .truncate()
                                    .child(heading),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(messages().detail_file_count(meta, self.files.len())),
                            ),
                    )
                    .when(
                        selected.is_some() && !self.commit_body.get(messages()).is_empty(),
                        |view| {
                            view.child(
                                controls::button("commit-message")
                                    .ghost()
                                    .label(messages().full_message())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let body = this.commit_body.clone();
                                        window.open_dialog(cx, move |dialog, _, _| {
                                            dialog
                                                .title(messages().commit_message())
                                                .w(px(640.))
                                                .centered()
                                                .child(
                                                    div()
                                                        .id("commit-body")
                                                        .max_h(px(420.))
                                                        .overflow_y_scroll()
                                                        .text_sm()
                                                        .child(body.to_string()),
                                                )
                                        });
                                    })),
                            )
                        },
                    )
                    .child(
                        controls::icon_button("close-detail", IconName::Close)
                            .tooltip(messages().close_details())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.close_detail(cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .items_stretch()
                    .when(selected.is_some(), |view| {
                        view.child(
                            detail_file_list(
                                self.files.len(),
                                &self.detail_files_scroll,
                                cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                                    range
                                        .filter_map(|index| {
                                            this.files
                                                .get(index)
                                                .map(|file| this.file_row(file, index, false, cx))
                                        })
                                        .collect::<Vec<_>>()
                                }),
                            )
                            .track_focus(&self.files_focus)
                            .w(px(205.))
                            .flex_shrink_0()
                            .border_r_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().sidebar),
                        )
                    })
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(
                                h_flex()
                                    .h(px(28.))
                                    .flex_shrink_0()
                                    .px(px(12.))
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .text_size(px(11.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        div().flex_1().min_w_0().truncate().child(
                                            self.selected_file
                                                .and_then(|i| self.files.get(i))
                                                .map(|f| f.display_path().into_owned())
                                                .unwrap_or_default(),
                                        ),
                                    )
                                    .when(
                                        self.image_diff.is_none() && self.diff_message.is_none(),
                                        |view| {
                                            view.child(
                                                controls::icon_button(
                                                    "previous-change",
                                                    IconName::ChevronUp,
                                                )
                                                .tooltip(messages().previous_change())
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.diff.update(cx, |diff, cx| {
                                                        diff.previous_change(cx)
                                                    })
                                                })),
                                            )
                                            .child(
                                                controls::icon_button(
                                                    "next-change",
                                                    IconName::ChevronDown,
                                                )
                                                .tooltip(messages().next_change())
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.diff
                                                        .update(cx, |diff, cx| diff.next_change(cx))
                                                })),
                                            )
                                        },
                                    ),
                            )
                            .when_some(self.diff_message.as_ref(), |view, message| {
                                view.child(
                                    div()
                                        .p_4()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(message.to_string()),
                                )
                            })
                            .when(self.diff_message.is_none(), |view| {
                                if let Some(images) = &self.image_diff {
                                    view.child(images.view(cx))
                                } else {
                                    view.child(diff_view(&self.diff))
                                }
                            }),
                    ),
            );
        div()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(
                h_resizable("workspace-layout")
                    .on_resize(cx.listener(
                        |this,
                         state: &Entity<gpui_kit::component::resizable::ResizableState>,
                         _,
                         cx| {
                            if let Some(width) = state.read(cx).sizes().first() {
                                this.config.file_panel_width = f32::from(*width).clamp(200., 460.);
                                this.save(cx);
                            }
                        },
                    ))
                    .child(
                        resizable_panel()
                            .size(px(self.config.file_panel_width.clamp(200., 460.)))
                            .size_range(px(200.)..px(460.))
                            .fixed()
                            .child(left),
                    )
                    .child(
                        resizable_panel().child(
                            v_resizable("history-detail-layout")
                                .on_resize(cx.listener(
                                    |this,
                                     state: &Entity<
                                        gpui_kit::component::resizable::ResizableState,
                                    >,
                                     _,
                                     cx| {
                                        if this.detail_open
                                            && let Some(height) = state.read(cx).sizes().get(1)
                                        {
                                            this.config.detail_panel_height =
                                                f32::from(*height).clamp(200., 800.);
                                            this.save(cx);
                                        }
                                    },
                                ))
                                .child(
                                    resizable_panel()
                                        .size_range(px(160.)..px(2000.))
                                        .child(history),
                                )
                                .child(
                                    resizable_panel()
                                        .size(px(self.config.detail_panel_height.clamp(200., 800.)))
                                        .size_range(px(200.)..px(800.))
                                        .fixed()
                                        .visible(self.detail_open)
                                        .child(detail),
                                ),
                        ),
                    ),
            )
            .into_any_element()
    }
}

pub(super) fn diff_view(state: &Entity<DiffState>) -> Diff {
    Diff::new(state)
        .header_visible(false)
        .syntax_highlight(false)
        .soft_wrap(true)
        // The surrounding panels already own each shared separator.
        .border_0()
        .size_full()
}

pub(super) fn detail_file_list(
    count: usize,
    scroll: &UniformListScrollHandle,
    render: impl Fn(std::ops::Range<usize>, &mut Window, &mut App) -> Vec<AnyElement> + 'static,
) -> Div {
    div()
        .relative()
        .h_full()
        .min_h_0()
        .overflow_hidden()
        .child(
            uniform_list("commit-files", count, render)
                .size_full()
                .track_scroll(scroll),
        )
        .child(
            Scrollbar::vertical(scroll)
                .mode(ScrollbarMode::Always)
                .viewport_from_layout(),
        )
}

fn graph_cell(row: crate::graph::GraphRow, width: f32, opacity: f32, cx: &App) -> impl IntoElement {
    let colors = [
        cx.theme().chart_1,
        cx.theme().chart_2,
        cx.theme().chart_3,
        cx.theme().chart_4,
        cx.theme().chart_5,
    ]
    .map(|color| color.opacity(opacity));
    let node = row.node;
    div()
        .w(px(width))
        .h(px(HISTORY_ROW_HEIGHT))
        .flex_shrink_0()
        .relative()
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let mut line = |from: usize, y1: f32, to: usize, y2: f32| {
                        let mut path = PathBuilder::stroke(px(1.5));
                        path.move_to(bounds.origin + point(px(8. + from as f32 * 14.), px(y1)));
                        path.line_to(bounds.origin + point(px(8. + to as f32 * 14.), px(y2)));
                        if let Ok(path) = path.build() {
                            window.paint_path(path, colors[from % colors.len()]);
                        }
                    };
                    for lane in 0..row.incoming {
                        line(lane, 0., lane, 16.);
                    }
                    for (from, to) in &row.edges {
                        line(*from, 16., *to, 32.);
                    }
                },
            )
            .size_full(),
        )
        .child(
            div()
                .absolute()
                .left(px(4.5 + node as f32 * 14.))
                .top(px(12.5))
                .size(px(7.))
                .rounded_full()
                .bg(colors[node % colors.len()]),
        )
}
