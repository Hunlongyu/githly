use super::*;
use std::time::Duration;

impl GitPanel {
    pub(super) fn update_statistics(&mut self, force: bool, cx: &mut Context<Self>) {
        let now = cx.background_executor().now();
        if !force
            && (self.statistics_pending
                || self
                    .statistics_read
                    .is_some_and(|time| now.duration_since(time) < Duration::from_secs(60)))
        {
            return;
        }
        let Some(repo) = self.repository.clone() else {
            return;
        };
        let Some(git) = self.git.clone() else { return };
        self.statistics_generation = self.statistics_generation.wrapping_add(1);
        let generation = self.statistics_generation;
        let project = self.project_generation;
        self.statistics_pending = true;
        self.statistics_read = Some(now);
        let previous = self.statistics.clone();
        let task = cx.background_spawn(async move {
            let result = if !repo.changes.is_empty() {
                git.line_statistics(
                    &repo.root,
                    DiffTarget::WorkingTree {
                        head: repo.head.as_deref(),
                    },
                    &repo.changes,
                )
                .await
            } else {
                Ok(Default::default())
            };
            let result = result.map(Arc::new).map_err(|error| format!("{error:#}"));
            (previous.as_ref() != Some(&result)).then_some(result)
        });
        self.statistics_task = Some(cx.spawn(async move |this, cx| {
            let changed = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.project_generation != project || this.statistics_generation != generation {
                    return;
                }
                this.statistics_pending = false;
                if let Some(result) = changed {
                    let retired = this.statistics.replace(result);
                    cx.background_spawn(async move { drop(retired) }).detach();
                    cx.notify();
                }
            });
        }));
    }
}
