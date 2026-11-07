use super::*;
use crate::git::HistoryScope;

impl GitPanel {
    pub(super) fn browse_branch(&mut self, scope: HistoryScope, cx: &mut Context<Self>) {
        let Some(previous) = self.repository.clone() else {
            return;
        };
        let Some(git) = self.git.clone() else { return };
        self.branch_open = false;
        self.refresh_task = None;
        self.refresh_error = None;
        self.reset_history_reader(cx);
        self.cancel_history_filter();
        self.project_generation = self.project_generation.wrapping_add(1);
        self.statistics_task = None;
        self.statistics_pending = false;
        let generation = self.project_generation;
        self.tip("project", None, cx);
        let limit = self.initial_history_limit;
        let task = cx.background_spawn(async move { git.browse(&previous, scope, limit).await });
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.project_generation != generation {
                    return;
                }
                match result {
                    Ok(repo) => {
                        this.record_repository_read(cx);
                        this.close_detail(cx);
                        this.history_scroll = UniformListScrollHandle::new();
                        let retired = this.repository.replace(Arc::new(repo));
                        cx.background_spawn(async move { drop(retired) }).detach();
                    }
                    Err(error) => this.tip(
                        "project",
                        Some(notification(
                            NotificationType::Error,
                            LocalizedText::diagnostic(error),
                        )),
                        cx,
                    ),
                }
                this.filter_history(cx);
                this.start_auto_refresh(cx);
                this.update_statistics(true, cx);
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn switch_branch(&mut self, name: String, cx: &mut Context<Self>) {
        if self.switching_branch {
            return;
        }
        let Some(repo) = &self.repository else { return };
        let root = repo.root.clone();
        let Some(git) = self.git.clone() else { return };
        self.branch_open = false;
        self.switching_branch = true;
        self.tip(
            "checkout",
            Some(
                notification(
                    NotificationType::Info,
                    LocalizedText::new(Locale::switching_branch),
                )
                .autohide(false),
            ),
            cx,
        );
        let path = root.clone();
        let task = cx.background_spawn(async move { git.switch_branch(&root, &name).await });
        // A checkout owns its lifetime independently of selection/project UI.
        // Closing the detail or changing the project must not abort a write.
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.switching_branch = false;
                if this
                    .repository
                    .as_ref()
                    .is_some_and(|repo| repo.root == path)
                {
                    match result {
                        Ok(()) => {
                            this.tip("checkout", None, cx);
                            this.load_project(path, cx);
                        }
                        Err(error) => this.tip(
                            "checkout",
                            Some(notification(
                                NotificationType::Error,
                                LocalizedText::diagnostic(error),
                            )),
                            cx,
                        ),
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
