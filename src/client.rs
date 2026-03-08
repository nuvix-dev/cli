use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, SET_COOKIE};
use serde::Serialize;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct NuvixClient {
    http: Client,
    base_url: String,
    session: Option<String>,
}

#[allow(dead_code)]
impl NuvixClient {
    pub fn new(base_url: String, session: Option<String>) -> Result<Self> {
        let http = Client::builder()
            .build()
            .context("failed to initialize HTTP client")?;
        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            session,
        })
    }

    pub fn login_email(base_url: &str, email: &str, password: &str) -> Result<String> {
        #[derive(Serialize)]
        struct LoginPayload<'a> {
            email: &'a str,
            password: &'a str,
        }

        let client = Client::builder()
            .build()
            .context("failed to initialize HTTP client")?;
        let url = format!("{}/sessions/email", base_url.trim_end_matches('/'));

        let response = client
            .post(url)
            .json(&LoginPayload { email, password })
            .send()
            .context("failed to call auth endpoint")?;

        if !response.status().is_success() {
            bail!("login failed with status {}", response.status());
        }

        extract_nc_session(response.headers())
            .context("login succeeded but nc_session cookie was not returned")
    }

    pub fn apply_session_header(
        &self,
        request: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        if let Some(session) = &self.session {
            request.header("x-nuvix-session", session)
        } else {
            request
        }
    }

    pub fn get(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        let url = format!("{}/{}", self.base_url, path.trim_start_matches('/'));
        self.apply_session_header(self.http.get(url))
    }
}

fn extract_nc_session(headers: &HeaderMap) -> Result<String> {
    for value in headers.get_all(SET_COOKIE) {
        let cookie = value
            .to_str()
            .context("invalid set-cookie header value")?
            .trim();
        if let Some(raw) = cookie.strip_prefix("nc_session=") {
            let session = raw.split(';').next().unwrap_or("").trim();
            if !session.is_empty() {
                return Ok(session.to_string());
            }
        }
    }

    bail!("nc_session not found in set-cookie headers")
}

pub fn ensure_console_api_url(
    profile: &crate::global_config::GlobalProjectProfile,
) -> Result<String> {
    if let Some(url) = &profile.console_api_url {
        return Ok(url.clone());
    }

    if let Some(url) = &profile.console_url {
        return Ok(url.clone());
    }

    bail!(
        "project profile is missing console_api_url/console_url. Set it with `nuvix project set-urls`"
    )
}

pub fn ensure_console_url(profile: &crate::global_config::GlobalProjectProfile) -> Result<String> {
    if let Some(url) = &profile.console_url {
        return Ok(url.clone());
    }

    if let Some(url) = &profile.console_api_url {
        return Ok(url.clone());
    }

    bail!(
        "project profile is missing console_url/console_api_url. Set it with `nuvix project set-urls`"
    )
}
