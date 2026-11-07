use super::*;

pub(super) async fn patch(git: &Git, root: &Path, name: &Path) -> Result<String> {
    let path = root.join(name);
    let metadata = blocking::unblock(move || std::fs::symlink_metadata(path)).await?;
    anyhow::ensure!(
        metadata.is_file() || metadata.is_symlink(),
        LocalizedText::new(Locale::non_text)
    );
    anyhow::ensure!(
        metadata.len() <= 1024 * 1024,
        LocalizedText::new(Locale::file_too_large)
    );

    // Outside repository context, Git compares raw worktree bytes without applying
    // repository clean filters. Quote non-UTF-8 path bytes in the textual header.
    let args = [
        "--git-dir=/dev/null",
        "-c",
        "core.autocrlf=false",
        "-c",
        "core.quotePath=true",
        "-c",
        "core.attributesFile=/dev/null",
        "diff",
        "--no-index",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--text",
        "--src-prefix=a/",
        "--dst-prefix=b/",
        "--",
        "/dev/null",
    ];
    // Even after --, no-index treats a literal "-" operand as standard input.
    let operand = if name == Path::new("-") {
        Path::new("./-")
    } else {
        name
    };
    let bytes = git
        .run_with_stdout(
            Some(root),
            args.map(OsStr::new)
                .into_iter()
                .chain([operand.as_os_str()]),
            |status, bytes: &Vec<u8>| status.code() == Some(1) && bytes.starts_with(b"diff --git "),
            |stdout| async move {
                let mut bytes = Vec::new();
                stdout
                    .take(DIFF_LIMIT as u64 + 1)
                    .read_to_end(&mut bytes)
                    .await?;
                anyhow::ensure!(
                    bytes.len() <= DIFF_LIMIT,
                    LocalizedText::new(Locale::diff_too_large)
                );
                Ok(bytes)
            },
        )
        .await?;
    anyhow::ensure!(!bytes.contains(&0), LocalizedText::new(Locale::non_text));
    let mut patch =
        String::from_utf8(bytes).with_context(|| LocalizedText::new(Locale::non_text))?;
    if name == Path::new("-") {
        // Git also copies the stdin-disambiguating "./" into its headers.
        // git apply rejects that path component. Leave all hunk bytes untouched.
        patch = patch
            .replacen("diff --git a/./- b/./-\n", "diff --git a/- b/-\n", 1)
            .replacen("\n+++ b/./-\n", "\n+++ b/-\n", 1);
    }
    Ok(patch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::component::diff::DiffFile;

    #[test]
    fn new_file_patches_apply_with_original_bytes_and_path() {
        let (repo, git) = super::super::history_tests::fixture(0);
        let root = repo.path();
        let command = |args: &[&str]| super::super::history_tests::command(&git, root, args);
        command(&["config", "core.autocrlf", "false"]);
        for text in ["", "one\r\ntwo\r\n", "one\r\ntwo", "\n", "last\r"] {
            let name = "未跟踪 a b.txt";
            let path = root.join(name);
            std::fs::write(&path, text).unwrap();
            let output =
                futures_lite::future::block_on(patch(&git, root, Path::new(name))).unwrap();
            let files = DiffFile::parse(&output).unwrap();
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].path().as_ref(), name);
            assert_eq!(files[0].additions(), text.split_inclusive('\n').count());
            std::fs::remove_file(&path).unwrap();
            std::fs::write(root.join("test.patch"), &output).unwrap();
            command(&["apply", "--whitespace=nowarn", "--", "test.patch"]);
            assert_eq!(std::fs::read(path).unwrap(), text.as_bytes());
        }
    }

    #[cfg(unix)]
    #[test]
    fn control_characters_and_quotes_round_trip_through_git_and_the_viewer() {
        let (repo, git) = super::super::history_tests::fixture(0);
        let name = "quote\"\\\r\n\t name";
        std::fs::write(repo.path().join(name), b"x\n").unwrap();
        let output =
            futures_lite::future::block_on(patch(&git, repo.path(), Path::new(name))).unwrap();
        assert_eq!(DiffFile::parse(&output).unwrap()[0].path().as_ref(), name);
    }

    #[test]
    fn expanded_patch_obeys_the_diff_limit() {
        let (root, git) = super::super::history_tests::fixture(0);
        std::fs::write(root.path().join("lines"), vec![b'\n'; 1024 * 1024]).unwrap();
        assert_eq!(
            futures_lite::future::block_on(patch(&git, root.path(), Path::new("lines")))
                .unwrap_err()
                .to_string(),
            messages().diff_too_large()
        );
    }

    #[test]
    fn untracked_diff_reads_a_literal_dash_and_preserves_raw_text() {
        let (repo, git) = super::super::history_tests::fixture(0);
        let root = repo.path();
        std::fs::write(root.join(".gitattributes"), "* text eol=lf\n").unwrap();
        let path = Path::new("-");
        let read = || futures_lite::future::block_on(patch(&git, root, path));
        std::fs::write(root.join(path), b"one\r\ntwo\r\n").unwrap();
        let patch = read().unwrap();
        assert!(patch.contains("+one\r\n+two\r\n"));
        assert_eq!(DiffFile::parse(&patch).unwrap()[0].path().as_ref(), "-");
        std::fs::write(root.join("dash.patch"), patch).unwrap();
        std::fs::remove_file(root.join(path)).unwrap();
        super::super::history_tests::command(
            &git,
            root,
            &["apply", "--whitespace=nowarn", "--", "dash.patch"],
        );
        assert_eq!(std::fs::read(root.join(path)).unwrap(), b"one\r\ntwo\r\n");
        for binary in [b"a\0b".as_slice(), b"\xff"] {
            std::fs::write(root.join(path), binary).unwrap();
            assert_eq!(read().unwrap_err().to_string(), messages().non_text());
        }
        std::fs::write(root.join(path), vec![b'a'; 1024 * 1024 + 1]).unwrap();
        assert_eq!(read().unwrap_err().to_string(), messages().file_too_large());
    }

    #[cfg(unix)]
    #[test]
    fn untracked_symlink_shows_its_target_path() {
        let (repo, git) = super::super::history_tests::fixture(0);
        std::os::unix::fs::symlink("absent", repo.path().join("link")).unwrap();
        let patch =
            futures_lite::future::block_on(patch(&git, repo.path(), Path::new("link"))).unwrap();
        assert!(patch.contains("new file mode 120000"));
        assert!(patch.contains("+absent\n"));
    }
}
