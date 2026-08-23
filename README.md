> [!WARNING]
> The only website RustMc owns is https://rustmc.vercel.app/ do not download from other sources

![RustMc Icon](https://raw.githubusercontent.com/kordrawr-maker/RustMc/main/Icons/Rustmc.png)

# RustMc

- No hardcoded versions: i wont have to update RustMc for you to use the latest versions on minecraft
- Lightweight: the program is very lightweight and fast meaning your server can run without interruptions


## Install

Needs a Rust toolchain (https://rustup.rs):

```
cargo build --release
```

The binary is `target/release/rustmc` (`rustmc.exe` on Windows). Copy it into an empty folder where the server should live and run it from there.

## Quick start

```
rustmc setup
rustmc run
```

While running "rustmc run"

- `stats` - one snapshot of the server process: uptime, CPU %, RAM (against the configured `-Xmx`), threads, disk read/write
- `stats live` - the same every 2 seconds any input stops it
- `stop` - graceful shutdown (Ctrl+C does the same, force-kills after 60s)
- anything else - forwarded to the server console

## Setup

```
rustmc setup
```

Prompts for server type (Vanilla, Paper, Fabric, NeoForge), version, memory, port and name. Then it downloads a bundled Temurin JRE and the server jar, runs the loader installer for Fabric/NeoForge, writes the EULA, `server.properties` and `server.json`. Everything lives in `server/` next to the binary.

Flags:

- `--force` - wipe an existing `server/` folder and reinstall
- `--dry-run` - print the install plan without downloading anything

## Run

```
rustmc run
```

Streams server logs to the console. Typed input goes to the server console unless it is one of the commands listed above.

## Configuration

`server.json` next to the binary holds the launch config. Memory values use JVM suffixes (`G`, `M`, `K`). Paper gets Aikar's GC flags, other types a standard G1GC set. `server.properties`, `eula.txt` and the world live in `server/`.

## Security

- All downloads are HTTPS against a pinned host allowlist (mojang, papermc, fabricmc, neoforged, adoptium, github)
- Every download is checksum-verified (SHA-1/SHA-256 from the official APIs or maven sidecars)
- Archives are extracted with path traversal protection
- The server is spawned directly with an argument vector, never through a shell
- The EULA must be accepted explicitly during setup
- Config parsing rejects unknown fields

## Supported

- Server types: Vanilla, Paper, Fabric, NeoForge
- OS: Windows x64, Linux x64/aarch64
- Java: bundled (Temurin JRE picked automatically per version: 25 for 26.x, 21 for 1.20.5+, 17 for older)
