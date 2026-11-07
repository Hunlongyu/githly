use super::*;
use std::time::{Duration, Instant};

const ACTIVE_INTERVAL: Duration = Duration::from_secs(3);
const LIST_INTERVAL: Duration = Duration::from_secs(30);
const IDLE_INTERVAL: Duration = Duration::from_secs(600);

pub(super) struct RepositoryRead {
    finished_at: Instant,
    pub(super) label: String,
}

pub(super) struct AutoRefresh {
    wake: async_channel::Sender<()>,
    _task: Task<()>,
}

impl GitPanel {
    pub(super) fn record_repository_read(&mut self, cx: &Context<Self>) {
        self.last_read = Some(RepositoryRead {
            finished_at: cx.background_executor().now(),
            label: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        });
    }

    pub(super) fn wake_auto_refresh(&self) {
        if let Some(refresh) = &self.refresh_task {
            let _ = refresh.wake.try_send(());
        }
    }

    pub(super) fn release_project(&mut self, cx: &mut Context<Self>) {
        self.project_generation = self.project_generation.wrapping_add(1);
        self.refresh_task = None;
        self.reset_history_reader(cx);
        self.load_task = None;
        self.tip("project", None, cx);
        self.tip("checkout", None, cx);
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
        self.cancel_history_filter();
        self.file_rows = None;
        self.history_scroll = UniformListScrollHandle::new();
        self.changes_scroll = Default::default();
        self.branch_picker.update(cx, |picker, cx| picker.clear(cx));
        // Large file lists are released on the executor, not during a click.
        let previous = self.repository.take();
        cx.background_spawn(async move { drop(previous) }).detach();
    }

    pub(super) fn start_auto_refresh(&mut self, cx: &mut Context<Self>) {
        let generation = self.project_generation;
        let window = self.window;
        let executor = cx.background_executor().clone();
        let mut last_success = self
            .last_read
            .as_ref()
            .map_or_else(|| executor.now(), |read| read.finished_at);
        let mut interval_start = last_success;
        let mut last_list_read = last_success;
        let (wake, changes) = async_channel::bounded(1);
        let task = cx.spawn(async move |this, cx| {
            let mut was_active = false;
            loop {
                let Ok(active) = window.update(cx, |_, window, _| {
                    window.is_visible() && window.is_window_active()
                }) else {
                    break;
                };
                let interval = if active {
                    ACTIVE_INTERVAL
                } else {
                    IDLE_INTERVAL
                };
                if active && !was_active {
                    interval_start = last_success;
                }
                was_active = active;
                let delay = (interval_start + interval).saturating_duration_since(executor.now());
                if !delay.is_zero() {
                    // A window transition interrupts the wait, never an in-flight Git read.
                    futures_lite::future::or(
                        async {
                            executor.timer(delay).await;
                        },
                        async {
                            let _ = changes.recv().await;
                        },
                    )
                    .await;
                    continue;
                }
                let Ok(history_loading) = this.read_with(cx, |this, _| this.history_loading) else {
                    break;
                };
                if history_loading {
                    if changes.recv().await.is_err() {
                        break;
                    }
                    continue;
                }
                let Ok(Some((git, previous, update_history, initial_limit))) =
                    this.read_with(cx, |this, _| {
                        if this.project_generation != generation {
                            return None;
                        }
                        Some((
                            this.git.clone()?,
                            this.repository.clone()?,
                            this.history_scroll.0.borrow().base_handle.offset().y
                                >= px(-view::HISTORY_ROW_HEIGHT),
                            this.initial_history_limit,
                        ))
                    })
                else {
                    break;
                };
                let baseline_history = previous.commits.clone();
                // One task owns both timer and query. No tick queue, overlap,
                // watcher thread, or accumulating snapshot cache.
                let refresh_lists = executor.now().duration_since(last_list_read) >= LIST_INTERVAL;
                let result = executor
                    .spawn(async move {
                        git.refresh(&previous, update_history, refresh_lists, initial_limit)
                            .await
                    })
                    .await;
                interval_start = executor.now();
                if result.is_ok() {
                    last_success = interval_start;
                }
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
                            if refresh_lists {
                                last_list_read = interval_start;
                            }
                            let recovered = this.refresh_error.take().is_some();
                            this.record_repository_read(cx);
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
        });
        self.refresh_task = Some(AutoRefresh { wake, _task: task });
    }

    pub(super) fn apply_refresh(
        &mut self,
        next: Repository,
        cx: &mut Context<Self>,
    ) -> Option<Arc<Repository>> {
        let previous = self.repository.as_ref()?;
        let history_changed = !Arc::ptr_eq(&previous.commits, &next.commits);
        let branches_changed = !Arc::ptr_eq(&previous.branches, &next.branches);
        let offset = self.history_scroll.0.borrow().base_handle.offset();
        let top = ((-f32::from(offset.y)).max(0.) / view::HISTORY_ROW_HEIGHT).floor() as usize;
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
                let remainder = f32::from(offset.y) + top as f32 * view::HISTORY_ROW_HEIGHT;
                self.history_scroll.0.borrow().base_handle.set_offset(point(
                    offset.x,
                    px(-(position as f32) * view::HISTORY_ROW_HEIGHT + remainder),
                ));
            }
        }
        // Details are an independently owned snapshot. New commits, reset,
        // checkout, or status reordering must not change its comparison object.
        cx.notify();
        previous
    }
}
