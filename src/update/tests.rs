use super::*;
use std::collections::VecDeque;

struct Reply {
    url: String,
    status: u32,
    bytes: Vec<u8>,
    broken: bool,
}
struct Mock {
    replies: VecDeque<Reply>,
    calls: Vec<String>,
}
impl Mock {
    fn new(replies: Vec<Reply>) -> Self {
        Self {
            replies: replies.into(),
            calls: vec![],
        }
    }
}
impl Fetch for Mock {
    fn get(&mut self, url: &str, output: &mut dyn Write, limit: u64) -> Result<Response> {
        self.calls.push(url.into());
        let reply = self.replies.pop_front().expect("unexpected extra request");
        assert_eq!(reply.url, url);
        ensure!(reply.bytes.len() as u64 <= limit, "oversized");
        output.write_all(&reply.bytes)?;
        ensure!(!reply.broken, "connection interrupted");
        Ok(Response {
            status: reply.status,
            retry_after: Some(120),
            rate_reset: Some(1500),
        })
    }
}
fn reply(url: impl Into<String>, status: u32, bytes: impl Into<Vec<u8>>) -> Reply {
    Reply {
        url: url.into(),
        status,
        bytes: bytes.into(),
        broken: false,
    }
}
fn release(version: &str) -> Reply {
    reply(
        API,
        200,
        format!(r#"{{"tag_name":"v{version}","draft":false,"prerelease":false}}"#),
    )
}
fn checksum(content: &[u8]) -> String {
    Sha256::digest(content)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn manifest(distribution: Distribution, version: &str, content: &[u8]) -> Manifest {
    let version = Version::parse(version).unwrap();
    Manifest {
        version: version.to_string(),
        notes_url: format!("{RELEASES}/tag/v{version}"),
        notes: "Release notes in the app".into(),
        installer: Installer {
            url: format!(
                "{RELEASES}/download/v{version}/{}",
                distribution.package_name(&version)
            ),
            size: content.len() as u64,
            sha256: checksum(content),
        },
    }
}
fn available(content: &[u8]) -> Available {
    Available {
        manifest: manifest(Distribution::Native, "0.2.0", content),
        distribution: Distribution::Native,
    }
}

#[test]
fn no_update_never_downloads_a_manifest() {
    let mut fetch = Mock::new(vec![release("0.1.0")]);
    let result = check_with(
        &mut fetch,
        &mut Preferences::default(),
        "0.1.0",
        1000,
        Distribution::Native,
    )
    .unwrap();
    assert!(result.is_none());
    assert_eq!(fetch.calls.len(), 1);
}

#[test]
fn github_asset_digest_and_notes_are_used_without_a_second_request() {
    let expected = available(b"verified");
    let installer = &expected.manifest.installer;
    let body = serde_json::json!({"tag_name":"v0.2.0","draft":false,"prerelease":false,"body":"Fixes #20",
        "assets":[{"browser_download_url":installer.url,"size":installer.size,"digest":format!("sha256:{}",installer.sha256)}]});
    let mut fetch = Mock::new(vec![reply(API, 200, serde_json::to_vec(&body).unwrap())]);
    let update = check_with(
        &mut fetch,
        &mut Preferences::default(),
        "0.1.0",
        1000,
        Distribution::Native,
    )
    .unwrap()
    .unwrap();
    assert_eq!(update.manifest.notes, "Fixes #20");
    assert_eq!(update.manifest.installer.sha256, installer.sha256);
}

#[test]
fn rate_limit_uses_official_manifest_and_preserves_server_cooldown() {
    let distribution = Distribution::Native;
    let url = format!(
        "{RELEASES}/latest/download/{}",
        distribution.manifest_name()
    );
    let bytes = serde_json::to_vec(&manifest(distribution, "0.2.0", b"package")).unwrap();
    let mut prefs = Preferences::default();
    let mut fetch = Mock::new(vec![
        reply(API, 429, vec![]),
        reply(&url, 200, bytes.clone()),
    ]);
    let found = check_with(&mut fetch, &mut prefs, "0.1.0", 1000, distribution)
        .unwrap()
        .unwrap();
    assert_eq!(found.manifest.notes, "Release notes in the app");
    assert_eq!(prefs.api_retry_after, 1500);
    let mut fetch = Mock::new(vec![reply(&url, 200, bytes)]);
    check_with(&mut fetch, &mut prefs, "0.1.0", 1100, distribution).unwrap();
    let mut failed = Mock::new(vec![reply(&url, 503, vec![])]);
    assert!(check_with(&mut failed, &mut prefs, "0.1.0", 1200, distribution).is_err());
    assert_eq!(prefs.api_retry_after, 1500);
}

#[test]
fn rejects_stale_manifest_for_known_release() {
    let mode = Distribution::Native;
    let url = format!("{RELEASES}/download/v0.2.0/{}", mode.manifest_name());
    let mut fetch = Mock::new(vec![
        release("0.2.0"),
        reply(
            &url,
            200,
            serde_json::to_vec(&manifest(mode, "0.1.0", b"stale")).unwrap(),
        ),
    ]);
    assert!(check_with(&mut fetch, &mut Preferences::default(), "0.1.0", 1000, mode).is_err());
}

#[test]
fn downloads_retry_builtin_mirrors_with_verified_bytes_and_progress() {
    let directory = tempfile::tempdir().unwrap();
    let content = b"test installer";
    let update = available(content);
    let direct = &update.manifest.installer.url;
    let mut fetch = Mock::new(vec![
        reply(direct, 503, vec![]),
        reply(
            format!("{}{direct}", MIRRORS[0]),
            200,
            b"wrong bytes".to_vec(),
        ),
        reply(format!("{}{direct}", MIRRORS[1]), 200, content.to_vec()),
    ]);
    let received = AtomicU64::new(500);
    let path = download_with(&mut fetch, &update, directory.path(), &received).unwrap();
    assert_eq!(received.load(Ordering::Relaxed), content.len() as u64);
    assert_eq!(std::fs::read(path).unwrap(), content);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn failed_downloads_leave_no_partial_file() {
    let directory = tempfile::tempdir().unwrap();
    let update = available(b"expected");
    let direct = &update.manifest.installer.url;
    let mut broken = reply(direct, 200, b"exp".to_vec());
    broken.broken = true;
    let mut fetch = Mock::new(vec![
        broken,
        reply(
            format!("{}{direct}", MIRRORS[0]),
            200,
            b"expectedly long".to_vec(),
        ),
        reply(format!("{}{direct}", MIRRORS[1]), 200, b"mismatch".to_vec()),
    ]);
    assert!(download_with(&mut fetch, &update, directory.path(), &AtomicU64::new(0)).is_err());
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn rejects_wrong_origin_or_distribution() {
    let mut update = available(b"installer");
    update.manifest.installer.url = "https://other.example/payload.exe".into();
    assert!(validate(&update.manifest, update.distribution).is_err());
    update.manifest.installer.url = format!(
        "{RELEASES}/download/v0.2.0/Githly-0.2.0-{}",
        if cfg!(windows) {
            "linux-x64.deb"
        } else {
            "windows-x64-setup.exe"
        }
    );
    assert!(validate(&update.manifest, update.distribution).is_err());
    update = available(b"installer");
    update.manifest.installer.size = MAX_INSTALLER + 1;
    assert!(validate(&update.manifest, update.distribution).is_err());
}

#[test]
fn legacy_update_settings_keep_only_the_server_cooldown() {
    let prefs: Preferences = serde_json::from_str(
        r#"{"enabled":false,"interval_hours":72,"mirror":"https://example.org/","last_check":100,"api_retry_after":123}"#,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(prefs).unwrap(),
        serde_json::json!({"api_retry_after":123})
    );
}

#[cfg(windows)]
#[test]
fn portable_channel_never_accepts_the_setup_manifest() {
    let mode = Distribution::Portable;
    let zip = manifest(mode, "0.2.0", b"portable");
    assert_eq!(mode.manifest_name(), "update-windows-x64-portable.json");
    assert!(zip.installer.url.ends_with("-windows-x64.zip"));
    assert!(validate(&zip, Distribution::Native).is_err());
    assert!(validate(&available(b"setup").manifest, mode).is_err());
}

#[cfg(windows)]
#[test]
fn portable_upgrade_replaces_the_running_copy_in_its_own_directory() {
    const CHILD: &str = "GITHLY_TEST_PORTABLE_ARCHIVE";
    let mode = Distribution::Portable;
    if let Some(archive) = std::env::var_os(CHILD) {
        let path = PathBuf::from(archive);
        assert_eq!(
            std::env::current_exe().unwrap().canonicalize().unwrap(),
            path.with_file_name("My portable Githly.exe")
                .canonicalize()
                .unwrap()
        );
        let bytes = std::fs::read(&path).unwrap();
        let available = Available {
            manifest: manifest(mode, "0.2.0", &bytes),
            distribution: mode,
        };
        assert_eq!(Distribution::current().unwrap(), mode);
        install(&path, &available).unwrap();
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("My portable Githly.exe");
    std::fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
    // Another app's uninstaller in the same directory must not change the channel.
    let unrelated = dir.path().join("unins000.exe");
    std::fs::write(&unrelated, b"keep").unwrap();
    let archive = dir.path().join("update.zip");
    let mut zip = zip::ZipWriter::new(File::create(&archive).unwrap());
    zip.start_file("Githly.exe", zip::write::SimpleFileOptions::default())
        .unwrap();
    let replacement = std::fs::read(
        PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/whoami.exe"),
    )
    .unwrap();
    zip.write_all(&replacement).unwrap();
    drop(zip.finish().unwrap());
    let status = std::process::Command::new(&exe)
        .args([
            "--exact",
            "update::tests::portable_upgrade_replaces_the_running_copy_in_its_own_directory",
        ])
        .env(CHILD, &archive)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(std::fs::read(exe).unwrap(), replacement);
    assert_eq!(std::fs::read(unrelated).unwrap(), b"keep");
}
