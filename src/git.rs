use anyhow::{Context, Result, bail};
use futures_lite::io::{AsyncRead, AsyncReadExt};
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    time::Duration,
};

pub const IMAGE_FILE_LIMIT: u64 = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Git {
    pub executable: PathBuf,
    pub version: String,
}

#[derive(Clone, Debug)]
pub struct ChangedFile {
    pub path: String,
    pub original_path: Option<String>,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct Commit {
    pub id: String,
    pub short_id: String,
    pub author: String,
    pub date: String,
    pub subject: String,
    pub parents: Vec<String>,
    pub references: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Repository {
    pub root: PathBuf,
    pub branch: String,
    pub tracking: Option<BranchTracking>,
    pub changes: Vec<ChangedFile>,
    pub commits: Vec<Commit>,
    pub stashes: Vec<String>,
    pub branches: Vec<Branch>,
    pub graph: Vec<crate::graph::GraphRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    pub remote: bool,
    pub current: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchTracking {
    pub upstream: String,
    /// HEAD-only (ahead) and upstream-only (behind) commits. None means the
    /// configured upstream ref is unavailable locally, rather than zero.
    pub counts: Option<(usize, usize)>,
}

impl Git {
    pub async fn detect(configured: Option<PathBuf>) -> Result<Self> {
        let executable = if let Some(path) = configured {
            anyhow::ensure!(
                path.is_absolute() && path.is_file(),
                "Git 程序路径不存在：{}",
                path.display()
            );
            path
        } else {
            find_on_path()
                .context("未在系统 PATH 中找到 Git，请指定已有的 Git 程序或前往官网安装")?
        };
        let name = executable
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or_default();
        anyhow::ensure!(
            name.eq_ignore_ascii_case(if cfg!(windows) { "git.exe" } else { "git" }),
            "请选择 Git 可执行文件，而不是 git-bash 或安装程序"
        );
        let mut git = Self {
            executable,
            version: String::new(),
        };
        let output = git.run(None, ["--version"]).await?;
        let version = String::from_utf8_lossy(&output).trim().to_string();
        anyhow::ensure!(
            version.starts_with("git version "),
            "所选程序没有返回有效的 Git 版本"
        );
        git.version = version;
        Ok(git)
    }

    async fn run<I, S>(&self, root: Option<&Path>, args: I) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = std::process::Command::new(&self.executable);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        command
            .arg("--no-pager")
            .arg("--literal-pathspecs")
            .args(args)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("LC_ALL", "C");
        if let Some(root) = root {
            command.current_dir(root);
        }
        let mut command = async_process::Command::from(command);
        command
            .kill_on_drop(true)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = command.spawn().context("无法启动 Git 程序")?;
        let stdout = child.stdout.take().context("无法读取 Git 输出")?;
        let stderr = child.stderr.take().context("无法读取 Git 错误输出")?;
        let (status, stdout, stderr) = futures_lite::future::race(
            async {
                let (stdout, stderr) = futures_lite::future::try_zip(
                    read_limited(stdout, 8 * 1024 * 1024),
                    read_limited(stderr, 64 * 1024),
                )
                .await?;
                Ok((child.status().await?, stdout, stderr))
            },
            async {
                async_io::Timer::after(Duration::from_secs(20)).await;
                bail!("Git 读取超时，请稍后重试")
            },
        )
        .await?;
        if !status.success() {
            bail!("{}", String::from_utf8_lossy(&stderr).trim());
        }
        Ok(stdout)
    }

    pub async fn open(&self, path: &Path) -> Result<Repository> {
        let root = self
            .run(Some(path), ["rev-parse", "--show-toplevel"])
            .await
            .context("此目录不是可读取的 Git 工作目录")?;
        let root = PathBuf::from(String::from_utf8_lossy(&root).trim());
        let branch = self.run(Some(&root), ["branch", "--show-current"]).await?;
        let branch = String::from_utf8_lossy(&branch).trim().to_string();
        let tracking = self
            .run(
                Some(&root),
                [
                    "status",
                    "--porcelain=v2",
                    "--branch",
                    "--ahead-behind",
                    "-z",
                    "--untracked-files=no",
                ],
            )
            .await?;
        let status = self
            .run(
                Some(&root),
                ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
            )
            .await?;
        let has_head = self
            .run(Some(&root), ["rev-parse", "--verify", "--quiet", "HEAD"])
            .await
            .is_ok();
        let commits = if has_head {
            let log = self
                .run(
                    Some(&root),
                    [
                        "log",
                        "-n",
                        "200",
                        "-z",
                        "--date=format-local:%Y-%m-%d %H:%M",
                        "--format=%H%x00%h%x00%an%x00%ad%x00%s%x00%P%x00%D",
                        "--topo-order",
                    ],
                )
                .await?;
            parse_log(&log)
        } else {
            Vec::new()
        };
        let stashes = self
            .run(
                Some(&root),
                [
                    "stash",
                    "list",
                    "--date=format-local:%Y-%m-%d %H:%M",
                    "--format=%gd  %cd  %s",
                ],
            )
            .await?;
        let branches = self
            .run(
                Some(&root),
                [
                    "for-each-ref",
                    "--format=%(refname)%00%(symref)%00%(HEAD)",
                    "refs/heads",
                    "refs/remotes",
                ],
            )
            .await?;
        Ok(Repository {
            root,
            tracking: parse_tracking(&tracking),
            branch: if branch.is_empty() {
                "分离 HEAD".into()
            } else {
                branch
            },
            changes: parse_status(&status),
            graph: crate::graph::layout(&commits),
            commits,
            stashes: text_lines(&stashes),
            branches: parse_branches(&branches),
        })
    }

    pub async fn commit_files(&self, root: &Path, id: &str) -> Result<Vec<ChangedFile>> {
        let output = self
            .run(
                Some(root),
                [
                    "show",
                    "--format=",
                    "--diff-merges=first-parent",
                    "--name-status",
                    "--no-renames",
                    "-z",
                    id,
                    "--",
                ],
            )
            .await?;
        let fields: Vec<_> = output.split(|b| *b == 0).collect();
        Ok(fields
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| ChangedFile {
                original_path: None,
                status: String::from_utf8_lossy(pair[0]).trim().into(),
                path: String::from_utf8_lossy(pair[1]).into(),
            })
            .collect())
    }

    /// Read original bytes without text conversion. None selects the worktree;
    /// a missing file is distinct from a failed or oversized read.
    pub async fn image_file(
        &self,
        root: &Path,
        revision: Option<&str>,
        path: &str,
    ) -> Result<Option<Vec<u8>>> {
        if let Some(revision) = revision {
            let entry = self
                .run(Some(root), ["ls-tree", "-l", "-z", revision, "--", path])
                .await?;
            if entry.is_empty() {
                return Ok(None);
            }
            let header = entry
                .split(|byte| *byte == b'\t')
                .next()
                .unwrap_or_default();
            let header = std::str::from_utf8(header)?;
            let fields: Vec<_> = header.split_whitespace().collect();
            anyhow::ensure!(
                fields.len() == 4 && fields[0].starts_with("100") && fields[1] == "blob",
                "该版本不是普通图片文件"
            );
            anyhow::ensure!(
                fields[3].parse::<u64>()? <= IMAGE_FILE_LIMIT,
                "图片超过 8 MiB，暂不预览"
            );
            self.run(Some(root), ["cat-file", "blob", fields[2]])
                .await
                .map(Some)
        } else {
            use std::io::Read;
            let path = root.join(path);
            let metadata = match std::fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error).context("无法读取图片文件"),
            };
            anyhow::ensure!(metadata.is_file(), "此路径不是普通图片文件");
            anyhow::ensure!(
                metadata.len() <= IMAGE_FILE_LIMIT,
                "图片超过 8 MiB，暂不预览"
            );
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take(IMAGE_FILE_LIMIT + 1)
                .read_to_end(&mut bytes)?;
            anyhow::ensure!(
                bytes.len() as u64 <= IMAGE_FILE_LIMIT,
                "图片超过 8 MiB，暂不预览"
            );
            Ok(Some(bytes))
        }
    }

    pub async fn diff(
        &self,
        root: &Path,
        commit: Option<&str>,
        file: &ChangedFile,
    ) -> Result<String> {
        let output = if let Some(id) = commit {
            self.run(
                Some(root),
                [
                    "show",
                    "--format=",
                    "--diff-merges=first-parent",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--no-renames",
                    id,
                    "--",
                    &file.path,
                ],
            )
            .await?
        } else if file.status == "??" {
            let path = root.join(&file.path);
            let metadata = std::fs::metadata(&path)?;
            anyhow::ensure!(metadata.len() <= 1024 * 1024, "文件过大，暂不展开内容");
            let text = std::fs::read_to_string(&path).context("此文件无法作为文本显示")?;
            let count = text.lines().count();
            return Ok(format!(
                "diff --git a/{0} b/{0}\nnew file mode 100644\n--- /dev/null\n+++ b/{0}\n@@ -0,0 +1,{1} @@\n{2}",
                file.path,
                count,
                text.lines()
                    .map(|line| format!("+{line}\n"))
                    .collect::<String>()
            ));
        } else {
            self.run(
                Some(root),
                vec![
                    OsString::from("diff"),
                    "--no-ext-diff".into(),
                    "--no-textconv".into(),
                    "HEAD".into(),
                    "--".into(),
                    file.path.clone().into(),
                ],
            )
            .await?
        };
        anyhow::ensure!(output.len() <= 2 * 1024 * 1024, "差异过大，暂不展开内容");
        Ok(String::from_utf8_lossy(&output).into())
    }
}

async fn read_limited(reader: impl AsyncRead + Unpin, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes).await?;
    anyhow::ensure!(
        bytes.len() as u64 <= limit,
        "Git 输出超过本轮读取上限，请缩小查看范围"
    );
    Ok(bytes)
}

fn find_on_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(if cfg!(windows) { "git.exe" } else { "git" }))
        .find(|file| file.is_file())
}
fn text_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::to_owned)
        .collect()
}
fn parse_log(bytes: &[u8]) -> Vec<Commit> {
    let fields: Vec<_> = bytes.split(|b| *b == 0).collect();
    fields
        .as_chunks::<7>()
        .0
        .iter()
        .map(|f| Commit {
            id: String::from_utf8_lossy(f[0]).into(),
            short_id: String::from_utf8_lossy(f[1]).into(),
            author: String::from_utf8_lossy(f[2]).into(),
            date: String::from_utf8_lossy(f[3]).into(),
            subject: String::from_utf8_lossy(f[4]).into(),
            parents: String::from_utf8_lossy(f[5])
                .split_whitespace()
                .map(str::to_owned)
                .collect(),
            references: String::from_utf8_lossy(f[6])
                .split(", ")
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
        })
        .collect()
}
fn parse_status(bytes: &[u8]) -> Vec<ChangedFile> {
    let mut records = bytes.split(|b| *b == 0);
    let mut files = Vec::new();
    while let Some(record) = records.next() {
        if record.len() < 4 {
            continue;
        }
        let status = String::from_utf8_lossy(&record[..2]).into_owned();
        let path = String::from_utf8_lossy(&record[3..]).into_owned();
        let original_path = if status.contains('R') || status.contains('C') {
            records
                .next()
                .map(|path| String::from_utf8_lossy(path).into_owned())
        } else {
            None
        };
        files.push(ChangedFile {
            path,
            original_path,
            status,
        });
    }
    files
}

fn parse_branches(bytes: &[u8]) -> Vec<Branch> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\0');
            let reference = fields.next()?;
            // origin/HEAD is a symbolic alias, not a separate remote branch.
            if !fields.next()?.is_empty() {
                return None;
            }
            let current = fields.next()? == "*";
            let (name, remote) = if let Some(name) = reference.strip_prefix("refs/heads/") {
                (name, false)
            } else {
                (reference.strip_prefix("refs/remotes/")?, true)
            };
            Some(Branch {
                name: name.into(),
                remote,
                current,
            })
        })
        .collect()
}

fn parse_tracking(bytes: &[u8]) -> Option<BranchTracking> {
    let mut upstream = None;
    let mut counts = None;
    for header in bytes.split(|b| *b == 0) {
        let header = String::from_utf8_lossy(header);
        if let Some(value) = header.strip_prefix("# branch.upstream ") {
            upstream = Some(value.to_string());
        } else if let Some(value) = header.strip_prefix("# branch.ab ") {
            let mut parts = value.split_whitespace();
            counts = parts
                .next()
                .and_then(|n| n.strip_prefix('+'))
                .and_then(|n| n.parse().ok())
                .zip(
                    parts
                        .next()
                        .and_then(|n| n.strip_prefix('-'))
                        .and_then(|n| n.parse().ok()),
                );
        }
    }
    upstream.map(|upstream| BranchTracking { upstream, counts })
}

#[cfg(test)]
mod tests {
    #[test]
    fn branches_use_ref_namespaces_and_skip_symbolic_aliases() {
        let branches = super::parse_branches(
            b"refs/heads/origin/local\0\0 \nrefs/heads/main\0\0*\nrefs/remotes/origin/HEAD\0refs/remotes/origin/main\0 \nrefs/remotes/origin/main\0\0 \n",
        );
        assert_eq!(branches.len(), 3);
        assert_eq!(branches[0].name, "origin/local");
        assert!(!branches[0].remote);
        assert!(branches[1].current);
        assert!(branches[2].remote);
        assert_eq!(branches[2].name, "origin/main");
    }

    use super::*;
    #[test]
    fn tracking_counts_keep_push_pull_order_and_unknown_state() {
        assert_eq!(
            parse_tracking(b"# branch.upstream origin/main\0# branch.ab +3 -5\0"),
            Some(BranchTracking {
                upstream: "origin/main".into(),
                counts: Some((3, 5))
            })
        );
        assert_eq!(
            parse_tracking(b"# branch.upstream origin/main\0# branch.ab +0 -0\0")
                .unwrap()
                .counts,
            Some((0, 0))
        );
        assert_eq!(
            parse_tracking(b"# branch.upstream origin/gone\0")
                .unwrap()
                .counts,
            None
        );
        assert!(parse_tracking(b"# branch.head (detached)\0").is_none());
    }
    #[test]
    fn nul_status_preserves_paths_and_rename_boundaries() {
        let files =
            parse_status(" M src/a b.rs\0R  新文件.rs\0旧文件.rs\0?? :special.txt\0".as_bytes());
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].path, "src/a b.rs");
        assert_eq!(files[1].path, "新文件.rs");
        assert_eq!(files[1].original_path.as_deref(), Some("旧文件.rs"));
        assert_eq!(files[2].path, ":special.txt");
    }
    #[test]
    fn log_preserves_subject_and_author_fields() {
        let commits = parse_log(
            b"abc\0abc\0Author\x002026-11-06 12:00\0hello | world\0parent\0HEAD -> main, tag: v1\0",
        );
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].subject, "hello | world");
        assert_eq!(commits[0].parents, ["parent"]);
        assert_eq!(commits[0].references.len(), 2);
    }

    #[test]
    fn image_versions_read_raw_bytes_across_commits_renames_and_deletion() {
        futures_lite::future::block_on(async {
            let git = Git::detect(None).await.expect("测试需要系统 Git");
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path();
            let old = b"\x89PNG\0old\xff";
            let staged = b"\x89PNG\0staged\xfe";
            let modified = b"\x89PNG\0worktree\xfd";
            git.run(Some(root), ["init", "-b", "main"]).await.unwrap();
            git.run(Some(root), ["config", "user.name", "githly test"])
                .await
                .unwrap();
            git.run(Some(root), ["config", "user.email", "test@example.invalid"])
                .await
                .unwrap();
            let commit = [
                "-c",
                "core.hooksPath=NUL",
                "commit",
                "--no-gpg-sign",
                "-m",
                "image",
            ];
            std::fs::write(root.join("旧 图片.png"), old).unwrap();
            assert_eq!(
                git.image_file(root, None, "旧 图片.png")
                    .await
                    .unwrap()
                    .unwrap(),
                old
            );
            git.run(Some(root), ["add", "--", "."]).await.unwrap();
            git.run(Some(root), commit).await.unwrap();
            let initial = git.open(root).await.unwrap().commits.remove(0);
            assert!(initial.parents.is_empty());
            assert!(
                git.image_file(root, Some(&initial.id), "missing.png")
                    .await
                    .unwrap()
                    .is_none()
            );

            std::fs::write(root.join("旧 图片.png"), staged).unwrap();
            git.run(Some(root), ["add", "--", "."]).await.unwrap();
            std::fs::write(root.join("旧 图片.png"), modified).unwrap();
            assert_eq!(
                git.image_file(root, Some("HEAD"), "旧 图片.png")
                    .await
                    .unwrap()
                    .unwrap(),
                old
            );
            assert_eq!(
                git.image_file(root, None, "旧 图片.png")
                    .await
                    .unwrap()
                    .unwrap(),
                modified
            );
            git.run(Some(root), commit).await.unwrap();
            assert_eq!(
                git.image_file(root, Some("HEAD"), "旧 图片.png")
                    .await
                    .unwrap()
                    .unwrap(),
                staged
            );
            assert_eq!(
                git.image_file(root, Some("HEAD^"), "旧 图片.png")
                    .await
                    .unwrap()
                    .unwrap(),
                old
            );

            std::fs::write(root.join("旧 图片.png"), staged).unwrap();
            git.run(Some(root), ["mv", "--", "旧 图片.png", "新 图片.png"])
                .await
                .unwrap();
            let repository = git.open(root).await.unwrap();
            let renamed = &repository.changes[0];
            assert_eq!(renamed.path, "新 图片.png");
            assert_eq!(renamed.original_path.as_deref(), Some("旧 图片.png"));
            assert_eq!(
                git.image_file(
                    root,
                    Some("HEAD"),
                    renamed.original_path.as_deref().unwrap()
                )
                .await
                .unwrap()
                .unwrap(),
                staged
            );
            assert_eq!(
                git.image_file(root, None, &renamed.path)
                    .await
                    .unwrap()
                    .unwrap(),
                staged
            );
            git.run(Some(root), commit).await.unwrap();
            git.run(Some(root), ["rm", "--", "新 图片.png"])
                .await
                .unwrap();
            assert!(
                git.image_file(root, None, "新 图片.png")
                    .await
                    .unwrap()
                    .is_none()
            );
            git.run(Some(root), commit).await.unwrap();
            assert!(
                git.image_file(root, Some("HEAD"), "新 图片.png")
                    .await
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                git.image_file(root, Some("HEAD^"), "新 图片.png")
                    .await
                    .unwrap()
                    .unwrap(),
                staged
            );

            std::fs::File::create(root.join("large.png"))
                .unwrap()
                .set_len(IMAGE_FILE_LIMIT + 1)
                .unwrap();
            assert!(
                git.image_file(root, None, "large.png")
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("8 MiB")
            );
            git.run(Some(root), ["add", "--", "large.png"])
                .await
                .unwrap();
            git.run(Some(root), commit).await.unwrap();
            assert!(
                git.image_file(root, Some("HEAD"), "large.png")
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("8 MiB")
            );
        });
    }

    #[test]
    fn reads_real_repository_and_keeps_file_diffs_distinct() {
        futures_lite::future::block_on(async {
            let git = Git::detect(None).await.expect("测试需要系统 Git");
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path();
            let command = |args: &[&str]| {
                let mut command = std::process::Command::new(&git.executable);
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    command.creation_flags(0x08000000);
                }
                let output = command.current_dir(root).args(args).output().unwrap();
                assert!(
                    output.status.success(),
                    "git {args:?} failed ({}): {} {}",
                    output.status,
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            };
            command(&["init", "-b", "main"]);
            command(&["config", "user.name", "githly test"]);
            command(&["config", "user.email", "test@example.invalid"]);
            std::fs::write(root.join("文件 a.txt"), "first file\n").unwrap();
            std::fs::write(root.join("b.txt"), "second file\n").unwrap();
            command(&["add", "--", "."]);
            command(&[
                "-c",
                "core.hooksPath=NUL",
                "commit",
                "-m",
                "初次提交 | test",
            ]);
            let repo = git.open(root).await.unwrap();
            assert_eq!(repo.branch, "main");
            assert!(repo.tracking.is_none());
            assert_eq!(repo.commits.len(), 1);
            assert_eq!(repo.commits[0].subject, "初次提交 | test");
            assert!(repo.changes.is_empty());
            let files = git.commit_files(root, &repo.commits[0].id).await.unwrap();
            assert_eq!(files.len(), 2);
            let a = files.iter().find(|f| f.path == "文件 a.txt").unwrap();
            let b = files.iter().find(|f| f.path == "b.txt").unwrap();
            let patch_a = git.diff(root, Some(&repo.commits[0].id), a).await.unwrap();
            let patch_b = git.diff(root, Some(&repo.commits[0].id), b).await.unwrap();
            assert!(patch_a.contains("+first file"));
            assert!(!patch_b.contains("first file"));
            assert!(patch_b.contains("+second file"));
            assert_eq!(
                gpui_kit::component::diff::DiffFile::parse(&patch_a)
                    .unwrap()
                    .len(),
                1
            );
            std::fs::write(root.join("b.txt"), "changed second file\n").unwrap();
            let repo = git.open(root).await.unwrap();
            assert_eq!(repo.changes.len(), 1);
            let patch = git.diff(root, None, &repo.changes[0]).await.unwrap();
            assert!(patch.contains("+changed second file"));
            // A local upstream exercises ahead/behind without any network access.
            command(&["branch", "upstream"]);
            command(&["branch", "--set-upstream-to=upstream", "main"]);
            assert_eq!(
                git.open(root).await.unwrap().tracking.unwrap().counts,
                Some((0, 0))
            );
            command(&[
                "-c",
                "core.hooksPath=NUL",
                "commit",
                "--allow-empty",
                "-m",
                "local only",
            ]);
            assert_eq!(
                git.open(root).await.unwrap().tracking.unwrap().counts,
                Some((1, 0))
            );
            command(&["switch", "upstream"]);
            command(&[
                "-c",
                "core.hooksPath=NUL",
                "commit",
                "--allow-empty",
                "-m",
                "upstream only",
            ]);
            command(&["switch", "main"]);
            assert_eq!(
                git.open(root).await.unwrap().tracking.unwrap().counts,
                Some((1, 1))
            );
        });
    }

    #[test]
    fn missing_configured_git_is_reported_without_falling_back() {
        let directory = tempfile::tempdir().unwrap();
        assert!(
            futures_lite::future::block_on(Git::detect(Some(directory.path().join("git.exe"))))
                .is_err()
        );
    }

    #[test]
    fn oversized_output_is_rejected_at_the_reader() {
        let input = futures_lite::io::Cursor::new(vec![b'x'; 128]);
        assert!(futures_lite::future::block_on(read_limited(input, 32)).is_err());
    }
}
