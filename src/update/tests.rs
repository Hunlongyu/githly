use super::*;
use std::{collections::VecDeque, sync::Mutex};

struct Reply {
    url: String,
    status: u32,
    bytes: Vec<u8>,
    broken: bool,
}
struct Mock {
    replies: Mutex<VecDeque<Reply>>,
}
impl Mock {
    fn new(replies: Vec<Reply>) -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(replies.into()),
        })
    }
}
impl Fetch for Mock {
    fn get(&self, url: &str, output: &mut dyn Write, limit: u64) -> Result<Response> {
        let reply = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected extra request");
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
        format!(
            r#"{{"tag_name":"v{version}","draft":false,"prerelease":false,"created_at":"2026-01-01","assets":[]}}"#
        ),
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
    let fetch = Mock::new(vec![release("0.1.0")]);
    let result = check_with(
        fetch.clone(),
        &mut Preferences::default(),
        "0.1.0",
        1000,
        Distribution::Native,
    )
    .unwrap();
    assert!(result.is_none());
    assert!(fetch.replies.lock().unwrap().is_empty());
}

#[test]
fn github_selects_the_exact_distribution_and_digest_in_one_request() {
    let version = Version::parse("0.2.0").unwrap();
    let packages = [
        (Distribution::Native, b"native installer".as_slice()),
        #[cfg(any(windows, target_os = "linux"))]
        (Distribution::Portable, b"portable".as_slice()),
    ];
    let assets: Vec<_> = packages.iter().enumerate().map(|(index, (mode, bytes))| {
        let installer = manifest(*mode, "0.2.0", bytes).installer;
        serde_json::json!({
            "name": mode.package_name(&version),
            "url": format!("https://api.github.com/repos/Hunlongyu/githly/releases/assets/{index}"),
            "browser_download_url": installer.url,
            "size": installer.size,
            "digest": format!("sha256:{}", installer.sha256),
        })
    }).collect();
    let release = serde_json::json!({
        "tag_name": "v0.2.0", "draft": false, "prerelease": false,
        "body": "Fixes #20", "created_at": "2026-01-01T00:00:00Z", "assets": assets,
    });
    for (mode, content) in packages {
        let fetch = Mock::new(vec![reply(API, 200, serde_json::to_vec(&release).unwrap())]);
        let update = check_with(fetch, &mut Preferences::default(), "0.1.0", 1000, mode)
            .unwrap()
            .unwrap();
        assert_eq!(update.distribution, mode);
        assert_eq!(update.manifest.notes, "Fixes #20");
        assert_eq!(update.manifest.installer.sha256, checksum(content));
        assert_eq!(update.manifest.installer.size, content.len() as u64);
        assert!(
            update
                .manifest
                .installer
                .url
                .ends_with(&mode.package_name(&version))
        );
    }
}

#[test]
fn draft_and_prerelease_responses_cannot_offer_a_stable_update() {
    for flag in ["draft", "prerelease"] {
        let mut latest = release("0.2.0");
        let mut body: serde_json::Value = serde_json::from_slice(&latest.bytes).unwrap();
        body[flag] = true.into();
        latest.bytes = serde_json::to_vec(&body).unwrap();
        assert!(
            check_with(
                Mock::new(vec![latest]),
                &mut Preferences::default(),
                "0.1.0",
                1000,
                Distribution::Native
            )
            .is_err()
        );
    }
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
    let fetch = Mock::new(vec![
        reply(API, 429, vec![]),
        reply(&url, 200, bytes.clone()),
    ]);
    let found = check_with(fetch, &mut prefs, "0.1.0", 1000, distribution)
        .unwrap()
        .unwrap();
    assert_eq!(found.manifest.notes, "Release notes in the app");
    assert_eq!(prefs.api_retry_after, 1500);
    let fetch = Mock::new(vec![reply(&url, 200, bytes)]);
    check_with(fetch, &mut prefs, "0.1.0", 1100, distribution).unwrap();
    let failed = Mock::new(vec![reply(&url, 503, vec![])]);
    assert!(check_with(failed, &mut prefs, "0.1.0", 1200, distribution).is_err());
    assert_eq!(prefs.api_retry_after, 1500);
}

#[test]
fn incomplete_asset_metadata_uses_the_manifest_for_that_release() {
    let mode = Distribution::Native;
    let manifest = manifest(mode, "0.2.0", b"verified package");
    let mut latest: serde_json::Value = serde_json::from_slice(&release("0.2.0").bytes).unwrap();
    let asset = serde_json::json!({
        "name": mode.package_name(&Version::parse("0.2.0").unwrap()),
        "url": "https://api.github.com/repos/Hunlongyu/githly/releases/assets/1",
        "browser_download_url": manifest.installer.url,
        "size": manifest.installer.size,
        "digest": format!("sha256:{}", manifest.installer.sha256),
    });
    for (field, incomplete) in [
        ("size", serde_json::Value::Null),
        ("digest", "sha256:".into()),
    ] {
        let mut incomplete_asset = asset.clone();
        incomplete_asset[field] = incomplete;
        latest["assets"] = serde_json::json!([incomplete_asset]);
        let fetch = Mock::new(vec![
            reply(API, 200, serde_json::to_vec(&latest).unwrap()),
            reply(
                format!("{RELEASES}/download/v0.2.0/{}", mode.manifest_name()),
                200,
                serde_json::to_vec(&manifest).unwrap(),
            ),
        ]);
        let update = check_with(fetch, &mut Preferences::default(), "0.1.0", 1000, mode)
            .unwrap()
            .unwrap();
        assert_eq!(update.manifest.installer.sha256, manifest.installer.sha256);
    }
}

#[test]
fn rejects_stale_manifest_for_known_release() {
    let mode = Distribution::Native;
    let url = format!("{RELEASES}/download/v0.2.0/{}", mode.manifest_name());
    let fetch = Mock::new(vec![
        release("0.2.0"),
        reply(
            &url,
            200,
            serde_json::to_vec(&manifest(mode, "0.1.0", b"stale")).unwrap(),
        ),
    ]);
    assert!(
        check_with(
            fetch.clone(),
            &mut Preferences::default(),
            "0.1.0",
            1000,
            mode
        )
        .is_err()
    );
}

#[test]
fn downloads_retry_builtin_mirrors_with_verified_bytes_and_progress() {
    let directory = tempfile::tempdir().unwrap();
    let content = b"test installer";
    let update = available(content);
    let direct = &update.manifest.installer.url;
    let fetch = Mock::new(vec![
        reply(direct, 503, vec![]),
        reply(
            format!("{}{direct}", MIRRORS[0]),
            200,
            b"wrong bytes".to_vec(),
        ),
        reply(format!("{}{direct}", MIRRORS[1]), 200, content.to_vec()),
    ]);
    let received = AtomicU64::new(500);
    let path = download_with(fetch.as_ref(), &update, directory.path(), &received).unwrap();
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
    let fetch = Mock::new(vec![
        broken,
        reply(
            format!("{}{direct}", MIRRORS[0]),
            200,
            b"expectedly long".to_vec(),
        ),
        reply(format!("{}{direct}", MIRRORS[1]), 200, b"mismatch".to_vec()),
    ]);
    assert!(
        download_with(
            fetch.as_ref(),
            &update,
            directory.path(),
            &AtomicU64::new(0)
        )
        .is_err()
    );
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

#[cfg(target_os = "linux")]
#[test]
fn appimage_update_preserves_the_channel_and_replaces_only_verified_bytes() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let image = dir.path().join("My Githly.AppImage");
    let downloaded = dir.path().join("update");
    let mode = Distribution::Portable;
    let update = manifest(mode, "0.2.0", b"new");
    assert!(update.installer.url.ends_with(".AppImage"));
    assert_eq!(
        mode.manifest_name(),
        format!("update-{}-appimage.json", platform_id())
    );
    assert!(validate(&update, Distribution::Native).is_err());
    assert!(validate(&available(b"deb").manifest, mode).is_err());
    std::fs::write(&image, b"old").unwrap();
    std::fs::set_permissions(&image, std::fs::Permissions::from_mode(0o750)).unwrap();
    std::fs::write(&downloaded, b"bad").unwrap();
    assert!(appimage::replace(&image, &downloaded, &update.installer).is_err());
    assert_eq!(std::fs::read(&image).unwrap(), b"old");
    std::fs::write(&downloaded, b"new").unwrap();
    appimage::replace(&image, &downloaded, &update.installer).unwrap();
    assert_eq!(std::fs::read(&image).unwrap(), b"new");
    assert_eq!(
        std::fs::metadata(&image).unwrap().permissions().mode() & 0o777,
        0o750
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
        let InstallOutcome::Restart(restart) = install(&path, &available).unwrap() else {
            panic!("Portable installs must restart the replaced executable");
        };
        assert_eq!(
            restart,
            path.with_file_name("My portable Githly.exe")
                .canonicalize()
                .unwrap()
        );
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
