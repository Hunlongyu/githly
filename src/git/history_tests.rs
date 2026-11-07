use super::*;
use std::io::Write;

pub(crate) const NO_HOOKS: &str = if cfg!(windows) {
    "core.hooksPath=NUL"
} else {
    "core.hooksPath=/dev/null"
};

pub(crate) fn command(git: &Git, root: &Path, args: &[&str]) -> Vec<u8> {
    let mut cmd = std::process::Command::new(&git.executable);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let output = cmd.current_dir(root).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

pub(crate) fn fixture(count: usize) -> (tempfile::TempDir, Git) {
    let git = futures_lite::future::block_on(Git::detect(None)).unwrap();
    let directory = tempfile::tempdir().unwrap();
    command(&git, directory.path(), &["init", "-b", "main"]);
    command(&git, directory.path(), &["config", "user.name", "test"]);
    command(
        &git,
        directory.path(),
        &["config", "user.email", "test@example.invalid"],
    );
    let mut input = String::new();
    for index in 1..=count {
        let branch = if index == 210 { "side" } else { "main" };
        let message = format!("commit {index}");
        input.push_str(&format!("commit refs/heads/{branch}\nmark :{index}\ncommitter Test <test@example.invalid> {} +0000\ndata {}\n{message}\n", 1700000000 + index, message.len()));
        if index == 210 {
            input.push_str("from :100\n");
        }
        if index == 211 {
            input.push_str("merge :210\n");
        }
        input.push('\n');
    }
    let mut cmd = std::process::Command::new(&git.executable);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let mut child = cmd
        .current_dir(directory.path())
        .args(["fast-import", "--quiet"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (directory, git)
}

#[test]
#[ignore = "manual stress test: creates 100,000 Git commits"]
fn large_history_stream_stress() {
    for count in [19_685, 100_000] {
        let (directory, git) = fixture(count);
        futures_lite::future::block_on(async {
            let started = std::time::Instant::now();
            let mut repo = git.open(directory.path(), 46).await.unwrap();
            let first_batch = started.elapsed();
            assert_eq!(repo.commits.len(), 46);
            let first_address = &repo.commits[0] as *const Commit;
            let mut stream = HistoryStream::start(&git, &repo).unwrap();
            while !repo.history_complete {
                repo = stream.extend(&repo).await.unwrap();
            }
            assert_eq!(repo.commits.len(), count);
            assert_eq!(&repo.commits[0] as *const Commit, first_address);
            let expected = git
                .run(Some(directory.path()), ["rev-list", "--topo-order", "HEAD"])
                .await
                .unwrap();
            let expected = String::from_utf8(expected).unwrap();
            let expected: Vec<&str> = expected.lines().collect();
            assert_eq!(
                repo.commits
                    .iter()
                    .map(|c| c.id.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(repo.graph.len(), count);
            let weak = Arc::downgrade(&repo.commits);
            drop(repo);
            drop(stream);
            assert!(weak.upgrade().is_none());
            eprintln!(
                "{count} generated commits: first batch {first_batch:?}, full stream + comparison {:?}",
                started.elapsed()
            );
        });
    }
}

#[test]
fn stream_crosses_batches_without_duplicates_or_graph_discontinuity() {
    let (directory, git) = fixture(625);
    futures_lite::future::block_on(async {
        let root = directory.path();
        let mut repo = git.open(root, 46).await.unwrap();
        assert_eq!(repo.commits.len(), 46);
        let first_address = &repo.commits[0] as *const Commit;
        let initial_head = repo.commits[0].id.clone();
        let mut stream = HistoryStream::start(&git, &repo).unwrap();
        // Updating HEAD cannot move the stream's fixed traversal boundary.
        command(
            &git,
            root,
            &[
                "-c",
                NO_HOOKS,
                "commit",
                "--allow-empty",
                "-m",
                "external commit",
            ],
        );
        for expected in [146, 246, 346, 446, 546, 625] {
            repo = stream.extend(&repo).await.unwrap();
            assert_eq!(repo.commits.len(), expected);
            assert_eq!(
                &repo.commits[0] as *const Commit, first_address,
                "old batches must be shared"
            );
        }
        assert!(repo.history_complete);
        let raw = git
            .run(
                Some(root),
                [
                    "log",
                    "-z",
                    "--topo-order",
                    "--date=format-local:%Y-%m-%d %H:%M",
                    "--format=%H%x00%h%x00%an%x00%ad%x00%s%x00%P%x00%D",
                    &initial_head,
                    "--",
                ],
            )
            .await
            .unwrap();
        let expected = parse_log(&raw);
        assert_eq!(
            repo.commits.iter().map(|c| &c.id).collect::<Vec<_>>(),
            expected.iter().map(|c| &c.id).collect::<Vec<_>>()
        );
        assert_eq!(
            repo.graph.iter().cloned().collect::<Vec<_>>(),
            crate::graph::layout(&expected)
        );
        for (i, commit) in expected.iter().enumerate() {
            assert_eq!(repo.commits[i].id, commit.id);
        }
        assert!(repo.commits.get(expected.len()).is_none());
        let pending = git.refresh(&repo, false, true, 46).await.unwrap().unwrap();
        assert!(pending.history_pending);
        assert!(Arc::ptr_eq(&pending.commits, &repo.commits));
        let latest = git
            .refresh(&pending, true, true, 46)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(latest.commits[0].subject, "external commit");
        assert!(!latest.history_pending);
    });
}

#[test]
fn refresh_reuses_history_for_file_changes_and_detects_refs_and_stash() {
    let (directory, git) = fixture(4);
    futures_lite::future::block_on(async {
        let root = directory.path();
        let repo = git.open(root, 46).await.unwrap();
        assert!(git.refresh(&repo, true, true, 46).await.unwrap().is_none());
        std::fs::write(root.join("未跟踪.txt"), "hello").unwrap();
        let changed = git.refresh(&repo, true, true, 46).await.unwrap().unwrap();
        assert!(Arc::ptr_eq(&changed.commits, &repo.commits));
        assert_eq!(changed.changes[0].path, "未跟踪.txt");
        command(&git, root, &["stash", "push", "-u", "-m", "external stash"]);
        let stashed = git
            .refresh(&changed, true, true, 46)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stashed.stashes.len(), 1);
        assert!(stashed.changes.is_empty());
        command(&git, root, &["tag", "v-test"]);
        let tagged = git
            .refresh(&stashed, true, true, 46)
            .await
            .unwrap()
            .unwrap();
        assert!(
            tagged.commits[0]
                .references
                .iter()
                .any(|r| r.contains("v-test"))
        );
        assert!(Arc::ptr_eq(&tagged.changes, &stashed.changes));
        assert!(Arc::ptr_eq(&tagged.branches, &stashed.branches));
        let weak = Arc::downgrade(&tagged.commits);
        drop(tagged);
        assert!(weak.upgrade().is_none());
    });
}

#[test]
fn stream_exact_boundary_and_empty_repository_are_handled() {
    let (directory, git) = fixture(146);
    futures_lite::future::block_on(async {
        let repo = git.open(directory.path(), 46).await.unwrap();
        let mut stream = HistoryStream::start(&git, &repo).unwrap();
        let next = stream.extend(&repo).await.unwrap();
        assert_eq!(next.commits.len(), 146);
        let final_batch = stream.extend(&next).await.unwrap();
        assert_eq!(final_batch.commits.len(), 146);
        assert!(final_batch.history_complete);
    });
    let empty = tempfile::tempdir().unwrap();
    command(&git, empty.path(), &["init", "-b", "main"]);
    let repo = futures_lite::future::block_on(git.open(empty.path(), 46)).unwrap();
    assert!(repo.history_complete && repo.commits.is_empty());
}

#[test]
fn branch_browsing_is_read_only_and_checkout_rejects_dirty_worktree() {
    let (directory, git) = fixture(250);
    futures_lite::future::block_on(async {
        let root = directory.path();
        let repo = git.open(root, 46).await.unwrap();
        let side = git
            .browse(&repo, HistoryScope::Branch("refs/heads/side".into()), 46)
            .await
            .unwrap();
        assert_eq!(side.branch, "main");
        assert_eq!(side.head, repo.head);
        assert_eq!(side.commits[0].subject, "commit 210");
        let mut all = git.browse(&repo, HistoryScope::All, 46).await.unwrap();
        let mut stream = HistoryStream::start(&git, &all).unwrap();
        while !all.history_complete {
            all = stream.extend(&all).await.unwrap();
        }
        assert_eq!(all.commits.len(), 250);
        std::fs::write(root.join("dirty.txt"), "do not lose").unwrap();
        assert!(
            git.switch_branch(root, "side")
                .await
                .unwrap_err()
                .to_string()
                .contains(messages().dirty_checkout())
        );
        assert_eq!(git.open(root, 46).await.unwrap().branch, "main");
        std::fs::remove_file(root.join("dirty.txt")).unwrap();
        command(&git, root, &["tag", "tag-only"]);
        command(&git, root, &["branch", "group/nested"]);
        for name in ["tag-only", "group"] {
            assert!(
                git.switch_branch(root, name)
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains(messages().local_checkout_only())
            );
        }
        let marker = repo.git_dir.join("BISECT_LOG");
        std::fs::write(&marker, "").unwrap();
        assert!(
            git.switch_branch(root, "side")
                .await
                .unwrap_err()
                .to_string()
                .contains(messages().busy_checkout())
        );
        std::fs::remove_file(marker).unwrap();
        git.switch_branch(root, "side").await.unwrap();
        assert_eq!(git.open(root, 46).await.unwrap().branch, "side");
        command(
            &git,
            root,
            &[
                "-c",
                NO_HOOKS,
                "commit",
                "--allow-empty",
                "-m",
                "标题",
                "-m",
                "第一段\n\n第二段",
            ],
        );
        let body = git.commit_body(root, "HEAD").await.unwrap();
        assert!(body.contains("第一段\n\n第二段"));
    });
}

#[test]
fn untracked_statistics_bound_metadata_checks_as_well_as_reads() {
    let (directory, git) = fixture(1);
    std::fs::write(directory.path().join("last.txt"), "last\n").unwrap();
    let changes: Vec<_> = (0..=256)
        .map(|i| ChangedFile {
            // The first 256 files disappeared between status and statistics.
            path: if i == 256 {
                "last.txt".into()
            } else {
                format!("{i}.txt")
            },
            original_path: None,
            status: "??".into(),
        })
        .collect();
    let stats = futures_lite::future::block_on(git.line_statistics(
        directory.path(),
        None,
        ChangeScope::Working,
        &changes,
    ))
    .unwrap();
    assert!(stats.incomplete && stats.files["last.txt"].is_none());
}

#[test]
fn stash_details_bind_object_and_include_untracked_versions() {
    let (directory, git) = fixture(2);
    futures_lite::future::block_on(async {
        let root = directory.path();
        std::fs::write(root.join("tracked.txt"), "original\n").unwrap();
        command(&git, root, &["add", "tracked.txt"]);
        command(&git, root, &["-c", NO_HOOKS, "commit", "-m", "file"]);
        std::fs::write(root.join("tracked.txt"), "stash contents\n").unwrap();
        std::fs::write(root.join("未跟踪.txt"), "untracked snapshot\n").unwrap();
        command(&git, root, &["stash", "push", "-u", "-m", "first stash"]);
        let repo = git.open(root, 46).await.unwrap();
        let id = repo.stashes[0].id.clone();
        assert_eq!(repo.stashes[0].selector, "stash@{0}");
        std::fs::write(root.join("newer.txt"), "newer stash\n").unwrap();
        command(&git, root, &["stash", "push", "-u", "-m", "second stash"]);
        let detail = git.stash_detail(root, &id).await.unwrap();
        assert_eq!(detail.files.len(), 2);
        assert!(detail.untracked_paths.contains("未跟踪.txt"));
        let tracked = detail
            .files
            .iter()
            .find(|f| f.path == "tracked.txt")
            .unwrap();
        let untracked = detail
            .files
            .iter()
            .find(|f| f.path == "未跟踪.txt")
            .unwrap();
        assert!(
            git.diff(root, Some(&id), tracked)
                .await
                .unwrap()
                .contains("+stash contents")
        );
        assert!(
            git.diff(root, detail.untracked_revision.as_deref(), untracked)
                .await
                .unwrap()
                .contains("+untracked snapshot")
        );
    });
}

#[test]
fn staged_and_working_changes_keep_distinct_text_and_image_versions() {
    let (directory, git) = fixture(2);
    futures_lite::future::block_on(async {
        let root = directory.path();
        std::fs::write(root.join("file.txt"), "base\n").unwrap();
        std::fs::write(root.join("file.png"), b"base image").unwrap();
        command(&git, root, &["add", "."]);
        command(&git, root, &["-c", NO_HOOKS, "commit", "-m", "base"]);
        std::fs::write(root.join("file.txt"), "index\n").unwrap();
        std::fs::write(root.join("file.png"), b"index image").unwrap();
        command(&git, root, &["add", "."]);
        std::fs::write(root.join("file.txt"), "base\n").unwrap();
        std::fs::write(root.join("file.png"), b"working image").unwrap();
        let repo = git.open(root, 46).await.unwrap();
        let file = repo
            .changes
            .iter()
            .find(|file| file.path == "file.txt")
            .unwrap();
        assert!(ChangeScope::Staged.includes(file) && ChangeScope::Working.includes(file));
        let staged = git
            .diff_scoped(root, None, file, ChangeScope::Staged)
            .await
            .unwrap();
        let working = git
            .diff_scoped(root, None, file, ChangeScope::Working)
            .await
            .unwrap();
        let all = git
            .diff_scoped(root, None, file, ChangeScope::All)
            .await
            .unwrap();
        assert!(staged.contains("-base") && staged.contains("+index"));
        assert!(working.contains("-index") && working.contains("+base"));
        assert!(
            all.is_empty(),
            "net zero does not remove dual-state file record"
        );
        assert_eq!(
            git.image_file(root, Some(":"), "file.png")
                .await
                .unwrap()
                .unwrap(),
            b"index image"
        );
        assert_eq!(
            git.image_file(root, None, "file.png")
                .await
                .unwrap()
                .unwrap(),
            b"working image"
        );
        let empty = tempfile::tempdir().unwrap();
        command(&git, empty.path(), &["init", "-b", "main"]);
        std::fs::write(empty.path().join("new.txt"), "new\n").unwrap();
        command(&git, empty.path(), &["add", "."]);
        let unborn = git.open(empty.path(), 46).await.unwrap();
        let diff = git
            .diff_scoped(empty.path(), None, &unborn.changes[0], ChangeScope::All)
            .await
            .unwrap();
        assert!(diff.contains("+new"));
    });
}

#[test]
fn asynchronous_statistics_distinguish_text_binary_and_untracked() {
    let (directory, git) = fixture(2);
    futures_lite::future::block_on(async {
        let root = directory.path();
        std::fs::write(root.join("file.txt"), "base\n").unwrap();
        command(&git, root, &["add", "."]);
        command(&git, root, &["-c", NO_HOOKS, "commit", "-m", "base"]);
        std::fs::write(root.join("file.txt"), "one\ntwo\n").unwrap();
        std::fs::write(root.join("binary.dat"), b"a\0b").unwrap();
        std::fs::write(root.join("new.txt"), "new\nlines\n").unwrap();
        let repo = git.open(root, 46).await.unwrap();
        let stats = git
            .line_statistics(root, None, ChangeScope::All, &repo.changes)
            .await
            .unwrap();
        assert_eq!(stats.files["file.txt"], Some((2, 1)));
        assert_eq!(stats.files["new.txt"], Some((2, 0)));
        assert_eq!(stats.files["binary.dat"], None);
        assert_eq!((stats.added, stats.removed, stats.incomplete), (4, 1, true));
        command(&git, root, &["add", "file.txt"]);
        let stats = git
            .line_statistics(root, None, ChangeScope::Staged, &repo.changes)
            .await
            .unwrap();
        assert_eq!(stats.files.len(), 1);
        assert_eq!((stats.added, stats.removed), (2, 1));
    });
}

#[test]
fn worktree_visibility_refreshes_without_branch_changes() {
    let (directory, git) = fixture(1);
    let root = directory.path();
    let linked = tempfile::tempdir().unwrap();
    let path = linked.path().join("detached");

    futures_lite::future::block_on(async {
        let initial = git.open(root, 46).await.unwrap();
        assert!(!initial.has_other_worktrees);

        command(
            &git,
            root,
            &["worktree", "add", "--detach", path.to_str().unwrap()],
        );
        assert!(
            git.refresh(&initial, true, false, 46)
                .await
                .unwrap()
                .is_none()
        );
        let multiple = git
            .refresh(&initial, true, true, 46)
            .await
            .unwrap()
            .unwrap();
        assert!(multiple.has_other_worktrees);
        assert!(git.open(&path, 46).await.unwrap().has_other_worktrees);

        command(&git, root, &["worktree", "remove", path.to_str().unwrap()]);
        let single = git
            .refresh(&multiple, true, true, 46)
            .await
            .unwrap()
            .unwrap();
        assert!(!single.has_other_worktrees);
    });
}

#[test]
fn linked_worktrees_report_status_and_checkout_respects_occupancy() {
    let (directory, git) = fixture(3);
    let linked = tempfile::tempdir().unwrap();
    let path = linked.path().join("worktree with spaces");
    command(
        &git,
        directory.path(),
        &["worktree", "add", "-b", "linked", path.to_str().unwrap()],
    );
    std::fs::write(path.join("new.txt"), "worktree change").unwrap();
    futures_lite::future::block_on(async {
        let entries = git.worktrees(directory.path()).await.unwrap();
        assert_eq!(entries.len(), 2);
        let worktree = entries
            .iter()
            .find(|entry| entry.branch == "linked")
            .unwrap();
        assert_eq!(worktree.status, messages().worktree_changes(1));
        assert_eq!(git.open(&worktree.path, 46).await.unwrap().branch, "linked");
        assert!(git.switch_branch(directory.path(), "linked").await.is_err());
        assert_eq!(git.open(directory.path(), 46).await.unwrap().branch, "main");
    });
}
