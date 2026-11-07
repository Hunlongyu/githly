use super::*;
use crate::i18n::messages;

impl GitPanel {
    pub(super) fn select_stash(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(repo) = &self.repository else { return };
        let Some(stash) = repo.stashes.get(index) else {
            return;
        };
        let id = stash.id.clone();
        let root = repo.root.clone();
        let Some(git) = self.git.clone() else { return };
        self.clear_detail(cx);
        self.detail_open = true;
        self.diff_message = messages().loading_stash().into();
        let generation = self.generation;
        let task = cx.background_spawn(async move { git.stash_detail(&root, &id).await });
        self.detail_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                match result {
                    Ok(crate::git::StashDetail {
                        commit,
                        files,
                        untracked_revision,
                        untracked_paths,
                        body,
                    }) => {
                        this.selected_commit = Some(commit);
                        this.commit_body = body;
                        this.stash_revision = untracked_revision;
                        this.stash_untracked_paths = untracked_paths;
                        this.files = files;
                        if this.files.is_empty() {
                            this.diff_message = messages().empty_stash().into();
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
}
