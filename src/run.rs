use crate::archive;
use crate::config::Config;
use crate::stats;
use crate::version;
use anyhow::{bail, Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub fn run() -> Result<()> {
    let cfg = Config::load()?;
    let server_dir = Config::server_dir();
    if !server_dir.is_dir() {
        bail!("{} is missing - run 'rustmc setup' first", server_dir.display());
    }

    let java_path: PathBuf = match cfg.java.as_deref() {
        Some(p) if Path::new(p).is_file() => PathBuf::from(p),
        _ => archive::find_java(&server_dir.join("runtime"))
            .context("java not found - run 'rustmc setup'")?,
    };

    let eula_ok = std::fs::read_to_string(server_dir.join("eula.txt"))
        .map(|s| s.lines().any(|l| l.trim() == "eula=true"))
        .unwrap_or(false);
    if !eula_ok {
        bail!(
            "eula.txt not found or EULA not accepted - run 'rustmc setup' or edit {}/eula.txt",
            server_dir.display()
        );
    }

    if !cfg.jar.is_empty() && !server_dir.join(&cfg.jar).is_file() {
        bail!("server jar {} not found - run 'rustmc setup'", cfg.jar);
    }
    if version::parse_mem(&cfg.memory_min).is_none() || version::parse_mem(&cfg.memory_max).is_none()
    {
        bail!("invalid memory setting in server.json");
    }

    let mut cmd = Command::new(&java_path);
    cmd.current_dir(&server_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if cfg.server_type != "neoforge" {
        cmd.arg(format!("-Xms{}", cfg.memory_min))
            .arg(format!("-Xmx{}", cfg.memory_max));
    }
    cmd.args(&cfg.jvm_args);
    if cfg.server_type == "neoforge" {
        if cfg.java_args.is_empty() {
            bail!("neoforge config is missing java_args");
        }
        cmd.args(&cfg.java_args);
    } else {
        if cfg.jar.is_empty() {
            bail!("config is missing a jar to launch");
        }
        cmd.arg("-jar").arg(&cfg.jar);
    }
    cmd.arg("nogui");

    println!(
        "[rustmc] starting {} ({} / {})",
        cfg.name, cfg.server_type, cfg.version
    );
    let mut child = cmd.spawn().context("failed to launch java")?;
    let pid = child.id();

    let stdin = Arc::new(Mutex::new(child.stdin.take().unwrap()));
    let stopping = Arc::new(AtomicBool::new(false));
    let stop_time: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));

    if let Some(out) = child.stdout.take() {
        thread::spawn(move || {
            for line in BufReader::new(out).lines() {
                println!("{}", line.unwrap_or_default());
            }
        });
    }
    if let Some(err) = child.stderr.take() {
        thread::spawn(move || {
            for line in BufReader::new(err).lines() {
                println!("{}", line.unwrap_or_default());
            }
        });
    }

    let mem_max = cfg.memory_max.clone();
    let live = Arc::new(AtomicBool::new(false));
    {
        let stdin = stdin.clone();
        let stopping = stopping.clone();
        let stop_time = stop_time.clone();
        let live = live.clone();
        thread::spawn(move || {
            let input = std::io::stdin();
            loop {
                let mut line = String::new();
                if input.read_line(&mut line).is_err() {
                    return;
                }
                if live.swap(false, Ordering::SeqCst) {
                    println!("[rustmc] live stats off");
                }
                match line.trim() {
                    "" => {}
                    "stats" => stats::print_snapshot(pid, Some(&mem_max)),
                    "stats live" => {
                        live.store(true, Ordering::SeqCst);
                        let l = live.clone();
                        let mem = mem_max.clone();
                        thread::spawn(move || {
                            while l.load(Ordering::SeqCst) {
                                stats::print_snapshot(pid, Some(&mem));
                                thread::sleep(Duration::from_secs(2));
                            }
                        });
                    }
                    "stop" => {
                        send_stop(&stdin, &stopping, &stop_time);
                        return;
                    }
                    _ => {
                        let mut g = stdin.lock().unwrap();
                        let _ = g.write_all(line.as_bytes());
                        let _ = g.flush();
                    }
                }
            }
        });
    }
    {
        let stdin = stdin.clone();
        let stopping = stopping.clone();
        let stop_time = stop_time.clone();
        let _ = ctrlc::set_handler(move || send_stop(&stdin, &stopping, &stop_time));
    }

    println!(
        "[rustmc] console ready - 'stats' snapshot, 'stats live' loop, 'stop' to stop, other input goes to the server"
    );

    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let code = status.code().unwrap_or(1);
                if stopping.load(Ordering::SeqCst) {
                    println!("[rustmc] server stopped");
                } else if code == 0 {
                    println!("[rustmc] server exited");
                } else {
                    println!("[rustmc] server exited with code {code}");
                }
                break code;
            }
            Ok(None) => {
                if stopping.load(Ordering::SeqCst)
                    && stop_time
                        .lock()
                        .unwrap()
                        .is_some_and(|t| t.elapsed() >= Duration::from_secs(60))
                {
                    println!("[rustmc] server did not stop within 60s - killing");
                    let _ = child.kill();
                    *stop_time.lock().unwrap() = None;
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => bail!("could not poll server process: {e}"),
        }
    };

    thread::sleep(Duration::from_millis(150));
    let _ = std::io::stdout().flush();
    std::process::exit(code);
}

fn send_stop(
    stdin: &Arc<Mutex<ChildStdin>>,
    stopping: &AtomicBool,
    stop_time: &Mutex<Option<Instant>>,
) {
    if stopping.swap(true, Ordering::SeqCst) {
        return;
    }
    *stop_time.lock().unwrap() = Some(Instant::now());
    println!("[rustmc] sending stop to server ...");
    if let Ok(mut g) = stdin.lock() {
        let _ = g.write_all(b"stop\n");
        let _ = g.flush();
    }
}
