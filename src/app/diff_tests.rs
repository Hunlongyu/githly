//! Actual layout regressions: equal panes, wrapped lines, no horizontal scrolling.
use gpui_kit::{
    AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Render, ScrollDelta, Styled,
    TestAppContext, Window, WindowBounds, WindowOptions,
    component::diff::{DiffFile, DiffMode, DiffState},
    div, point, px, size,
    test::TestWindowExt,
};

struct DiffHost {
    state: Entity<DiffState>,
    width: f32,
}

impl Render for DiffHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(self.width))
            .h_full()
            .child(super::view::diff_view(&self.state))
    }
}

#[gpui_kit::test]
fn switching_to_long_added_and_deleted_files_keeps_code_inside_viewport(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let (window, host) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Default::default(),
                    size: size(px(640.), px(300.)),
                })),
                ..Default::default()
            },
            cx,
            |_, cx| {
                cx.new(|cx| DiffHost {
                    state: cx.new(|cx| DiffState::new([], cx).with_mode(DiffMode::Split)),
                    width: 640.,
                })
            },
        )
        .unwrap()
    });
    let short = "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-old\n+new\n"
        .to_string();
    let long = "x".repeat(746);
    let added = format!(
        "diff --git a/design.json b/design.json\nnew file mode 100644\n--- /dev/null\n+++ b/design.json\n@@ -0,0 +1,2 @@\n+{{\n+{long}\n"
    );
    let deleted = format!(
        "diff --git a/deleted.txt b/deleted.txt\ndeleted file mode 100644\n--- a/deleted.txt\n+++ /dev/null\n@@ -1,2 +0,0 @@\n-old\n-{long}\n"
    );
    for (patch, side) in [
        (&short, "Modified"),
        (&added, "Modified"),
        (&deleted, "Original"),
        (&short, "Modified"),
    ] {
        cx.update_window(window, |_, window, cx| {
            let state = host.read(cx).state.clone();
            state.update(cx, |state, cx| state.set_files([], cx));
            window.render_frame(cx);
            state.update(cx, |state, cx| {
                state.set_files(DiffFile::parse(patch).unwrap(), cx)
            });
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            for _ in 0..3 {
                window.render_frame(cx);
            }
            let line = window
                .find_all(("line", 0usize))
                .into_iter()
                .find(|line| line.label().is_some_and(|label| label.contains(side)))
                .expect("the selected file must render its first source line");
            let bounds = line.bounds();
            assert!(bounds.size.width > px(0.));
            assert!(
                bounds.left() >= px(0.) && bounds.right() <= px(640.),
                "{side} escaped: {bounds:?}"
            );
            assert!(bounds.top() >= px(0.) && bounds.bottom() <= px(300.));
        })
        .unwrap();
    }
}

fn assert_equal_panes(window: &mut Window) {
    let body = window.find("diff-body").bounds();
    let old = window.find(("old", 0usize)).bounds();
    let new = window.find(("new", 0usize)).bounds();
    assert!((old.size.width - body.size.width / 2.).abs() <= px(1.));
    assert!((new.size.width - old.size.width).abs() <= px(1.));
    assert!((old.left() - body.left()).abs() <= px(1.));
    assert!((new.left() - old.right()).abs() <= px(1.));
    assert!((new.right() - body.right()).abs() <= px(1.));
}

#[gpui_kit::test]
fn long_lines_wrap_in_equal_panes_and_ignore_horizontal_scroll(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let patch = format!(
        "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-{}\n+{}\n",
        "a".repeat(180),
        "b".repeat(180)
    );
    let (window, host) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Default::default(),
                    size: size(px(800.), px(400.)),
                })),
                ..Default::default()
            },
            cx,
            |_, cx| {
                cx.new(|cx| DiffHost {
                    state: cx.new(|cx| {
                        DiffState::new(DiffFile::parse(&patch).unwrap(), cx)
                            .with_mode(DiffMode::Split)
                    }),
                    width: 640.,
                })
            },
        )
        .unwrap()
    });
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        for width in [640., 480., 780.] {
            host.update(cx, |host, cx| {
                host.width = width;
                cx.notify();
            });
            for _ in 0..3 {
                window.render_frame(cx);
            }
            assert_equal_panes(window);
            let before: Vec<_> = window
                .find_all("source")
                .iter()
                .map(|line| line.bounds())
                .collect();
            assert_eq!(before.len(), 2);
            assert!(
                before.iter().all(|bounds| bounds.size.height > px(40.)),
                "long lines must wrap"
            );
            window.scroll(
                "diff-body",
                ScrollDelta::Pixels(point(px(-180.), px(0.))),
                cx,
            );
            for _ in 0..3 {
                window.render_frame(cx);
            }
            let after: Vec<_> = window
                .find_all("source")
                .iter()
                .map(|line| line.bounds())
                .collect();
            assert_eq!(before, after, "horizontal gestures must not move the code");
            assert_equal_panes(window);
            for (side, expected) in [("new", 'b'), ("old", 'a')] {
                let pane = window.find((side, 0usize)).bounds();
                let start = point(pane.left() + px(70.), after[0].top() + px(8.));
                window.drag(start, start + point(px(50.), px(0.)), cx);
                let selected = host.read(cx).state.read(cx).selected_text(cx);
                assert!(
                    !selected.is_empty() && selected.chars().all(|ch| ch == expected),
                    "{side}: {selected:?}"
                );
            }
        }
    })
    .unwrap();
}
