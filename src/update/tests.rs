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
fn manifest(version: &str, content: &[u8]) -> Vec<u8> {
    let hash: String = Sha256::digest(content)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    serde_json::to_vec(&serde_json::json!({
        "version": version,
        "notes_url": format!("{RELEASES}/tag/v{version}"),
        "installer": { "url": format!("{RELEASES}/download/v{version}/Githly-{version}-windows-x64-setup.exe"), "size": content.len(), "sha256": hash }
    })).unwrap()
}
fn available(content: &[u8]) -> Available {
    Available {
        manifest: serde_json::from_slice(&manifest("0.2.0", content)).unwrap(),
        source: Source::Release,
        mirror: String::new(),
    }
}

#[test]
fn no_update_never_downloads_manifest() {
    let mut fetch = Mock::new(vec![release("0.1.0")]);
    let result = check_with(&mut fetch, &mut Preferences::default(), "0.1.0", 1000).unwrap();
    assert!(result.available.is_none());
    assert_eq!(fetch.calls.len(), 1);
}

#[test]
fn new_release_uses_matching_manifest_and_rejects_stale_mirror() {
    let address = format!("{RELEASES}/download/v0.2.0/update.json");
    let mut fetch = Mock::new(vec![
        release("0.2.0"),
        reply(&address, 200, manifest("0.2.0", b"installer")),
    ]);
    let result = check_with(&mut fetch, &mut Preferences::default(), "0.1.0", 1000).unwrap();
    assert_eq!(result.available.unwrap().manifest.version, "0.2.0");
    let mut preferences = Preferences {
        mirror: "https://mirror.example/".into(),
        ..Preferences::default()
    };
    let mut fetch = Mock::new(vec![
        release("0.2.0"),
        reply(&address, 503, vec![]),
        reply(
            format!("https://mirror.example/{address}"),
            200,
            manifest("0.1.0", b"old"),
        ),
    ]);
    assert!(
        check_with(&mut fetch, &mut preferences, "0.1.0", 1000)
            .unwrap_err()
            .to_string()
            .contains(messages().mirror_outdated())
    );
}

#[test]
fn rate_limit_uses_release_mirror_and_honors_server_cooldown() {
    for status in [403, 429] {
        let mirror = "https://mirror.example/";
        let address = format!("{mirror}{RELEASES}/latest/download/update.json");
        let mut preferences = Preferences {
            mirror: mirror.into(),
            ..Preferences::default()
        };
        let mut fetch = Mock::new(vec![
            reply(API, status, vec![]),
            reply(&address, 200, manifest("0.2.0", b"exe")),
        ]);
        let result = check_with(&mut fetch, &mut preferences, "0.1.0", 1000).unwrap();
        assert!(matches!(result.source, Source::Mirror(_)));
        assert_eq!(preferences.api_retry_after, 1500);
        let mut fetch = Mock::new(vec![reply(&address, 200, manifest("0.2.0", b"exe"))]);
        check_with(&mut fetch, &mut preferences, "0.1.0", 1100).unwrap();
        assert!(!fetch.calls.iter().any(|url| url == API));
    }
}

#[test]
fn mirror_failure_is_reported_and_cooldown_survives_failure() {
    let mut preferences = Preferences {
        mirror: "https://mirror.example/".into(),
        ..Preferences::default()
    };
    let mut fetch = Mock::new(vec![
        reply(API, 429, vec![]),
        reply(
            format!("https://mirror.example/{RELEASES}/latest/download/update.json"),
            503,
            vec![],
        ),
    ]);
    assert!(check_with(&mut fetch, &mut preferences, "0.1.0", 1000).is_err());
    assert_eq!(preferences.api_retry_after, 1500);
}

#[test]
fn downloads_retry_mirror_and_verify_before_promoting_file() {
    let directory = tempfile::tempdir().unwrap();
    let content = b"test installer";
    let mut update = available(content);
    update.mirror = "https://mirror.example/".into();
    let mut fetch = Mock::new(vec![
        reply(&update.manifest.installer.url, 503, vec![]),
        reply(
            format!("{}{}", update.mirror, update.manifest.installer.url),
            200,
            content.to_vec(),
        ),
    ]);
    let path = download_with(&mut fetch, &update, directory.path()).unwrap();
    assert_eq!(std::fs::read(path).unwrap(), content);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn truncated_corrupt_and_interrupted_downloads_leave_no_partial_file() {
    for (bytes, broken) in [
        (b"exe".to_vec(), false),
        (b"bad installer".to_vec(), false),
        (b"test".to_vec(), true),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let update = available(b"test installr");
        let mut response = reply(&update.manifest.installer.url, 200, bytes);
        response.broken = broken;
        let mut fetch = Mock::new(vec![response]);
        assert!(download_with(&mut fetch, &update, directory.path()).is_err());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}

#[test]
fn manifest_and_mirror_addresses_cannot_supply_arbitrary_executables() {
    for address in [
        "http://mirror.example",
        "https://user:password@mirror.example",
        "https://mirror.example/?url=",
    ] {
        assert!(mirror_prefix(address).is_err());
    }
    let mut update = available(b"installer");
    update.manifest.installer.url = "https://other.example/payload.exe".into();
    assert!(validate(&update.manifest).is_err());
    update = available(b"installer");
    update.manifest.installer.size = MAX_INSTALLER + 1;
    assert!(validate(&update.manifest).is_err());
    assert_eq!(
        mirror_prefix("https://mirror.example/proxy").unwrap(),
        "https://mirror.example/proxy/"
    );
}

#[test]
fn install_failure_preserves_current_program() {
    let directory = tempfile::tempdir().unwrap();
    let update = available(b"not an executable");
    let path = directory.path().join("invalid.exe");
    std::fs::write(&path, b"not an executable").unwrap();
    assert!(install(&path, &update.manifest.installer).is_err());
    assert!(std::env::current_exe().unwrap().is_file());
}
