//! Release metadata and bounded downloads. All entry points run on a worker.
use crate::i18n::messages;
use anyhow::{Context, Result, bail, ensure};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

#[cfg(windows)]
mod http;
#[cfg(unix)]
#[path = "update/unix_http.rs"]
mod http;
use http::Http;
#[cfg(test)]
mod tests;

const RELEASES: &str = "https://github.com/Hunlongyu/githly/releases";
const API: &str = "https://api.github.com/repos/Hunlongyu/githly/releases/latest";
const MAX_INSTALLER: u64 = 256 * 1024 * 1024;

fn platform_id() -> String {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    format!("{}-{arch}", std::env::consts::OS)
}

fn manifest_name() -> String {
    // Keep the manifest URL used by already-installed Windows x64 clients.
    if cfg!(all(windows, target_arch = "x86_64")) {
        "update.json".into()
    } else {
        format!("update-{}.json", platform_id())
    }
}

fn package_name(version: &Version) -> String {
    #[cfg(windows)]
    let suffix = "-setup.exe";
    #[cfg(target_os = "linux")]
    let suffix = ".deb";
    #[cfg(target_os = "macos")]
    let suffix = ".dmg";
    format!("Githly-{version}-{}{suffix}", platform_id())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub enabled: bool,
    pub interval_hours: u64,
    pub mirror: String,
    pub last_check: u64,
    pub api_retry_after: u64,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_hours: 24,
            mirror: String::new(),
            last_check: 0,
            api_retry_after: 0,
        }
    }
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn mirror_prefix(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(String::new());
    }
    let parsed = url::Url::parse(value).context(messages().invalid_mirror())?;
    ensure!(
        parsed.scheme() == "https"
            && parsed.host_str().is_some()
            && parsed.username().is_empty()
            && parsed.password().is_none()
            && parsed.query().is_none()
            && parsed.fragment().is_none(),
        messages().mirror_https()
    );
    Ok(format!("{}/", parsed.as_str().trim_end_matches('/')))
}

#[derive(Clone, Debug, Deserialize)]
pub struct Installer {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub notes_url: String,
    pub installer: Installer,
}
#[derive(Clone, Debug)]
pub struct Available {
    pub manifest: Manifest,
    pub source: Source,
    pub mirror: String,
}
#[derive(Debug)]
pub struct Check {
    pub available: Option<Available>,
    pub source: Source,
}

#[derive(Clone, Debug)]
pub enum Source {
    GitHub,
    Release,
    Mirror(String),
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitHub => f.write_str("GitHub"),
            Self::Release => f.write_str("GitHub Release"),
            Self::Mirror(prefix) => f.write_str(&messages().mirror_source(prefix)),
        }
    }
}

#[derive(Default)]
pub(super) struct Response {
    pub status: u32,
    pub retry_after: Option<u64>,
    pub rate_reset: Option<u64>,
}
pub(super) trait Fetch {
    fn get(&mut self, url: &str, output: &mut dyn Write, limit: u64) -> Result<Response>;
}

fn metadata(fetch: &mut impl Fetch, address: &str) -> Result<(Response, Vec<u8>)> {
    let mut bytes = Vec::new();
    let response = fetch.get(address, &mut bytes, 1024 * 1024)?;
    Ok((response, bytes))
}

fn validate(manifest: &Manifest) -> Result<Version> {
    let version = Version::parse(&manifest.version).context(messages().invalid_version())?;
    ensure!(
        version.pre.is_empty() && version.build.is_empty(),
        messages().stable_only()
    );
    ensure!(
        manifest.notes_url == format!("{RELEASES}/tag/v{version}"),
        messages().invalid_notes()
    );
    ensure!(
        manifest.installer.url
            == format!("{RELEASES}/download/v{version}/{}", package_name(&version)),
        messages().invalid_installer_url()
    );
    ensure!(
        manifest.installer.sha256.len() == 64
            && manifest
                .installer
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()),
        messages().invalid_checksum()
    );
    ensure!(
        (1..=MAX_INSTALLER).contains(&manifest.installer.size),
        messages().installer_too_large()
    );
    Ok(version)
}

pub fn check(preferences: &mut Preferences) -> Result<Check> {
    check_with(&mut Http, preferences, env!("CARGO_PKG_VERSION"), now())
}

fn check_with(
    fetch: &mut impl Fetch,
    preferences: &mut Preferences,
    current: &str,
    time: u64,
) -> Result<Check> {
    let mirror = mirror_prefix(&preferences.mirror)?;
    let current = Version::parse(current)?;
    let mut expected = None;
    let mut fallback = time < preferences.api_retry_after;
    if !fallback {
        match metadata(fetch, API) {
            Ok((response, bytes)) if response.status == 200 => {
                #[derive(Deserialize)]
                struct Release {
                    tag_name: String,
                    draft: bool,
                    prerelease: bool,
                }
                let release: Release =
                    serde_json::from_slice(&bytes).context(messages().invalid_release())?;
                ensure!(
                    !release.draft && !release.prerelease,
                    messages().no_release()
                );
                let version = Version::parse(
                    release
                        .tag_name
                        .strip_prefix('v')
                        .unwrap_or(&release.tag_name),
                )?;
                if version <= current {
                    return Ok(Check {
                        available: None,
                        source: Source::GitHub,
                    });
                }
                expected = Some(version);
            }
            Ok((response, _)) => {
                if response.status == 403 || response.status == 429 {
                    preferences.api_retry_after = response
                        .rate_reset
                        .unwrap_or(0)
                        .max(time.saturating_add(response.retry_after.unwrap_or(3600)))
                        .max(time.saturating_add(60));
                }
                fallback = true;
            }
            Err(_) => fallback = true,
        }
    }
    let manifest_file = manifest_name();
    let direct = expected.as_ref().map_or_else(
        || format!("{RELEASES}/latest/download/{manifest_file}"),
        |version| format!("{RELEASES}/download/v{version}/{manifest_file}"),
    );
    let mut used_mirror = fallback && !mirror.is_empty();
    let address = if used_mirror {
        format!("{mirror}{direct}")
    } else {
        direct.clone()
    };
    let first = metadata(fetch, &address);
    let (response, bytes) = match first {
        Ok((response, bytes)) if response.status == 200 => (response, bytes),
        failed if !used_mirror && !mirror.is_empty() => {
            drop(failed);
            used_mirror = true;
            metadata(fetch, &format!("{mirror}{direct}"))?
        }
        Ok((response, _)) => bail!(messages().release_unavailable(response.status)),
        Err(error) => return Err(error),
    };
    ensure!(
        response.status == 200,
        messages().source_unavailable(response.status)
    );
    let manifest: Manifest =
        serde_json::from_slice(&bytes).context(messages().invalid_manifest())?;
    let version = validate(&manifest)?;
    if let Some(expected) = expected {
        ensure!(version == expected, messages().mirror_outdated());
    }
    let source = if used_mirror {
        Source::Mirror(mirror.clone())
    } else {
        Source::Release
    };
    Ok(Check {
        available: (version > current).then(|| Available {
            manifest,
            source: source.clone(),
            mirror,
        }),
        source,
    })
}

pub fn download(available: &Available) -> Result<PathBuf> {
    let directory = directories::BaseDirs::new()
        .context(messages().update_cache_dir())?
        .cache_dir()
        .join("githly")
        .join("updates");
    std::fs::create_dir_all(&directory)?;
    download_with(&mut Http, available, &directory)
}

struct PartialFile(PathBuf);
impl Drop for PartialFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn download_with(
    fetch: &mut impl Fetch,
    available: &Available,
    directory: &Path,
) -> Result<PathBuf> {
    validate(&available.manifest)?;
    let mirror = mirror_prefix(&available.mirror)?;
    let direct = &available.manifest.installer.url;
    let mirrored = format!("{mirror}{direct}");
    let urls: Vec<&str> = if mirror.is_empty() {
        vec![direct]
    } else if matches!(available.source, Source::Mirror(_)) {
        vec![&mirrored, direct]
    } else {
        vec![direct, &mirrored]
    };
    // A single fixed cache entry bounds disk usage across versions. Unique partial
    // files prevent a second application instance from sharing an active writer.
    let partial = PartialFile(directory.join(format!("installer-{}.part", std::process::id())));
    let extension = Path::new(&available.manifest.installer.url)
        .extension()
        .expect("validated package URL has an extension");
    let destination = directory.join("Githly-update").with_extension(extension);
    let mut error = anyhow::anyhow!(messages().installer_failed());
    for url in urls {
        let result = (|| {
            let mut file = File::create(&partial.0)?;
            let response = fetch.get(url, &mut file, available.manifest.installer.size)?;
            ensure!(
                response.status == 200,
                messages().download_http(response.status)
            );
            file.sync_all()?;
            drop(file);
            verify_file(&partial.0, &available.manifest.installer)?;
            if destination.exists() {
                std::fs::remove_file(&destination).context(messages().installer_in_use())?;
            }
            std::fs::rename(&partial.0, &destination)?;
            Ok(destination.clone())
        })();
        match result {
            Ok(path) => return Ok(path),
            Err(failed) => error = failed,
        }
    }
    Err(error)
}

pub fn verify_file(path: &Path, installer: &Installer) -> Result<()> {
    use std::io::Read;
    let mut file = File::open(path)?;
    ensure!(
        file.metadata()?.len() == installer.size,
        messages().installer_incomplete()
    );
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    let actual: String = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    ensure!(
        actual.eq_ignore_ascii_case(&installer.sha256),
        messages().checksum_failed()
    );
    Ok(())
}

pub fn install(path: &Path, installer: &Installer) -> Result<()> {
    verify_file(path, installer)?;
    #[cfg(unix)]
    {
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        let status = std::process::Command::new(opener)
            .arg(path)
            .status()
            .context(messages().installer_launch_failed())?;
        ensure!(status.success(), messages().installer_launch_failed());
        Ok(())
    }
    #[cfg(windows)]
    {
        let mut command = std::process::Command::new(path);
        // The normal installer keeps errors visible and uses Restart Manager to close
        // applications gracefully. No shell, forced termination, or silent overwrite.
        command.arg("/UPDATE").arg("/NORESTART");
        if let Some(directory) = std::env::current_exe()?.parent()
            && directory.join("unins000.exe").is_file()
        {
            command.arg(format!("/DIR={}", directory.display()));
        }
        command
            .spawn()
            .context(messages().installer_launch_failed())?;
        Ok(())
    }
}
