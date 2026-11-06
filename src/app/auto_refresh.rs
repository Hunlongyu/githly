use super::*;
use std::time::Duration;

const ACTIVE_INTERVAL: Duration = Duration::from_secs(15);
const IDLE_INTERVAL: Duration = Duration::from_secs(60);

impl GitPanel {
    pub(super) fn release_project(&mut self, cx: &mut Context<Self>) {
        self.project_generation = self.project_generation.wrapping_add(1);
        self.refresh_task = None;
        self.reset_history_reader(cx);
        self.load_task = None;
        self.loading = false;
        self.refresh_error = None;
        self.branch_open = false;
        self.statistics_task = None;
        self.statistics_pending = false;
        self.statistics_error = None;
        self.statistics_read = None;
        let statistics = std::mem::take(&mut self.statistics);
        cx.background_spawn(async move { drop(statistics) })
            .detach();
        self.last_read = None;
        self.close_detail(cx);
        self.history_indices = Vec::new();
        self.history_filter_task = None;
        self.history_filter_generation = self.history_filter_generation.wrapping_add(1);
        self.history_filtering = false;
        self.history_scroll = UniformListScrollHandle::new();
        self.changes_scroll = UniformListScrollHandle::new();
        self.branch_picker.update(cx, |picker, cx| picker.clear(cx));
        // Large file lists are released on the executor, not during a click.
        let previous = self.repository.take();
        cx.background_spawn(async move { drop(previous) }).detach();
    }

    pub(super) fn start_auto_refresh(&mut self, cx: &mut Context<Self>) {
        let generation = self.project_generation;
        let window = self.window;
        let executor = cx.background_executor().clone();
        self.refresh_task = Some(cx.spawn(async move |this, cx| {
            loop {
                let Ok(delay) = this.update(cx, |this, cx| {
                    let active = window
                        .update(cx, |_, window, _| {
                            window.is_visible() && window.is_window_active()
                        })
                        .unwrap_or(false);
                    if active && this.refresh_error.is_none() {
                        ACTIVE_INTERVAL
                    } else {
                        IDLE_INTERVAL
                    }
                }) else {
                    break;
                };
                executor.timer(delay).await;
                let Ok(Some((git, previous))) = this.update(cx, |this, _| {
                    if this.project_generation != generation {
                        return None;
                    }
                    Some((this.git.clone()?, this.repository.clone()?))
                }) else {
                    break;
                };
                let Ok(Some(update_history)) = this.update(cx, |this, _| {
                    if this.history_loading {
                        return None;
                    }
                    Some(this.history_scroll.0.borrow().base_handle.offset().y >= px(-32.))
                }) else {
                    continue;
                };
                let baseline_history = previous.commits.clone();
                // One task owns both timer and query. No tick queue, overlap,
                // watcher thread, or accumulating snapshot cache.
                let result = executor
                    .spawn(async move { git.refresh(&previous, update_history).await })
                    .await;
                let Ok(retired) = this.update(cx, |this, cx| {
                    if this.project_generation != generation {
                        return None;
                    }
                    // A scroll request can finish while a status query runs.
                    // Never replace the newly extended history with that old result.
                    if this.history_loading
                        || !this
                            .repository
                            .as_ref()
                            .is_some_and(|repo| Arc::ptr_eq(&repo.commits, &baseline_history))
                    {
                        return None;
                    }
                    if this.history_near_end && this.history_error.is_some() {
                        this.load_more_history(cx);
                    }
                    match result {
                        Ok(next) => {
                            let recovered = this.refresh_error.take().is_some();
                            this.last_read =
                                Some(chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
                            let retired = next.and_then(|repo| this.apply_refresh(repo, cx));
                            if recovered {
                                cx.notify();
                            }
                            this.update_statistics(false, cx);
                            retired
                        }
                        Err(error) => {
                            let message = format!("{error:#}");
                            if this.refresh_error.as_ref() != Some(&message) {
                                this.refresh_error = Some(message);
                                cx.notify();
                            }
                            None
                        }
                    }
                }) else {
                    break;
                };
                executor.spawn(async move { drop(retired) }).await;
            }
        }));
    }

    pub(super) fn apply_refresh(
        &mut self,
        next: Repository,
        cx: &mut Context<Self>,
    ) -> Option<Arc<Repository>> {
        let previous = self.repository.as_ref()?;
        let history_changed = !Arc::ptr_eq(&previous.commits, &next.commits);
        let branches_changed = previous.branches != next.branches;
        let offset = self.history_scroll.0.borrow().base_handle.offset();
        let top = ((-f32::from(offset.y)).max(0.) / 32.).floor() as usize;
        let anchor = self
            .history_indices
            .get(top)
            .and_then(|index| previous.commits.get(*index))
            .map(|c| c.id.clone());
        if branches_changed {
            self.branch_picker.update(cx, |picker, cx| {
                picker.set_branches(next.branches.clone(), cx)
            });
        }
        let previous = self.repository.replace(Arc::new(next));
        if history_changed {
            self.reset_history_reader(cx);
            self.filter_history(cx);
            if let Some(position) = anchor.and_then(|id| {
                self.history_indices
                    .iter()
                    .position(|index| self.repository.as_ref().unwrap().commits[*index].id == id)
            }) {
                let remainder = f32::from(offset.y) + top as f32 * 32.;
                self.history_scroll
                    .0
                    .borrow()
                    .base_handle
                    .set_offset(point(offset.x, px(-(position as f32) * 32. + remainder)));
            }
        }
        // Details are an independently owned snapshot. New commits, reset,
        // checkout, or status reordering must not change its comparison object.
        cx.notify();
        previous
    }
}
