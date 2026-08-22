use crate::api::{self, adoptium};
use crate::config::{self, Config};
use crate::net;
use crate::prompt;
use crate::version;
use anyhow::{bail, Context, Result};
use reqwest::Client;

pub async fn run(force: bool, dry_run: bool) -> Result<()> {
    let (os, arch) = platform()?;
    let server_dir = Config::server_dir();
    let cfg_path = Config::path();

    if server_dir.exists() || cfg_path.exists() {
        if !force {
            bail!(
                "server/ or server.json already exists here - rerun with --force to wipe and reinstall"
            );
        }
        if !prompt::yes_no("Delete the existing server folder (including worlds)?", false)? {
            return Ok(());
        }
        let _ = std::fs::remove_dir_all(&server_dir);
        let _ = std::fs::remove_file(&cfg_path);
    }

    println!("rustmc setup - platform: {os}-{arch}");

    let types = ["vanilla", "paper", "fabric", "neoforge"];
    let labels = vec![
        "Vanilla - official Mojang server".to_string(),
        "Paper - optimized server with plugin API".to_string(),
        "Fabric - lightweight mod loader".to_string(),
        "NeoForge - mod loader".to_string(),
    ];
    let server_type = types[prompt::menu_index("Server type:", &labels)?];

    let client = net::client()?;
    let (mc_version, extra) = choose_version(&client, server_type).await?;

    let ram = prompt::ask(
        "Server max memory (JVM -Xmx, e.g. 2G or 2048M):",
        &suggest_ram(),
    )?;
    if version::parse_mem(&ram).is_none() {
        bail!("invalid memory value: {ram}");
    }
    let port: u16 = prompt::ask("Server port:", "25565")?
        .parse()
        .context("invalid port")?;
    let name = prompt::ask("Server name (shows in the server list):", "Minecraft Server")?;
    if !prompt::yes_no("Accept the Minecraft EULA (https://aka.ms/MinecraftEULA)?", true)? {
        bail!("EULA must be accepted to run a server");
    }

    let java_major: u16 = if server_type == "vanilla" {
        api::vanilla::java_major(&client, &mc_version)
            .await?
            .unwrap_or_else(|| version::java_major(&mc_version))
    } else {
        version::java_major(&mc_version)
    };
    if version::parse(&mc_version).map(|v| v < (1, 17, 0)).unwrap_or(false) {
        println!("  note: versions before 1.17 may need Java 8, best-effort only");
    }

    println!("\nInstall plan");
    println!("  server type : {server_type}");
    println!(
        "  version     : {mc_version}{}",
        if extra.is_empty() {
            String::new()
        } else {
            format!(" ({extra})")
        }
    );
    println!("  java        : Temurin JRE {java_major}");
    println!("  memory      : {ram}");
    println!("  port        : {port}");
    println!("  folder      : {}", server_dir.display());

    if dry_run {
        println!("\nDry run - nothing was downloaded.");
        return Ok(());
    }
    if !prompt::yes_no("Install now?", true)? {
        return Ok(());
    }

    std::fs::create_dir_all(&server_dir)?;
    std::fs::create_dir_all(server_dir.join("runtime"))?;

    println!("[1/3] downloading JRE and server jar ...");
    let jre = adoptium::jre(&client, java_major, os, arch).await?;
    let jre_dest = server_dir.join("runtime").join(&jre.name);
    let jre_opts = net::DownloadOpts {
        max_size: 512 << 20,
        sha256: if jre.sha256.is_empty() {
            None
        } else {
            Some(&jre.sha256)
        },
        sha1: None,
        label: "temurin jre",
    };
    let jre_dl = net::download_file(&client, &jre.url, &jre_dest, &jre_opts);
    let jar_dl = async {
        match server_type {
            "vanilla" => api::vanilla::download(&client, &mc_version, &server_dir).await,
            "paper" => api::paper::download(&client, &mc_version, &server_dir).await,
            _ => Ok(String::new()),
        }
    };
    let (jr, jv) = tokio::join!(jre_dl, jar_dl);
    jr?;
    let mut jar = jv?;

    println!("[2/3] extracting JRE ...");
    let runtime_dir = server_dir.join("runtime");
    if jre.name.ends_with(".tar.gz") {
        crate::archive::extract_tar_gz(&jre_dest, &runtime_dir)?;
    } else {
        crate::archive::extract_zip(&jre_dest, &runtime_dir)?;
    }
    std::fs::remove_file(&jre_dest)?;
    let java_path =
        crate::archive::find_java(&runtime_dir).context("could not find java in the extracted JRE")?;

    match server_type {
        "fabric" => {
            println!("[3/3] running the Fabric installer ...");
            let installer = api::fabric::installer_version(&client).await?;
            jar = api::fabric::install(&client, &java_path, &server_dir, &mc_version, &extra, &installer)
                .await?;
        }
        "neoforge" => {
            println!("[3/3] running the NeoForge installer ...");
            api::neoforge::install(&client, &java_path, &server_dir, &extra).await?;
        }
        _ => {}
    }

    let mut cfg = Config {
        name: name.clone(),
        server_type: server_type.to_string(),
        version: mc_version,
        loader: if extra.is_empty() { None } else { Some(extra.clone()) },
        jar,
        java: Some(java_path.to_string_lossy().into_owned()),
        memory_min: ram.clone(),
        memory_max: ram.clone(),
        port,
        jvm_args: config::jvm_flags(server_type),
        java_args: Vec::new(),
    };

    if server_type == "neoforge" {
        let args_file = if cfg!(windows) {
            "win_args.txt"
        } else {
            "unix_args.txt"
        };
        cfg.jar = String::new();
        cfg.java_args = vec![
            "@user_jvm_args.txt".to_string(),
            format!("@libraries/net/neoforged/neoforge/{extra}/{args_file}"),
        ];
        std::fs::write(
            server_dir.join("user_jvm_args.txt"),
            format!("# generated by rustmc\n-Xms{ram}\n-Xmx{ram}\n"),
        )?;
    }

    std::fs::write(
        server_dir.join("eula.txt"),
        "# accepted via rustmc setup\n# https://aka.ms/MinecraftEULA\neula=true\n",
    )?;
    std::fs::write(
        server_dir.join("server.properties"),
        format!("# generated by rustmc\nserver-port={port}\nmotd={name}\nonline-mode=true\n"),
    )?;
    cfg.save()?;

    println!("\nDone. Start the server with: rustmc run");
    println!("Console commands: stats, stats live, stop");
    Ok(())
}

async fn choose_version(client: &Client, server_type: &str) -> Result<(String, String)> {
    match server_type {
        "fabric" => {
            let games = api::fabric::game_versions(client).await?;
            let game = match prompt::menu_or_manual("Minecraft version:", &games)? {
                Some(v) => v,
                None => prompt::ask("Minecraft version:", "")?,
            };
            let loaders = api::fabric::loader_versions(client, &game).await?;
            let loader = match prompt::menu_or_manual("Fabric loader version:", &loaders)? {
                Some(v) => v,
                None => prompt::ask("Fabric loader version:", "")?,
            };
            Ok((game, loader))
        }
        "neoforge" => {
            let all = api::neoforge::all_versions(client).await?;
            let mcs = api::neoforge::unique_mc_versions(&all);
            let mc = match prompt::menu_or_manual("Minecraft version:", &mcs)? {
                Some(v) => v,
                None => prompt::ask("Minecraft version:", "")?,
            };
            let builds = api::neoforge::filter_for_mc(&all, &mc);
            if builds.is_empty() {
                bail!("no NeoForge builds for Minecraft {mc}");
            }
            let build = match prompt::menu_or_manual("NeoForge version:", &builds)? {
                Some(v) => v,
                None => prompt::ask("NeoForge version:", "")?,
            };
            Ok((mc, build))
        }
        _ => {
            let versions = match server_type {
                "vanilla" => api::vanilla::versions(client).await?,
                "paper" => api::paper::versions(client).await?,
                _ => unreachable!(),
            };
            let v = match prompt::menu_or_manual("Minecraft version:", &versions)? {
                Some(v) => v,
                None => prompt::ask("Minecraft version:", "")?,
            };
            Ok((v, String::new()))
        }
    }
}

fn platform() -> Result<(&'static str, &'static str)> {
    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        bail!("unsupported OS - rustmc supports Windows and Linux");
    };
    let arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        bail!("unsupported architecture");
    };
    Ok((os, arch))
}

fn suggest_ram() -> String {
    let total = system_ram().unwrap_or(4 << 30);
    let gb = ((total / 2) >> 30).clamp(1, 8);
    format!("{gb}G")
}

#[cfg(windows)]
fn system_ram() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    unsafe {
        let mut m = std::mem::zeroed::<MEMORYSTATUSEX>();
        m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut m) != 0 {
            Some(m.ullTotalPhys)
        } else {
            None
        }
    }
}

#[cfg(target_os = "linux")]
fn system_ram() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kb: u64 = s.lines().next()?.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn system_ram() -> Option<u64> {
    None
}
