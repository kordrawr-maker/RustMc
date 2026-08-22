use anyhow::{bail, Context, Result};
use reqwest::{Client, Url};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const ALLOWED_HOSTS: &[&str] = &[
    "mojang.com",
    "minecraft.net",
    "papermc.io",
    "fabricmc.net",
    "neoforged.net",
    "adoptium.net",
    "github.com",
    "githubusercontent.com",
];

pub fn client() -> Result<Client> {
    Client::builder()
        .user_agent(concat!("rustmc/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(20))
        .build()
        .map_err(Into::into)
}

pub fn ensure_allowed(url: &Url) -> Result<()> {
    if url.scheme() != "https" {
        bail!("refusing non-https url: {url}");
    }
    let host = url.host_str().unwrap_or("");
    let ok = ALLOWED_HOSTS
        .iter()
        .any(|h| host == *h || host.ends_with(&format!(".{h}")));
    if !ok {
        bail!("host not in download allowlist: {host}");
    }
    Ok(())
}

pub async fn fetch_json(client: &Client, url: &str) -> Result<serde_json::Value> {
    let resp = client.get(url).send().await.context("request failed")?;
    ensure_allowed(resp.url())?;
    if !resp.status().is_success() {
        bail!("GET {url} -> {}", resp.status());
    }
    let body = resp.bytes().await?;
    if body.len() > 16 * 1024 * 1024 {
        bail!("response too large from {url}");
    }
    serde_json::from_slice(&body).context("invalid json response")
}

pub struct DownloadOpts<'a> {
    pub max_size: u64,
    pub sha1: Option<&'a str>,
    pub sha256: Option<&'a str>,
    pub label: &'a str,
}

pub async fn download_file(
    client: &Client,
    url: &str,
    dest: &Path,
    opts: &DownloadOpts<'_>,
) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut resp = client.get(url).send().await.context("request failed")?;
    ensure_allowed(resp.url())?;
    if !resp.status().is_success() {
        bail!("GET {url} -> {}", resp.status());
    }
    let total = resp.content_length();
    let part = PathBuf::from(format!("{}.part", dest.display()));
    let mut file = std::fs::File::create(&part)?;
    let mut n: u64 = 0;
    let mut last = Instant::now();
    while let Some(chunk) = resp.chunk().await? {
        n += chunk.len() as u64;
        if n > opts.max_size {
            bail!(
                "download exceeded the {} byte limit: {}",
                opts.max_size,
                opts.label
            );
        }
        file.write_all(&chunk)?;
        if last.elapsed() >= Duration::from_millis(200) {
            match total {
                Some(t) => eprint!(
                    "\r  {:<24} {:>8} / {} MB",
                    opts.label,
                    n / (1 << 20),
                    t / (1 << 20)
                ),
                None => eprint!("\r  {:<24} {:>8} MB", opts.label, n / (1 << 20)),
            }
            last = Instant::now();
        }
    }
    file.flush()?;
    drop(file);
    eprintln!("\r  {:<24} done ({} MB)", opts.label, n / (1 << 20));
    verify_hash(&part, opts)?;
    if dest.exists() {
        std::fs::remove_file(dest)?;
    }
    std::fs::rename(&part, dest)?;
    Ok(())
}

fn verify_hash(path: &Path, opts: &DownloadOpts<'_>) -> Result<()> {
    if opts.sha1.is_none() && opts.sha256.is_none() {
        return Ok(());
    }
    let data = std::fs::read(path)?;
    if let Some(expected) = opts.sha1 {
        let actual = hex::encode(Sha1::digest(&data));
        if !actual.eq_ignore_ascii_case(expected) {
            bail!(
                "sha1 mismatch for {}: expected {}, got {}",
                opts.label,
                expected,
                actual
            );
        }
    }
    if let Some(expected) = opts.sha256 {
        let actual = hex::encode(Sha256::digest(&data));
        if !actual.eq_ignore_ascii_case(expected) {
            bail!(
                "sha256 mismatch for {}: expected {}, got {}",
                opts.label,
                expected,
                actual
            );
        }
    }
    Ok(())
}

pub async fn fetch_sidecar_sha1(client: &Client, url: &str) -> Result<Option<String>> {
    let resp = client
        .get(format!("{url}.sha1"))
        .send()
        .await
        .context("request failed")?;
    ensure_allowed(resp.url())?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let body = resp.text().await?;
    Ok(body.split_whitespace().next().map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        s.parse().unwrap()
    }

    #[test]
    fn allows_known_hosts() {
        assert!(ensure_allowed(&url("https://api.papermc.io/v2/x")).is_ok());
        assert!(ensure_allowed(&url("https://piston-meta.mojang.com/x")).is_ok());
        assert!(ensure_allowed(&url("https://objects.githubusercontent.com/x")).is_ok());
    }

    #[test]
    fn rejects_bad_urls() {
        assert!(ensure_allowed(&url("http://api.papermc.io/x")).is_err());
        assert!(ensure_allowed(&url("https://evil.example.com/x")).is_err());
        assert!(ensure_allowed(&url("https://papermc.io.evil.com/x")).is_err());
    }
}
