use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub name: String,
    #[serde(rename = "type")]
    pub server_type: String,
    pub version: String,
    #[serde(default)]
    pub loader: Option<String>,
    #[serde(default)]
    pub jar: String,
    #[serde(default)]
    pub java: Option<String>,
    pub memory_min: String,
    pub memory_max: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub jvm_args: Vec<String>,
    #[serde(default)]
    pub java_args: Vec<String>,
}

fn default_port() -> u16 {
    25565
}

impl Config {
    pub fn path() -> PathBuf {
        PathBuf::from("server.json")
    }

    pub fn server_dir() -> PathBuf {
        PathBuf::from("server")
    }

    pub fn load() -> Result<Self> {
        let p = Self::path();
        let data = std::fs::read_to_string(&p)
            .with_context(|| format!("could not read {} - run 'rustmc setup' first", p.display()))?;
        serde_json::from_str(&data).with_context(|| format!("{} is invalid", p.display()))
    }

    pub fn save(&self) -> Result<()> {
        let p = Self::path();
        std::fs::write(&p, serde_json::to_string_pretty(self)?)
            .with_context(|| format!("could not write {}", p.display()))?;
        Ok(())
    }
}

pub fn jvm_flags(server_type: &str) -> Vec<String> {
    let mut flags: Vec<String> = vec![
        "-XX:+UseG1GC",
        "-XX:+ParallelRefProcEnabled",
        "-XX:MaxGCPauseMillis=200",
        "-XX:+UnlockExperimentalVMOptions",
        "-XX:+DisableExplicitGC",
        "-XX:+AlwaysPreTouch",
        "-XX:G1NewSizePercent=30",
        "-XX:G1MaxNewSizePercent=40",
        "-XX:G1HeapRegionSize=8M",
        "-XX:G1ReservePercent=20",
        "-XX:G1HeapWastePercent=5",
        "-XX:G1MixedGCCountTarget=4",
        "-XX:InitiatingHeapOccupancyPercent=15",
        "-XX:G1MixedGCLiveThresholdPercent=90",
        "-XX:G1RSetUpdatingPauseTimePercent=5",
        "-XX:SurvivorRatio=32",
        "-XX:+PerfDisableSharedMem",
        "-XX:MaxTenuringThreshold=1",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    if server_type == "paper" {
        flags.push("-Dusing.aikars.flags=https://mcflags.emc.gs".into());
        flags.push("-Daikars.new.flags=true".into());
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let cfg = Config {
            name: "test".into(),
            server_type: "paper".into(),
            version: "1.21.9".into(),
            loader: None,
            jar: "paper-1.21.9-1.jar".into(),
            java: Some("server/runtime/jdk/bin/java.exe".into()),
            memory_min: "2G".into(),
            memory_max: "2G".into(),
            port: 25565,
            jvm_args: vec!["-XX:+UseG1GC".into()],
            java_args: vec![],
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(back.server_type, "paper");
        assert_eq!(back.port, 25565);
        assert_eq!(back.jar, "paper-1.21.9-1.jar");
    }

    #[test]
    fn rejects_unknown_fields() {
        let json = r#"{"name":"x","type":"paper","version":"1.21.9","memory_min":"2G","memory_max":"2G","port":25565,"hacker_field":1}"#;
        assert!(serde_json::from_str::<Config>(json).is_err());
    }
}
