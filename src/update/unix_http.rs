use super::{Fetch, Response};
use crate::i18n::{Locale, LocalizedText};
use anyhow::{Result, ensure};
use std::{
    io::{self, Write},
    time::Duration,
};

pub(super) struct Http;

impl Fetch for Http {
    fn get(&self, address: &str, output: &mut dyn Write, limit: u64) -> Result<Response> {
        let config = ureq::Agent::config_builder()
            .https_only(true)
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_global(Some(Duration::from_secs(if limit <= 1024 * 1024 {
                45
            } else {
                600
            })))
            .user_agent(concat!("Githly/", env!("CARGO_PKG_VERSION")))
            .build();
        let mut reply = ureq::Agent::new_with_config(config)
            .get(address)
            .header(
                "Accept",
                if address == super::API {
                    "application/vnd.github+json"
                } else {
                    "application/octet-stream"
                },
            )
            .call()?;
        let number = |name: &str| {
            reply
                .headers()
                .get(name)?
                .to_str()
                .ok()?
                .parse::<u64>()
                .ok()
        };
        let response = Response {
            status: u32::from(reply.status().as_u16()),
            retry_after: number("retry-after"),
            rate_reset: number("x-ratelimit-reset"),
        };
        if response.status == 200 {
            ensure!(
                number("content-length").is_none_or(|length| length <= limit),
                LocalizedText::new(Locale::update_size_limit)
            );
            io::copy(
                &mut reply.body_mut().with_config().limit(limit).reader(),
                output,
            )?;
        }
        Ok(response)
    }
}
