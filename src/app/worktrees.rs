use super::*;
use crate::i18n::messages;
use gpui_kit::component::tooltip::Tooltip;

pub(super) fn dropdown(git: Git, root: PathBuf, panel: WeakEntity<GitPanel>) -> impl IntoElement {
    controls::button("worktrees")
        .ghost()
        .label("worktree")
        .dropdown_caret(true)
        .dropdown_menu(move |menu, window, cx| {
            let git = git.clone();
            let root = root.clone();
            let panel = panel.clone();
            let query_root = root.clone();
            let query = cx.background_spawn(async move { git.worktrees(&query_root).await });
            let task = cx.spawn_in(window, async move |menu, cx| {
                let result = query.await;
                let _ = menu.update_in(cx, |menu, window, cx| {
                    menu.rebuild(window, cx, |mut menu, _, _| match result {
                        Ok(entries) if entries.is_empty() => menu.label(messages().no_worktrees()),
                        Ok(entries) => {
                            for entry in entries {
                                menu = menu.item(worktree_item(entry, &root, panel.clone()));
                            }
                            menu
                        }
                        Err(error) => menu.label(messages().worktrees_failed(error)),
                    });
                });
            });
            cx.on_release(move |_, _| drop(task)).detach();
            menu.min_w(px(400.))
                .max_w(px(520.))
                .scrollable(true)
                .label(messages().loading_worktrees())
        })
}

fn worktree_item(
    entry: crate::git::Worktree,
    root: &std::path::Path,
    panel: WeakEntity<GitPanel>,
) -> PopupMenuItem {
    let current = entry.path == root;
    let path = entry.path.display().to_string();
    let description = format!("{} · {}", entry.branch, entry.status);
    PopupMenuItem::element(move |_, cx| {
        let full_path = path.clone();
        v_flex()
            .id(SharedString::from(path.clone()))
            .flex_1()
            .min_w_0()
            .py_1()
            .child(div().text_sm().truncate().child(path.clone()))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .truncate()
                    .child(description.clone()),
            )
            .tooltip(move |window, cx| Tooltip::new(full_path.clone()).build(window, cx))
    })
    .checked(current)
    .disabled(!entry.available)
    .on_click(move |_, _, cx| {
        if !current {
            let _ = panel.update(cx, |panel, cx| panel.load_project(entry.path.clone(), cx));
        }
    })
}
