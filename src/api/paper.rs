use crate::net;
use crate::version;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

const BASE: &str = "https://fill.papermc.io/v3/projects/paper";

#[derive(Deserialize)]
struct Project {
    versions: BTreeMap<String, Vec<String>>,
}

#[derive(Deserialize)]
struct Build {
    id: i64,
    channel: String,
    downloads: BTreeMap<String, Download>,
}

#[derive(Deserialize)]
struct Download {
    name: String,
    checksums: Checksums,
    url: String,
}

#[derive(Deserialize)]
struct Checksums {
    sha256: String,
}

pub async fn versions(client: &Client) -> Result<Vec<String>> {
    let p: Project = serde_json::from_value(net::fetch_json(client, BASE).await?)?;
    let mut out: Vec<String> = p
        .versions
        .values()
        .filter_map(|v| v.first().cloned())
        .filter(|v| version::dots(v).is_some())
        .collect();
    version::sort_desc(&mut out);
    Ok(out)
}

pub async fn download(client: &Client, version: &str, server_dir: &Path) -> Result<String> {
    let builds: Vec<Build> = serde_json::from_value(
        net::fetch_json(client, &format!("{BASE}/versions/{version}/builds")).await?,
    )?;
    let pick = builds
        .iter()
        .filter(|b| b.channel == "STABLE")
        .max_by_key(|b| b.id)
        .or_else(|| builds.iter().max_by_key(|b| b.id))
        .context("no usable Paper build for this version")?;
    let dl = pick
        .downloads
        .get("server:default")
        .or_else(|| pick.downloads.values().next())
        .context("no server download in Paper build")?;
    let name = dl.name.clone();
    let sha256 = dl.checksums.sha256.clone();
    let url = dl.url.clone();
    let dest = server_dir.join(&name);
    net::download_file(
        client,
        &url,
        &dest,
        &net::DownloadOpts {
            max_size: 128 << 20,
            sha1: None,
            sha256: Some(&sha256),
            label: "paper jar",
        },
    )
    .await?;
    Ok(name)
}
