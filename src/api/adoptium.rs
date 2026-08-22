use crate::net;
use anyhow::{bail, Result};
use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
struct Assets(Vec<Asset>);

#[derive(Deserialize)]
struct Asset {
    binaries: Vec<Binary>,
}

#[derive(Deserialize)]
struct Binary {
    package: Package,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    link: String,
    #[serde(default)]
    checksum: String,
}

pub struct Jre {
    pub name: String,
    pub url: String,
    pub sha256: String,
}

pub async fn jre(client: &Client, major: u16, os: &str, arch: &str) -> Result<Jre> {
    let url = format!(
        "https://api.adoptium.net/v3/assets/feature_releases/{major}/ga?architecture={arch}&heap_size=normal&image_type=jre&jvm_impl=hotspot&os={os}&page=0&page_size=1&project=jdk&sort_method=DEFAULT&sort_order=DESC&vendor=eclipse"
    );
    let assets: Assets = serde_json::from_value(net::fetch_json(client, &url).await?)?;
    let suffix = if os == "windows" { ".zip" } else { ".tar.gz" };
    for asset in assets.0 {
        for binary in asset.binaries {
            let p = binary.package;
            if p.name.ends_with(suffix) && !p.link.is_empty() {
                return Ok(Jre {
                    name: p.name,
                    url: p.link,
                    sha256: p.checksum,
                });
            }
        }
    }
    bail!("no {os} {arch} JRE available for Java {major}")
}
