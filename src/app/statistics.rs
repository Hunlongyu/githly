use super::*;
use std::time::{Duration, Instant};

impl GitPanel {
    pub(super) fn update_statistics(&mut self, force: bool, cx: &mut Context<Self>) {
        if !force
            && (self.statistics_pending
                || self
                    .statistics_read
                    .is_some_and(|time| time.elapsed() < Duration::from_secs(60)))
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
        let retired = std::mem::take(&mut self.statistics);
        cx.background_spawn(async move { drop(retired) }).detach();
        self.statistics_error = None;
        self.statistics_read = Some(Instant::now());
        let task = cx.background_spawn(async move {
            if !repo.changes.is_empty() {
                git.line_statistics(&repo.root, None, &repo.changes).await
            } else {
                Ok(Default::default())
            }
        });
        self.statistics_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.project_generation != project || this.statistics_generation != generation {
                    return;
                }
                this.statistics_pending = false;
                match result {
                    Ok(stats) => {
                        let retired = std::mem::replace(&mut this.statistics, stats);
                        cx.background_spawn(async move { drop(retired) }).detach();
                    }
                    Err(error) => this.statistics_error = Some(format!("{error:#}")),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}
