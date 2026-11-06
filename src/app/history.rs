use super::*;
use std::time::{Duration, Instant};

impl GitPanel {
    pub(super) fn reset_history_reader(&mut self, cx: &mut Context<Self>) {
        self.history_task = None;
        self.history_loading = false;
        self.history_near_end = false;
        self.history_error = None;
        self.history_retry_after = None;
        let stream = self.history_stream.take();
        cx.background_spawn(async move { drop(stream) }).detach();
    }

    pub(super) fn load_more_history(&mut self, cx: &mut Context<Self>) {
        if self.history_loading
            || !self.history_search.read(cx).value().is_empty()
            || self
                .history_retry_after
                .is_some_and(|time| Instant::now() < time)
        {
            return;
        }
        let Some(previous) = self.repository.clone() else {
            return;
        };
        if previous.history_complete || previous.commits.is_empty() {
            return;
        }
        let Some(git) = self.git.clone() else { return };
        let stream = self.history_stream.take();
        let generation = self.project_generation;
        let baseline = previous.commits.clone();
        self.history_loading = true;
        self.history_error = None;
        let task = cx.background_spawn(async move {
            let mut stream = match stream {
                Some(stream) => stream,
                None => crate::git::HistoryStream::start(&git, &previous)?,
            };
            let next = stream.extend(&previous).await?;
            let stream = (!next.history_complete).then_some(stream);
            anyhow::Ok((next, stream))
        });
        self.history_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.project_generation != generation
                    || !this
                        .repository
                        .as_ref()
                        .is_some_and(|repo| Arc::ptr_eq(&repo.commits, &baseline))
                {
                    return;
                }
                this.history_loading = false;
                match result {
                    Ok((next, stream)) => {
                        this.history_stream = stream;
                        let retired = this.repository.replace(Arc::new(next));
                        let start = this.history_indices.len();
                        let end = this.repository.as_ref().unwrap().commits.len();
                        if this.history_search.read(cx).value().is_empty() {
                            this.history_indices.extend(start..end);
                        } else {
                            this.filter_history(cx);
                        }
                        cx.background_spawn(async move { drop(retired) }).detach();
                    }
                    Err(error) => {
                        this.history_error = Some(format!("{error:#}"));
                        this.history_retry_after = Some(Instant::now() + Duration::from_secs(60));
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}
