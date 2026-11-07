use super::*;
use std::{fmt::Write, io::Read};

// Use Git's C-style quoted-path syntax. Spaces and UTF-8 are literal inside
// quotes, and three-digit octal escapes are unambiguous for control bytes.
fn quoted_path(prefix: char, name: &str) -> String {
    let mut path = format!("\"{prefix}/");
    for ch in name.chars() {
        match ch {
            '"' | '\\' => {
                path.push('\\');
                path.push(ch);
            }
            ch if ch.is_ascii_control() => write!(path, "\\{:03o}", ch as u32).unwrap(),
            ch => path.push(ch),
        }
    }
    path.push('"');
    path
}

pub(super) fn patch(root: &Path, name: &str) -> Result<String> {
    const FILE_LIMIT: u64 = 1024 * 1024;
    let path = root.join(name);
    let metadata = std::fs::symlink_metadata(&path)?;
    let (mode, text) = if metadata.is_symlink() {
        let target = std::fs::read_link(&path)?;
        (
            "120000",
            target.to_str().context(messages().non_text())?.to_owned(),
        )
    } else {
        anyhow::ensure!(metadata.is_file(), messages().non_text());
        anyhow::ensure!(metadata.len() <= FILE_LIMIT, messages().file_too_large());
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        std::fs::File::open(path)?
            .take(FILE_LIMIT + 1)
            .read_to_end(&mut bytes)?;
        anyhow::ensure!(
            bytes.len() as u64 <= FILE_LIMIT,
            messages().file_too_large()
        );
        anyhow::ensure!(!bytes.contains(&0), messages().non_text());
        #[cfg(unix)]
        let executable = {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode() & 0o100 != 0
        };
        #[cfg(not(unix))]
        let executable = false;
        (
            if executable { "100755" } else { "100644" },
            String::from_utf8(bytes).context(messages().non_text())?,
        )
    };
    let old = quoted_path('a', name);
    let new = quoted_path('b', name);
    let mut output = format!("diff --git {old} {new}\nnew file mode {mode}\n");
    if !text.is_empty() {
        let count = text.split_inclusive('\n').count();
        writeln!(output, "--- /dev/null\n+++ {new}\n@@ -0,0 +1,{count} @@")?;
        let ending = if text.ends_with('\n') {
            ""
        } else {
            "\n\\ No newline at end of file\n"
        };
        let length = output.len() + text.len() + count + ending.len();
        anyhow::ensure!(length <= DIFF_LIMIT, messages().diff_too_large());
        output.reserve(length - output.len());
        for line in text.split_inclusive('\n') {
            output.push('+');
            output.push_str(line);
        }
        output.push_str(ending);
    }
    Ok(output)
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
            let output = patch(root, name).unwrap();
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

    #[test]
    fn paths_with_control_characters_and_quotes_round_trip_through_the_viewer() {
        let name = "quote\"\\\r\n\t name";
        let old = quoted_path('a', name);
        let new = quoted_path('b', name);
        let output = format!(
            "diff --git {old} {new}\nnew file mode 100644\n--- /dev/null\n+++ {new}\n@@ -0,0 +1 @@\n+x\n"
        );
        assert_eq!(DiffFile::parse(&output).unwrap()[0].path().as_ref(), name);
    }

    #[test]
    fn expanded_patch_obeys_the_diff_limit() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("lines"), vec![b'\n'; 1024 * 1024]).unwrap();
        assert_eq!(
            patch(root.path(), "lines").unwrap_err().to_string(),
            messages().diff_too_large()
        );
    }
}
