use super::*;

struct WorktreeView {
    panel: WeakEntity<GitPanel>,
    entries: Vec<crate::git::Worktree>,
    message: Option<String>,
    task: Option<Task<()>>,
}

impl Render for WorktreeView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .max_h(px(420.))
            .gap_2()
            .when_some(self.message.clone(), |view, message| {
                view.child(div().text_sm().child(message))
            })
            .when(!self.entries.is_empty(), |view| {
                view.child(
                    uniform_list(
                        "worktree-list",
                        self.entries.len(),
                        cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                            range
                                .map(|index| {
                                    let entry = &this.entries[index];
                                    let path = entry.path.clone();
                                    let parent = this.panel.clone();
                                    let bare = entry.bare;
                                    v_flex()
                                        .id(("worktree", index))
                                        .h(px(76.))
                                        .p_2()
                                        .gap_1()
                                        .rounded_md()
                                        .hover(|view| view.bg(cx.theme().list_hover))
                                        .child(
                                            div()
                                                .text_sm()
                                                .truncate()
                                                .child(entry.path.display().to_string()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .truncate()
                                                .child(format!(
                                                    "{} · {}",
                                                    entry.branch, entry.status
                                                )),
                                        )
                                        .on_click(move |_, window, cx| {
                                            if bare {
                                                return;
                                            }
                                            window.close_dialog(cx);
                                            let _ = parent.update(cx, |panel, cx| {
                                                panel.load_project(path.clone(), cx)
                                            });
                                        })
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .h(px((self.entries.len() as f32 * 76.).min(380.)))
                    .w_full(),
                )
            })
    }
}

impl GitPanel {
    pub(super) fn open_worktrees(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = &self.repository else { return };
        let root = repo.root.clone();
        let Some(git) = self.git.clone() else { return };
        let parent = cx.entity().downgrade();
        let content = cx.new(|cx| {
            let task = cx.background_spawn(async move { git.worktrees(&root).await });
            let task = cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |this: &mut WorktreeView, cx| {
                    match result {
                        Ok(entries) => {
                            this.entries = entries;
                            this.message = None;
                        }
                        Err(error) => this.message = Some(format!("{error:#}")),
                    }
                    this.task = None;
                    cx.notify();
                });
            });
            WorktreeView {
                panel: parent,
                entries: Vec::new(),
                message: Some("正在读取工作目录…".into()),
                task: Some(task),
            }
        });
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title("已有工作目录")
                .w(px(640.))
                .centered()
                .child(content.clone())
        });
    }
}
