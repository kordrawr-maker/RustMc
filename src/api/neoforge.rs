use crate::net;
use crate::version;
use anyhow::{bail, Result};
use reqwest::Client;
use serde::Deserialize;
use std::path::Path;

const VERSIONS_URL: &str =
    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
const MAVEN_BASE: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge";

#[derive(Deserialize)]
struct Versions {
    versions: Vec<String>,
}

pub async fn all_versions(client: &Client) -> Result<Vec<String>> {
    let v: Versions = serde_json::from_value(net::fetch_json(client, VERSIONS_URL).await?)?;
    let mut out = v.versions;
    sort_neoforge_desc(&mut out);
    Ok(out)
}

pub fn mc_version_of(v: &str) -> Option<String> {
    let d = numeric_components(v)?;
    match d.as_slice() {
        [major, minor, _build] if *major <= 21 => legacy_mc_version(*major, *minor),
        [major, minor, _build] => Some(format!("{major}.{minor}")),
        [major, minor, patch, ..] if *patch == 0 => Some(format!("{major}.{minor}")),
        [major, minor, patch, ..] => Some(format!("{major}.{minor}.{patch}")),
        _ => None,
    }
}

fn legacy_mc_version(major: u32, minor: u32) -> Option<String> {
    Some(if minor == 0 {
        format!("1.{major}")
    } else {
        format!("1.{major}.{minor}")
    })
}

fn numeric_components(v: &str) -> Option<Vec<u32>> {
    let mut out = Vec::new();
    for part in v.split('.') {
        let digits = part
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>();
        if digits.is_empty() {
            return None;
        }
        out.push(digits.parse().ok()?);
    }
    if out.is_empty() { None } else { Some(out) }
}

fn sort_neoforge_desc(v: &mut [String]) {
    v.sort_by_key(|s| std::cmp::Reverse(numeric_components(s)));
}

pub fn unique_mc_versions(versions: &[String]) -> Vec<String> {
    let mut out: Vec<String> = versions.iter().filter_map(|v| mc_version_of(v)).collect();
    version::sort_desc(&mut out);
    out.dedup();
    out
}

pub fn filter_for_mc(versions: &[String], mc: &str) -> Vec<String> {
    versions
        .iter()
        .filter(|v| mc_version_of(v).as_deref() == Some(mc))
        .cloned()
        .collect()
}

pub async fn install(client: &Client, java: &Path, server_dir: &Path, version: &str) -> Result<()> {
    let url = format!("{MAVEN_BASE}/{version}/neoforge-{version}-installer.jar");
    let sha1 = net::fetch_sidecar_sha1(client, &url).await?;
    let dest = server_dir.join(format!("neoforge-{version}-installer.jar"));
    net::download_file(
        client,
        &url,
        &dest,
        &net::DownloadOpts {
            max_size: 256 << 20,
            sha1: sha1.as_deref(),
            sha256: None,
            label: "neoforge installer",
        },
    )
    .await?;
    super::run_java(
        java,
        server_dir,
        &[
            "-jar".into(),
            format!("neoforge-{version}-installer.jar"),
            "--installServer".into(),
        ],
    )?;
    std::fs::remove_file(&dest).ok();
    let args_file = if cfg!(windows) {
        "win_args.txt"
    } else {
        "unix_args.txt"
    };
    let p = server_dir
        .join("libraries")
        .join("net/neoforged/neoforge")
        .join(version)
        .join(args_file);
    if !p.is_file() {
        bail!("NeoForge install finished but {} is missing", p.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_neoforge_to_mc() {
        assert_eq!(mc_version_of("21.1.115").as_deref(), Some("1.21.1"));
        assert_eq!(mc_version_of("20.4.8").as_deref(), Some("1.20.4"));
        assert_eq!(mc_version_of("21.0.23").as_deref(), Some("1.21"));
        assert_eq!(mc_version_of("26.2.7").as_deref(), Some("26.2"));
        assert_eq!(mc_version_of("26.2.0.62").as_deref(), Some("26.2"));
        assert_eq!(mc_version_of("26.2.0.17-beta").as_deref(), Some("26.2"));
        assert_eq!(mc_version_of("26.2.1.4").as_deref(), Some("26.2.1"));
        assert_eq!(mc_version_of("garbage"), None);
    }

    #[test]
    fn filters_by_mc_version() {
        let all = vec![
            "21.1.115".to_string(),
            "21.1.100".to_string(),
            "20.4.8".to_string(),
            "26.2.0.62".to_string(),
            "26.2.0.17-beta".to_string(),
        ];
        let m = filter_for_mc(&all, "1.21.1");
        assert_eq!(m.len(), 2);
        assert!(m.contains(&"21.1.115".to_string()));
        let m = filter_for_mc(&all, "26.2");
        assert_eq!(m.len(), 2);
        assert!(m.contains(&"26.2.0.62".to_string()));
    }
}
