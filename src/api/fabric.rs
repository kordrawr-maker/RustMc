use crate::net;
use crate::version;
use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::Deserialize;
use std::path::Path;

const META: &str = "https://meta.fabricmc.net";
const MAVEN: &str = "https://maven.fabricmc.net/net/fabricmc";

#[derive(Deserialize)]
struct GameEntry {
    version: String,
    stable: bool,
}

#[derive(Deserialize)]
struct LoaderEntry {
    loader: Loader,
}

#[derive(Deserialize)]
struct Loader {
    version: String,
}

#[derive(Deserialize)]
struct InstallerEntry {
    version: String,
    stable: bool,
}

pub async fn game_versions(client: &Client) -> Result<Vec<String>> {
    let v: Vec<GameEntry> =
        serde_json::from_value(net::fetch_json(client, &format!("{META}/v2/versions/game")).await?)?;
    let mut out: Vec<String> = v
        .iter()
        .filter(|e| e.stable && version::dots(&e.version).is_some())
        .map(|e| e.version.clone())
        .collect();
    version::sort_desc(&mut out);
    Ok(out)
}

pub async fn loader_versions(client: &Client, game: &str) -> Result<Vec<String>> {
    let v: Vec<LoaderEntry> = serde_json::from_value(
        net::fetch_json(client, &format!("{META}/v2/versions/loader/{game}")).await?,
    )?;
    let mut out: Vec<String> = v.iter().map(|e| e.loader.version.clone()).collect();
    version::sort_desc(&mut out);
    Ok(out)
}

pub async fn installer_version(client: &Client) -> Result<String> {
    let v: Vec<InstallerEntry> = serde_json::from_value(
        net::fetch_json(client, &format!("{META}/v2/versions/installer")).await?,
    )?;
    let pick = |stable: bool| -> Option<String> {
        v.iter()
            .filter(|e| e.stable == stable)
            .map(|e| e.version.clone())
            .max_by(|a, b| version::dots(a).cmp(&version::dots(b)))
    };
    pick(true)
        .or_else(|| pick(false))
        .context("no fabric installer version found")
}

pub async fn install(
    client: &Client,
    java: &Path,
    server_dir: &Path,
    game: &str,
    loader: &str,
    installer: &str,
) -> Result<String> {
    if !server_dir.join("server.jar").is_file() {
        super::vanilla::download(client, game, server_dir).await?;
    }
    let url = format!("{MAVEN}/fabric-installer/{installer}/fabric-installer-{installer}.jar");
    let sha1 = net::fetch_sidecar_sha1(client, &url).await?;
    let dest = server_dir.join(format!("fabric-installer-{installer}.jar"));
    net::download_file(
        client,
        &url,
        &dest,
        &net::DownloadOpts {
            max_size: 64 << 20,
            sha1: sha1.as_deref(),
            sha256: None,
            label: "fabric installer",
        },
    )
    .await?;
    super::run_java(
        java,
        server_dir,
        &[
            "-jar".into(),
            format!("fabric-installer-{installer}.jar"),
            "server".into(),
            "-mcversion".into(),
            game.to_string(),
            "-loader".into(),
            loader.to_string(),
        ],
    )?;
    std::fs::remove_file(&dest).ok();
    if !server_dir.join("fabric-server-launch.jar").is_file() {
        bail!("fabric install finished but fabric-server-launch.jar is missing");
    }
    Ok("fabric-server-launch.jar".into())
}
