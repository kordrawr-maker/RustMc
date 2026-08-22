use crate::net;
use crate::version;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use std::path::Path;

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Deserialize)]
struct Manifest {
    versions: Vec<ManifestVersion>,
}

#[derive(Deserialize)]
struct ManifestVersion {
    id: String,
    #[serde(rename = "type")]
    typ: String,
    url: String,
}

#[derive(Deserialize)]
struct VersionJson {
    downloads: Downloads,
    #[serde(rename = "javaVersion", default)]
    java_version: Option<JavaVersion>,
}

#[derive(Deserialize)]
struct JavaVersion {
    #[serde(rename = "majorVersion")]
    major_version: u16,
}

#[derive(Deserialize)]
struct Downloads {
    server: Download,
}

#[derive(Deserialize)]
struct Download {
    url: String,
    sha1: String,
}

pub async fn versions(client: &Client) -> Result<Vec<String>> {
    let v: Manifest = serde_json::from_value(net::fetch_json(client, MANIFEST_URL).await?)?;
    let mut out: Vec<String> = v
        .versions
        .iter()
        .filter(|x| x.typ == "release" && version::dots(&x.id).is_some())
        .map(|x| x.id.clone())
        .collect();
    version::sort_desc(&mut out);
    Ok(out)
}

pub async fn download(client: &Client, version: &str, server_dir: &Path) -> Result<String> {
    let vj: VersionJson = fetch_version(client, version).await?;
    let dest = server_dir.join("server.jar");
    net::download_file(
        client,
        &vj.downloads.server.url,
        &dest,
        &net::DownloadOpts {
            max_size: 128 << 20,
            sha1: Some(&vj.downloads.server.sha1),
            sha256: None,
            label: "vanilla server jar",
        },
    )
    .await?;
    Ok("server.jar".into())
}

pub async fn java_major(client: &Client, version: &str) -> Result<Option<u16>> {
    let vj: VersionJson = fetch_version(client, version).await?;
    Ok(vj.java_version.map(|j| j.major_version))
}

async fn fetch_version(client: &Client, version: &str) -> Result<VersionJson> {
    let v: Manifest = serde_json::from_value(net::fetch_json(client, MANIFEST_URL).await?)?;
    let entry = v
        .versions
        .iter()
        .find(|x| x.id == version)
        .with_context(|| format!("version {version} not found in the Mojang manifest"))?;
    serde_json::from_value(net::fetch_json(client, &entry.url).await?).context("invalid version json")
}
