//! Release metadata and bounded downloads. All entry points run on a worker.
use crate::i18n::messages;
use anyhow::{Context, Result, ensure};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(windows)]
mod http;
#[cfg(unix)]
#[path = "update/unix_http.rs"]
mod http;
use http::Http;
#[cfg(target_os = "linux")]
mod appimage;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod portable;
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

// Public release-file proxies. Only GitHub supplies the expected asset digest.
const MIRRORS: &[&str] = &["https://gh-proxy.org/", "https://ghfast.top/"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Distribution {
    Native,
    #[cfg(any(windows, target_os = "linux"))]
    Portable,
}

impl Distribution {
    fn current() -> Result<Self> {
        #[cfg(windows)]
        if !portable::is_installed(&std::env::current_exe()?)? {
            return Ok(Self::Portable);
        }
        #[cfg(target_os = "linux")]
        if appimage::current_path()?.is_some() {
            return Ok(Self::Portable);
        }
        Ok(Self::Native)
    }

    fn manifest_name(self) -> String {
        #[cfg(windows)]
        if self == Self::Portable {
            return format!("update-{}-portable.json", platform_id());
        }
        #[cfg(target_os = "linux")]
        if self == Self::Portable {
            return format!("update-{}-appimage.json", platform_id());
        }

        // Keep the manifest URL used by already-installed Windows x64 clients.
        if cfg!(all(windows, target_arch = "x86_64")) {
            "update.json".into()
        } else {
            format!("update-{}.json", platform_id())
        }
    }

    fn package_name(self, version: &Version) -> String {
        #[cfg(windows)]
        let suffix = if self == Self::Portable {
            ".zip"
        } else {
            "-setup.exe"
        };
        #[cfg(target_os = "linux")]
        let suffix = if self == Self::Portable {
            ".AppImage"
        } else {
            ".deb"
        };
        #[cfg(target_os = "macos")]
        let suffix = ".dmg";
        format!("Githly-{version}-{}{suffix}", platform_id())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub api_retry_after: u64,
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(test, derive(Serialize))]
pub struct Installer {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(test, derive(Serialize))]
pub struct Manifest {
    pub version: String,
    pub notes_url: String,
    #[serde(default)]
    pub notes: String,
    pub installer: Installer,
}
#[derive(Clone, Debug)]
pub struct Available {
    pub manifest: Manifest,
    pub distribution: Distribution,
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

fn validate(manifest: &Manifest, distribution: Distribution) -> Result<Version> {
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
            == format!(
                "{RELEASES}/download/v{version}/{}",
                distribution.package_name(&version)
            ),
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

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    body: Option<String>,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

pub fn check(preferences: &mut Preferences) -> Result<Option<Available>> {
    check_with(
        &mut Http,
        preferences,
        env!("CARGO_PKG_VERSION"),
        now(),
        Distribution::current()?,
    )
}

fn check_with(
    fetch: &mut impl Fetch,
    preferences: &mut Preferences,
    current: &str,
    time: u64,
    distribution: Distribution,
) -> Result<Option<Available>> {
    let current = Version::parse(current)?;
    let mut expected = None;
    let mut notes = String::new();
    if time >= preferences.api_retry_after {
        match metadata(fetch, API) {
            Ok((response, bytes)) if response.status == 200 => {
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
                    return Ok(None);
                }
                notes = release.body.unwrap_or_default();
                let url = format!(
                    "{RELEASES}/download/v{version}/{}",
                    distribution.package_name(&version)
                );
                if let Some(asset) = release
                    .assets
                    .iter()
                    .find(|asset| asset.browser_download_url == url)
                    && let Some(hash) = asset
                        .digest
                        .as_deref()
                        .and_then(|digest| digest.strip_prefix("sha256:"))
                {
                    let manifest = Manifest {
                        version: version.to_string(),
                        notes_url: format!("{RELEASES}/tag/v{version}"),
                        notes,
                        installer: Installer {
                            url,
                            size: asset.size,
                            sha256: hash.into(),
                        },
                    };
                    validate(&manifest, distribution)?;
                    return Ok(Some(Available {
                        manifest,
                        distribution,
                    }));
                }
                expected = Some(version);
            }
            Ok((response, _)) if matches!(response.status, 403 | 429) => {
                preferences.api_retry_after = response
                    .rate_reset
                    .unwrap_or(0)
                    .max(time.saturating_add(response.retry_after.unwrap_or(3600)))
                    .max(time.saturating_add(60));
            }
            _ => {}
        }
    }

    let name = distribution.manifest_name();
    let url = expected.as_ref().map_or_else(
        || format!("{RELEASES}/latest/download/{name}"),
        |v| format!("{RELEASES}/download/v{v}/{name}"),
    );
    let (response, bytes) = metadata(fetch, &url)?;
    ensure!(
        response.status == 200,
        messages().source_unavailable(response.status)
    );
    let mut manifest: Manifest =
        serde_json::from_slice(&bytes).context(messages().invalid_manifest())?;
    let version = validate(&manifest, distribution)?;
    ensure!(
        expected.is_none_or(|expected| expected == version),
        messages().invalid_manifest()
    );
    if !notes.is_empty() {
        manifest.notes = notes;
    }
    Ok((version > current).then_some(Available {
        manifest,
        distribution,
    }))
}

pub fn download(available: &Available, received: &AtomicU64) -> Result<PathBuf> {
    let directory = directories::BaseDirs::new()
        .context(messages().update_cache_dir())?
        .cache_dir()
        .join("githly")
        .join("updates");
    std::fs::create_dir_all(&directory)?;
    download_with(&mut Http, available, &directory, received)
}

struct DownloadWriter<'a> {
    file: File,
    received: &'a AtomicU64,
}

impl Write for DownloadWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let count = self.file.write(bytes)?;
        self.received.fetch_add(count as u64, Ordering::Relaxed);
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
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
    received: &AtomicU64,
) -> Result<PathBuf> {
    validate(&available.manifest, available.distribution)?;
    let direct = &available.manifest.installer.url;
    // A single fixed cache entry bounds disk usage across versions. Unique partial
    // files prevent a second application instance from sharing an active writer.
    let partial = PartialFile(directory.join(format!("installer-{}.part", std::process::id())));
    let extension = Path::new(&available.manifest.installer.url)
        .extension()
        .expect("validated package URL has an extension");
    let destination = directory.join("Githly-update").with_extension(extension);
    let mut error = anyhow::anyhow!(messages().installer_failed());
    for prefix in std::iter::once(&"").chain(MIRRORS) {
        let url = format!("{prefix}{direct}");
        received.store(0, Ordering::Relaxed);
        let result = (|| {
            let mut writer = DownloadWriter {
                file: File::create(&partial.0)?,
                received,
            };
            let response = fetch.get(&url, &mut writer, available.manifest.installer.size)?;
            ensure!(
                response.status == 200,
                messages().download_http(response.status)
            );
            writer.file.sync_all()?;
            drop(writer);
            verify_file(&partial.0, &available.manifest.installer)?;
            std::fs::rename(&partial.0, &destination).context(messages().installer_in_use())?;
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
    verify_reader(&mut File::open(path)?, installer)
}

fn verify_reader(file: &mut File, installer: &Installer) -> Result<()> {
    use std::io::Read;
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

pub enum InstallOutcome {
    Restart(PathBuf),
    #[cfg(windows)]
    InstallerStarted,
}

pub fn install(path: &Path, available: &Available) -> Result<InstallOutcome> {
    let distribution = Distribution::current()?;
    ensure!(
        distribution == available.distribution,
        messages().invalid_installer_url()
    );
    validate(&available.manifest, distribution)?;
    #[cfg(windows)]
    if distribution == Distribution::Portable {
        return portable::apply(path, &available.manifest.installer).map(InstallOutcome::Restart);
    }
    #[cfg(target_os = "linux")]
    if distribution == Distribution::Portable {
        let image = appimage::current_path()?.context(messages().invalid_installer_url())?;
        appimage::replace(&image, path, &available.manifest.installer)?;
        return Ok(InstallOutcome::Restart(image));
    }
    verify_file(path, &available.manifest.installer)?;
    #[cfg(target_os = "linux")]
    {
        // Wait for the package transaction, including the desktop authorization
        // prompt. Opening a .deb in a viewer does not confirm it was installed.
        let output = std::process::Command::new("/usr/bin/pkexec")
            .args(["/usr/bin/apt-get", "--yes", "--no-remove", "install"])
            .arg(path)
            .output()
            .context(messages().installer_launch_failed())?;
        ensure!(
            output.status.success(),
            "apt ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        let installed = std::process::Command::new("/usr/bin/dpkg-query")
            .args(["-W", "-f=${Status} ${Version}", "githly"])
            .output()?;
        ensure!(
            installed.status.success()
                && installed.stdout
                    == format!("install ok installed {}", available.manifest.version).as_bytes(),
            "dpkg did not confirm installation of Githly {}",
            available.manifest.version
        );
        Ok(InstallOutcome::Restart("/usr/bin/githly".into()))
    }
    #[cfg(target_os = "macos")]
    {
        macos::install(path, &available.manifest.version).map(InstallOutcome::Restart)
    }
    #[cfg(windows)]
    {
        let mut command = std::process::Command::new(path);
        // The normal installer keeps errors visible and uses Restart Manager to close
        // applications gracefully. No shell, forced termination, or silent overwrite.
        command.arg("/UPDATE").arg("/NORESTART");
        if let Some(directory) = std::env::current_exe()?.parent() {
            command.arg(format!("/DIR={}", directory.display()));
        }
        command
            .spawn()
            .context(messages().installer_launch_failed())?;
        Ok(InstallOutcome::InstallerStarted)
    }
}
