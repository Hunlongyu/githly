use crate::i18n::messages;
use crate::{
    config::{Appearance, Config, ConfigStore},
    git::{ChangeScope, ChangedFile, Commit, Git, Repository},
};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{
    ActiveTheme, Disableable, IconName, Sizable, StyledExt, ThemeMode, WindowExt,
    button::ButtonVariants,
    diff::{Diff, DiffMode, DiffState},
    h_flex,
    menu::{DropdownMenu, PopupMenuItem},
    notification::Notification,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::{path::PathBuf, sync::Arc};

mod auto_refresh;
mod branch_picker;
mod branches;
mod controls;
mod diff_data;
#[cfg(all(test, feature = "ui-tests"))]
mod diff_tests;
mod file_list;
mod history;
mod image_diff;
#[cfg(all(test, feature = "ui-tests"))]
mod memory_tests;
#[cfg(all(test, feature = "ui-tests"))]
mod refresh_tests;
mod settings;
mod stash;
mod statistics;
mod title_bar;
mod updates;
mod view;
mod worktrees;

gpui_kit::actions!(
    githly,
    [OpenProject, OpenSettings, FocusHistory, ToggleBranches]
);

pub struct GitPanel {
    store: ConfigStore,
    config: Config,
    config_error: Option<String>,
    git: Option<Git>,
    repository: Option<Arc<Repository>>,
    git_error: Option<String>,
    detecting: bool,
    stash_tab: bool,
    collapsed_change_groups: [bool; 2],
    change_scope: ChangeScope,
    detail_change_scope: ChangeScope,
    statistics: crate::git::LineStatistics,
    statistics_pending: bool,
    statistics_error: Option<String>,
    statistics_read: Option<std::time::Instant>,
    statistics_task: Option<Task<()>>,
    statistics_generation: u64,
    selected_commit: Option<Commit>,
    selected_file: Option<usize>,
    files: Vec<ChangedFile>,
    detail_files_scroll: UniformListScrollHandle,
    diff: Entity<DiffState>,
    image_diff: Option<image_diff::ImageDiff>,
    diff_message: String,
    commit_body: String,
    stash_revision: Option<String>,
    stash_untracked_paths: std::collections::HashSet<String>,
    generation: u64,
    detect_task: Option<Task<()>>,
    load_task: Option<Task<()>>,
    refresh_task: Option<auto_refresh::AutoRefresh>,
    project_generation: u64,
    refresh_error: Option<String>,
    window: AnyWindowHandle,
    initial_history_limit: usize,
    history_scroll: UniformListScrollHandle,
    history_stream: Option<crate::git::HistoryStream>,
    history_task: Option<Task<()>>,
    history_loading: bool,
    history_near_end: bool,
    history_error: Option<String>,
    history_retry_after: Option<std::time::Instant>,
    history_filter_task: Option<Task<()>>,
    history_filter_generation: u64,
    history_filtering: bool,
    detail_task: Option<Task<()>>,
    diff_task: Option<Task<()>>,
    save_task: Option<Task<()>>,
    last_read: Option<auto_refresh::RepositoryRead>,
    focus: FocusHandle,
    history_focus: FocusHandle,
    files_focus: FocusHandle,
    changes_focus: FocusHandle,
    changes_scroll: UniformListScrollHandle,
    branch_open: bool,
    switching_branch: bool,
    initial_project: Option<PathBuf>,
    file_search: Entity<InputState>,
    branch_picker: Entity<branch_picker::BranchPicker>,
    history_search: Entity<InputState>,
    history_indices: Vec<usize>,
    detail_open: bool,
    updates: updates::UpdateState,
}

impl GitPanel {
    pub fn new(
        store: ConfigStore,
        config: Config,
        config_error: Option<String>,
        initial_project: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let file_search =
            cx.new(|cx| InputState::new(window, cx).placeholder(messages().filter_paths()));
        let history_search =
            cx.new(|cx| InputState::new(window, cx).placeholder(messages().search_history()));
        cx.subscribe(&file_search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        cx.subscribe(&history_search, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.filter_history(cx);
                cx.notify();
            }
        })
        .detach();
        let mut panel = Self {
            store,
            config,
            config_error,
            git: None,
            repository: None,
            git_error: None,
            detecting: false,
            stash_tab: false,
            collapsed_change_groups: [false; 2],
            change_scope: ChangeScope::All,
            detail_change_scope: ChangeScope::All,
            statistics: crate::git::LineStatistics::default(),
            statistics_pending: false,
            statistics_error: None,
            statistics_read: None,
            statistics_task: None,
            statistics_generation: 0,
            selected_commit: None,
            selected_file: None,
            files: Vec::new(),
            detail_files_scroll: UniformListScrollHandle::new(),
            diff: cx.new(|cx| DiffState::new([], cx).with_mode(DiffMode::Split)),
            image_diff: None,
            diff_message: String::new(),
            commit_body: String::new(),
            stash_revision: None,
            stash_untracked_paths: std::collections::HashSet::new(),
            generation: 0,
            detect_task: None,
            load_task: None,
            refresh_task: None,
            project_generation: 0,
            refresh_error: None,
            window: window.window_handle(),
            initial_history_limit: view::initial_history_limit(window),
            history_scroll: UniformListScrollHandle::new(),
            history_stream: None,
            history_task: None,
            history_loading: false,
            history_near_end: false,
            history_error: None,
            history_retry_after: None,
            history_filter_task: None,
            history_filter_generation: 0,
            history_filtering: false,
            detail_task: None,
            diff_task: None,
            save_task: None,
            last_read: None,
            focus: cx.focus_handle(),
            history_focus: cx.focus_handle(),
            files_focus: cx.focus_handle(),
            changes_focus: cx.focus_handle(),
            changes_scroll: Default::default(),
            branch_open: false,
            switching_branch: false,
            initial_project,
            file_search,
            branch_picker: cx.new(|cx| branch_picker::BranchPicker::new(window, cx)),
            history_search,
            history_indices: Vec::new(),
            detail_open: false,
            updates: updates::UpdateState::default(),
        };
        panel.focus.focus(window, cx);
        cx.observe_window_activation(window, |panel, _, _| panel.wake_auto_refresh())
            .detach();
        cx.observe_window_visibility(window, |panel, _, _, _| panel.wake_auto_refresh())
            .detach();
        cx.subscribe(
            &panel.branch_picker,
            |this, _, event: &branch_picker::BranchEvent, cx| match event {
                branch_picker::BranchEvent::Browse(scope) => this.browse_branch(scope.clone(), cx),
                branch_picker::BranchEvent::Switch(name) => this.switch_branch(name.clone(), cx),
            },
        )
        .detach();
        panel.detect_git(panel.config.git_path.clone(), false, cx);
        panel.save(cx);
        if !cfg!(test) {
            panel.check_update(cx);
        }
        panel
    }

    fn tip(&self, key: &'static str, note: Option<Notification>, cx: &mut Context<Self>) {
        // A click may already hold the window. Update its overlay after the callback.
        let window = self.window;
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| match note {
                Some(note) => window.push_notification(note.id1::<GitPanel>(key), cx),
                None => window.remove_notification1::<GitPanel>(key, cx),
            });
        });
    }

    fn clear_recent(&mut self, cx: &mut Context<Self>) {
        self.config.recent_projects.clear();
        self.save(cx);
        self.tip(
            "recent",
            Some(Notification::success(messages().recent_cleared())),
            cx,
        );
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        // A damaged or newer configuration is never silently overwritten.
        if self.config_error.is_some() {
            return;
        }
        let store = self.store.clone();
        let config = self.config.clone();
        let previous = self.save_task.take();
        let executor = cx.background_executor().clone();
        self.save_task = Some(cx.spawn(async move |this, cx| {
            if let Some(previous) = previous {
                previous.await;
            }
            let task = executor.spawn(async move { store.save(&config) });
            if let Err(error) = task.await {
                let _ = this.update(cx, |this, cx| {
                    this.tip(
                        "save",
                        Some(Notification::error(messages().save_failed(error))),
                        cx,
                    );
                });
            }
        }));
    }

    fn detect_git(&mut self, path: Option<PathBuf>, persist: bool, cx: &mut Context<Self>) {
        self.detecting = true;
        self.git_error = None;
        let selected = path.clone();
        let task = cx.background_spawn(async move { Git::detect(path).await });
        self.detect_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.detecting = false;
                match result {
                    Ok(git) => {
                        this.git = Some(git);
                        if persist {
                            this.config.git_path = selected;
                            this.save(cx);
                        }
                        if let Some(project) = this.initial_project.take() {
                            this.load_project(project, cx);
                        }
                    }
                    Err(error) => {
                        this.git_error = Some(format!("{error:#}"));
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn choose_path(&mut self, executable: bool, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: executable,
            directories: !executable,
            multiple: false,
            prompt: Some(
                if executable {
                    messages().choose_git_exe()
                } else {
                    messages().open_project()
                }
                .into(),
            ),
        });
        cx.spawn(async move |this, cx| match receiver.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.into_iter().next() {
                    let _ = this.update(cx, |this, cx| {
                        if executable {
                            this.detect_git(Some(path), true, cx);
                        } else {
                            this.load_project(path, cx);
                        }
                    });
                }
            }
            Ok(Err(error)) => {
                let _ = this.update(cx, |this, cx| {
                    this.tip(
                        "path-picker",
                        Some(Notification::error(error.to_string())),
                        cx,
                    );
                });
            }
            _ => {}
        })
        .detach();
    }

    fn clear_detail(&mut self, cx: &mut Context<Self>) {
        self.detail_open = false;
        self.generation += 1;
        self.detail_task = None;
        self.diff_task = None;
        self.files = Vec::new();
        self.detail_files_scroll
            .scroll_to_item(0, ScrollStrategy::Top);
        self.selected_commit = None;
        self.selected_file = None;
        self.diff.update(cx, |diff, cx| diff.set_files([], cx));
        self.clear_image_diff(cx);
        self.diff_message.clear();
        self.commit_body = String::new();
        self.stash_revision = None;
        self.stash_untracked_paths = std::collections::HashSet::new();
    }

    fn clear_image_diff(&mut self, cx: &mut Context<Self>) {
        if let Some(images) = self.image_diff.take() {
            cx.defer(move |cx| {
                for window in cx.windows() {
                    let _ = window.update(cx, |_, window, _| images.release_textures(window));
                }
            });
        }
    }

    fn close_detail(&mut self, cx: &mut Context<Self>) {
        self.clear_detail(cx);
        // set_files([]) releases document data and cancels presentation work.
        // Dropping the old state also releases retained layout/selection capacity.
        self.diff = cx.new(|cx| DiffState::new([], cx).with_mode(DiffMode::Split));
        self.detail_files_scroll = UniformListScrollHandle::new();
    }

    pub(crate) fn load_project(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let Some(git) = self.git.clone() else {
            self.initial_project = Some(path);
            return;
        };
        let limit = self.initial_history_limit;
        self.begin_project_load(async move { git.open(&path, limit).await }, cx);
    }

    fn begin_project_load(
        &mut self,
        read: impl Future<Output = anyhow::Result<Repository>> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        self.release_project(cx);
        self.stash_tab = false;
        let project_generation = self.project_generation;
        let executor = cx.background_executor().clone();
        let task = cx.background_spawn(read);
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let result = futures_lite::future::or(task, async {
                executor.timer(std::time::Duration::from_millis(250)).await;
                this.update(cx, |panel, cx| {
                    if panel.project_generation == project_generation {
                        panel.tip(
                            "project",
                            Some(Notification::info(messages().loading_project()).autohide(false)),
                            cx,
                        );
                    }
                })?;
                std::future::pending().await
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                if this.project_generation != project_generation {
                    return;
                }
                match result {
                    Ok(repository) => {
                        this.tip("project", None, cx);
                        this.config.remember_project(&repository.root);
                        this.branch_picker.update(cx, |picker, cx| {
                            picker.set_branches(repository.branches.clone(), cx);
                        });
                        this.repository = Some(Arc::new(repository));
                        this.filter_history(cx);
                        this.record_repository_read(cx);
                        this.save(cx);
                        this.start_auto_refresh(cx);
                        this.update_statistics(true, cx);
                    }
                    Err(error) => {
                        this.tip(
                            "project",
                            Some(Notification::error(format!("{error:#}"))),
                            cx,
                        );
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn select_commit(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(repository) = &self.repository else {
            return;
        };
        let Some(commit) = repository.commits.get(index).cloned() else {
            return;
        };
        let root = repository.root.clone();
        let Some(git) = self.git.clone() else {
            return;
        };
        self.clear_detail(cx);
        self.selected_commit = Some(commit.clone());
        self.detail_open = true;
        self.diff_message = messages().loading_files().into();
        let generation = self.generation;
        let task = cx.background_spawn(async move {
            let files = git.commit_files(&root, &commit.id).await?;
            let body = git.commit_body(&root, &commit.id).await?;
            anyhow::Ok((files, body))
        });
        self.detail_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                match result {
                    Ok((files, body)) => {
                        this.commit_body = body;
                        this.files = files;
                        if this.files.is_empty() {
                            this.diff_message = messages().commit_no_diff().into();
                        } else {
                            this.select_file(0, cx);
                        }
                    }
                    Err(error) => this.diff_message = format!("{error:#}"),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn select_change(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(mut file) = self
            .repository
            .as_ref()
            .and_then(|repo| repo.changes.get(index))
            .cloned()
        else {
            return;
        };
        self.clear_detail(cx);
        self.detail_change_scope = self.change_scope;
        if self.change_scope == ChangeScope::Working {
            file.original_path = None;
        }
        self.detail_open = true;
        self.files = vec![file];
        self.select_file(0, cx);
    }

    fn select_file(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(repository) = &self.repository else {
            return;
        };
        let Some(file) = self.files.get(index).cloned() else {
            return;
        };
        let root = repository.root.clone();
        let mut id = self
            .selected_commit
            .as_ref()
            .map(|commit| commit.id.clone());
        let mut original_revision = if let Some(commit) = &self.selected_commit {
            commit.parents.first().cloned()
        } else {
            repository.head.clone()
        };
        if self.stash_untracked_paths.contains(&file.path) {
            id = self.stash_revision.clone();
            original_revision = None;
        }
        let scope = self.detail_change_scope;
        let local = self.selected_commit.is_none();
        let mut image_modified_revision = id.clone();
        if local {
            match scope {
                ChangeScope::Staged => image_modified_revision = Some(":".into()),
                ChangeScope::Working => original_revision = Some(":".into()),
                ChangeScope::All => {}
            }
        }
        let Some(git) = self.git.clone() else {
            return;
        };
        self.selected_file = Some(index);
        self.diff_message = messages().loading_diff().into();
        self.clear_image_diff(cx);
        self.diff.update(cx, |diff, cx| diff.set_files([], cx));
        let generation = self.generation;
        let task = cx.background_spawn(async move {
            if image_diff::is_image(&file.path)
                || file
                    .original_path
                    .as_deref()
                    .is_some_and(image_diff::is_image)
            {
                return Ok(diff_data::Content::Image(
                    image_diff::load(
                        &git,
                        &root,
                        original_revision.as_deref(),
                        image_modified_revision.as_deref(),
                        &file,
                    )
                    .await,
                ));
            }
            let patch = if local {
                git.diff_scoped(&root, None, &file, scope).await?
            } else {
                git.diff(&root, id.as_deref(), &file).await?
            };
            diff_data::parse(&patch).map(diff_data::Content::Text)
        });
        self.diff_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation || this.selected_file != Some(index) {
                    return;
                }
                match result {
                    Ok(diff_data::Content::Image(images)) => {
                        this.diff_message.clear();
                        this.image_diff = Some(images);
                    }
                    Ok(diff_data::Content::Text(files)) => {
                        this.diff_message = if files.is_empty() {
                            messages().no_text_diff().into()
                        } else {
                            String::new()
                        };
                        this.diff.update(cx, |diff, cx| diff.set_files(files, cx));
                    }
                    Err(error) => {
                        this.diff_message = format!("{error:#}");
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dark = !cx.theme().mode.is_dark();
        crate::theme::apply(
            if dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            Some(window),
            cx,
        );
        self.config.appearance = if dark {
            Appearance::Dark
        } else {
            Appearance::Light
        };
        self.save(cx);
        cx.notify();
    }

    fn git_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_2()
            .child(
                controls::button("detect-git")
                    .label(messages().detect_again())
                    .disabled(self.detecting)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.detect_git(this.config.git_path.clone(), false, cx)
                    })),
            )
            .child(
                controls::button("choose-git")
                    .label(messages().choose_git())
                    .disabled(self.detecting)
                    .on_click(cx.listener(|this, _, _, cx| this.choose_path(true, cx))),
            )
            .child(
                controls::button("git-website")
                    .ghost()
                    .label(messages().git_website())
                    .on_click(|_, _, cx| cx.open_url("https://git-scm.com/install/")),
            )
    }
}

impl Render for GitPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.initial_history_limit = view::initial_history_limit(window);
        let mut title = h_flex()
            .w_full()
            .h_full()
            .gap_2()
            .text_sm()
            .child(div().font_semibold().mr_2().child("Githly"));
        if let Some(repository) = &self.repository {
            let root = repository.root.clone();
            let recent_projects = self.config.recent_projects.clone();
            let view = cx.entity().downgrade();
            title = title.child(title_control(
                controls::button("project-menu")
                    .icon(IconName::Folder)
                    .dropdown_caret(true)
                    .ghost()
                    .small()
                    .label(
                        repository
                            .root
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                    )
                    .tooltip(messages().last_read(
                        repository.root.display(),
                        self.last_read.as_ref().map_or("—", |read| &read.label),
                    ))
                    .dropdown_menu(move |menu, _, _| {
                        let open = view.clone();
                        let close = view.clone();
                        let clear = view.clone();
                        let mut menu = menu
                            .max_w(px(460.))
                            .max_h(px(420.))
                            .scrollable(true)
                            .label(messages().recent_projects());
                        for project in &recent_projects {
                            let current = *project == root;
                            let target = project.clone();
                            let switch = view.clone();
                            // Include the path to distinguish projects with the same folder name.
                            menu = menu.item(
                                PopupMenuItem::new(project.display().to_string())
                                    .checked(current)
                                    .on_click(move |_, _, cx| {
                                        if !current {
                                            let _ = switch.update(cx, |this, cx| {
                                                this.load_project(target.clone(), cx)
                                            });
                                        }
                                    }),
                            );
                        }
                        menu.separator()
                            .item(PopupMenuItem::new(messages().open_another()).on_click(
                                move |_, _, cx| {
                                    let _ = open.update(cx, |this, cx| this.choose_path(false, cx));
                                },
                            ))
                            .item(PopupMenuItem::new(messages().clear_recent()).on_click(
                                move |_, _, cx| {
                                    let _ = clear.update(cx, |this, cx| {
                                        this.clear_recent(cx);
                                    });
                                },
                            ))
                            .separator()
                            .item(PopupMenuItem::new(messages().close_project()).on_click(
                                move |_, _, cx| {
                                    let _ = close.update(cx, |this, cx| {
                                        this.release_project(cx);
                                        cx.notify();
                                    });
                                },
                            ))
                    }),
            ));
            if repository.has_other_worktrees
                && let Some(git) = &self.git
            {
                title = title.child(title_control(worktrees::dropdown(
                    git.clone(),
                    repository.root.clone(),
                    cx.entity().downgrade(),
                )));
            }
            title = title.child(title_control(
                gpui_kit::component::popover::Popover::new("branches-popover")
                    .p_0()
                    .open(self.branch_open)
                    .trigger(
                        controls::button("branches")
                            .icon(IconName::Network)
                            .dropdown_caret(true)
                            .ghost()
                            .small()
                            .label(repository.branch.clone()),
                    )
                    .on_open_change(cx.listener(|this, open: &bool, window, cx| {
                        this.branch_open = *open;
                        if *open {
                            this.branch_picker
                                .update(cx, |picker, cx| picker.focus_search(window, cx));
                        }
                        cx.notify();
                    }))
                    .child(self.branch_picker.clone()),
            ));
            let (tracking_text, tracking_tip) = match &repository.tracking {
                Some(tracking) => match tracking.counts {
                    Some((ahead, behind)) => (
                        format!("{}  ↑{} ↓{}", tracking.upstream, ahead, behind),
                        messages().upstream_counts(ahead, behind),
                    ),
                    None => (
                        format!("{}  ↑— ↓—", tracking.upstream),
                        messages().upstream_missing().into(),
                    ),
                },
                None => (
                    messages().no_upstream().into(),
                    messages().no_upstream_tip().into(),
                ),
            };
            title = title.child(
                div()
                    .id("branch-tracking")
                    .max_w(px(260.))
                    .min_w_0()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .truncate()
                    .child(tracking_text)
                    .tooltip(move |window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(tracking_tip.clone())
                            .build(window, cx)
                    }),
            );
        }
        title = title
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            )
            .when(self.updates.update_available(), |bar| {
                bar.child(title_control(
                    controls::icon_button(
                        "available-update",
                        gpui_kit::component::Icon::new(gpui_kit::assets::IconName::CloudDownload)
                            .text_color(cx.theme().success),
                    )
                    .accessibility_label(messages().update_available_tip())
                    .tooltip(messages().update_available_tip())
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_update_dialog(window, cx)),
                    ),
                ))
            })
            .child(title_control(
                controls::icon_button(
                    "theme",
                    if cx.theme().mode.is_dark() {
                        IconName::Sun
                    } else {
                        IconName::Moon
                    },
                )
                .accessibility_label(messages().toggle_theme())
                .tooltip(if cx.theme().mode.is_dark() {
                    messages().switch_light()
                } else {
                    messages().switch_dark()
                })
                .on_click(cx.listener(|this, _, window, cx| this.theme(window, cx))),
            ))
            .child(title_control(
                controls::icon_button("settings", IconName::Settings)
                    .accessibility_label(messages().settings())
                    .tooltip(messages().settings_shortcut())
                    .on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx))),
            ));
        v_flex()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let working = this.changes_focus.is_focused(window);
                if !working && !this.files_focus.is_focused(window) {
                    return;
                }
                let Some(repo) = &this.repository else { return };
                if !matches!(event.keystroke.key.as_str(), "down" | "up" | "home" | "end")
                    || (working && this.stash_tab)
                {
                    return;
                }
                let (indices, tracked_count) = if working {
                    let [mut tracked, mut untracked] = file_list::grouped_file_indices(
                        &repo.changes,
                        &this.file_search.read(cx).value(),
                        this.change_scope,
                    );
                    if this.collapsed_change_groups[0] {
                        tracked.clear();
                    }
                    let tracked_count = tracked.len();
                    if !this.collapsed_change_groups[1] {
                        tracked.append(&mut untracked);
                    }
                    (tracked, tracked_count)
                } else {
                    ((0..this.files.len()).collect(), 0)
                };
                if indices.is_empty() {
                    return;
                }
                let current = if working {
                    this.selected_file
                        .and_then(|i| this.files.get(i))
                        .and_then(|selected| {
                            indices
                                .iter()
                                .position(|i| repo.changes[*i].path == selected.path)
                        })
                } else {
                    this.selected_file
                };
                let position = match event.keystroke.key.as_str() {
                    "down" => current.map_or(0, |i| (i + 1).min(indices.len() - 1)),
                    "up" => current.unwrap_or(0).saturating_sub(1),
                    "home" => 0,
                    "end" => indices.len() - 1,
                    _ => return,
                };
                if working {
                    this.select_change(indices[position], cx);
                    if position < tracked_count {
                        // Leave room below the selected row for the pinned header.
                        let scroll = &this.changes_scroll;
                        let top = -scroll.0.borrow().base_handle.offset().y;
                        let row =
                            position + usize::from(px(view::FILE_ROW_HEIGHT) * position >= top);
                        scroll.scroll_to_item(row, ScrollStrategy::Nearest);
                    } else {
                        this.changes_scroll.scroll_to_item_with_offset(
                            position + 1,
                            ScrollStrategy::Nearest,
                            1,
                        );
                    }
                } else {
                    this.select_file(indices[position], cx);
                    this.detail_files_scroll
                        .scroll_to_item(position, ScrollStrategy::Nearest);
                }
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &OpenProject, _, cx| this.choose_path(false, cx)))
            .on_action(
                cx.listener(|this, _: &OpenSettings, window, cx| this.open_settings(window, cx)),
            )
            .on_action(cx.listener(|this, _: &FocusHistory, window, cx| {
                if this.repository.is_some() {
                    this.history_search
                        .update(cx, |search, cx| search.focus(window, cx));
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleBranches, window, cx| {
                if this.repository.is_some() {
                    this.branch_open = !this.branch_open;
                    if this.branch_open {
                        this.branch_picker
                            .update(cx, |picker, cx| picker.focus_search(window, cx));
                    }
                    cx.notify();
                }
            }))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                if let Some(path) = paths.paths().first() {
                    this.load_project(path.clone(), cx);
                }
            }))
            .size_full()
            .text_size(px(13.))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(title_bar::title_bar(title, window, cx))
            .when_some(self.refresh_error.clone(), |view, error| {
                view.child(
                    div()
                        .px_3()
                        .py_1()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(messages().refresh_failed(error)),
                )
            })
            .when_some(
                self.repository
                    .as_ref()
                    .and_then(|repo| repo.worktree_error.as_ref()),
                |view, error| {
                    view.child(
                        div()
                            .px_3()
                            .py_1()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(messages().worktree_retry(error)),
                    )
                },
            )
            .when_some(self.config_error.clone(), |this, error| {
                this.child(
                    div()
                        .p_3()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
            .child(if self.repository.is_some() {
                self.workspace(window, cx)
            } else {
                self.welcome(cx)
            })
    }
}

fn title_control(element: impl IntoElement) -> impl IntoElement {
    h_flex()
        .h_full()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(element)
}
