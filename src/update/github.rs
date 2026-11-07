use super::{
    API, Distribution, Fetch, Installer, Manifest, RELEASES, Response, metadata, valid_sha256,
};
use crate::i18n::{Locale, LocalizedText, messages};
use anyhow::{Context, Result, ensure};
use self_update::{
    Release,
    backends::github::Update,
    http_client::{HeaderMap, HttpClient, HttpResponse},
};
use semver::Version;
use serde::Deserialize;
use std::{
    io::{Cursor, Read},
    sync::{Arc, OnceLock},
    time::Duration,
};

pub(super) enum Latest {
    Current,
    Available(Manifest),
    NeedsManifest(Release),
    Unavailable(Response),
}

// self_update exposes the asset name, API URL and digest. Githly also needs the
// public download URL and byte count to pin mirror downloads to the same asset.
#[derive(Deserialize)]
struct Asset {
    url: String,
    browser_download_url: Option<String>,
    size: Option<u64>,
}

#[derive(Deserialize)]
struct ReleaseDetails {
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

struct GithubClient<F> {
    fetch: Arc<F>,
    response: OnceLock<Response>,
    assets: OnceLock<Vec<Asset>>,
}

struct Body(Vec<u8>, HeaderMap);

impl HttpResponse for Body {
    fn headers(&self) -> &HeaderMap {
        &self.1
    }

    fn body(self: Box<Self>) -> Box<dyn Read> {
        Box::new(Cursor::new(self.0))
    }
}

impl<F: Fetch + Send + Sync> HttpClient for GithubClient<F> {
    fn get(
        &self,
        url: &str,
        _: &HeaderMap,
        _: Option<Duration>,
    ) -> self_update::Result<Box<dyn HttpResponse>> {
        // This adapter is only for the anonymous, size-limited latest-release request.
        if url != API {
            return Err(self_update::Error::invalid_response(
                "unexpected release URL",
            ));
        }
        let (response, bytes) =
            metadata(self.fetch.as_ref(), url).map_err(self_update::Error::transport)?;
        let status = response.status;
        let _ = self.response.set(response);
        if status != 200 {
            return Err(self_update::Error::http_status_error(status as u16, url));
        }
        let details: ReleaseDetails =
            serde_json::from_slice(&bytes).map_err(self_update::Error::invalid_response)?;
        if details.draft || details.prerelease {
            return Err(self_update::Error::invalid_response(
                messages().no_release(),
            ));
        }
        let _ = self.assets.set(details.assets);
        Ok(Box::new(Body(bytes, HeaderMap::new())))
    }
}

pub(super) fn latest(
    fetch: Arc<impl Fetch + Send + Sync + 'static>,
    distribution: Distribution,
    current: &str,
) -> Result<Latest> {
    let client = Arc::new(GithubClient {
        fetch,
        response: OnceLock::new(),
        assets: OnceLock::new(),
    });
    let updater = Update::configure()
        .repo_owner("Hunlongyu")
        .repo_name("githly")
        .bin_name("githly")
        .current_version(current)
        .http_client(client.clone())
        .build()?;
    let releases = match updater.get_latest_release() {
        Ok(releases) => releases,
        Err(error) => {
            let response = client.response.get().copied().unwrap_or_default();
            if response.status != 200 {
                return Ok(Latest::Unavailable(response));
            }
            return Err(error).with_context(|| LocalizedText::new(Locale::invalid_release));
        }
    };
    if !releases.is_update_available()? {
        return Ok(Latest::Current);
    }
    let release = releases
        .into_vec()
        .into_iter()
        .next()
        .with_context(|| LocalizedText::new(Locale::no_release))?;
    let version = Version::parse(release.version())?;
    ensure!(
        version.pre.is_empty() && version.build.is_empty(),
        LocalizedText::new(Locale::stable_only)
    );
    let name = distribution.package_name(&version);
    let url = format!("{RELEASES}/download/v{version}/{name}");
    let details = client
        .assets
        .get()
        .with_context(|| LocalizedText::new(Locale::invalid_release))?;
    let selected = release
        .assets()
        .iter()
        .find(|asset| asset.name() == name)
        .and_then(|asset| {
            let hash = asset
                .digest()?
                .strip_prefix("sha256:")
                .filter(|hash| valid_sha256(hash))?;
            let extra = details.iter().find(|extra| {
                extra.url == asset.download_url()
                    && extra.browser_download_url.as_deref() == Some(url.as_str())
            })?;
            Some((extra.size?, hash))
        });
    match selected {
        Some((size, hash)) => Ok(Latest::Available(Manifest {
            version: version.to_string(),
            notes_url: format!("{RELEASES}/tag/v{version}"),
            notes: release.body().unwrap_or_default().to_owned(),
            installer: Installer {
                url,
                size,
                sha256: hash.into(),
            },
        })),
        _ => Ok(Latest::NeedsManifest(release)),
    }
}
