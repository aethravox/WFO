# Wayfarers Online

**Wayfarers Online (WFO)** is a cross-platform multiplayer MMORPG prototype written in Rust. It includes a multiplayer game server, native desktop and Android clients, a WebAssembly build, and an offline game mode.

> WFO is an active prototype. APIs, network protocol, save formats, and configuration may change.

## Features

- Account registration and authentication with Argon2id password hashes
- Persistent account progression stored in SQLite
- Native TLS multiplayer and secure WebSocket gameplay
- Offline mode with a separate local profile
- Nine character classes, including Duelist, Summoner, Artificer, Chronicler, and Berserker, with combat, experience, leveling, gold, mobs, crystals, chat, and duels
- Persistent H-to-SSS adventurer ranks earned through world activities
- Zone discovery, a nearby-creature bestiary, personal map markers, emotes, and level-up effects
- LAN discovery support
- CPU rasterization with GPU rendering support on desktop
- Desktop, Android, and WebAssembly targets
- Independent, `no_std` localization and UI support through [`langes`](https://crates.io/crates/langes)

## Supported Platforms

| Target | Status | Notes |
| --- | --- | --- |
| Linux desktop | Supported | Native X11 client |
| Windows desktop | Supported | Native cross-platform client |
| Android | Supported | ARM64 and ARMv7 native libraries |
| WebAssembly | Supported | Browser-compatible player build |

Hardware-accelerated rendering uses `wgpu`. A supported GPU or software-backed graphics backend is required for native rendering.

## Workspace

| Component | Description |
| --- | --- |
| [`crates/mmo-protocol`](crates/mmo-protocol/) | Shared packets, snapshots, world types, and protocol constants |
| [`crates/mmo-server`](crates/mmo-server/) | SQLite-backed game server with TLS, WebSocket, and HTTP endpoints |
| [`crates/mmo-renderer`](crates/mmo-renderer/) | Shared simulation, input, HUD, offline, and rendering logic |
| [`crates/mmo-player`](crates/mmo-player/) | Native desktop and Android player |
| [`crates/mmo-wasm`](crates/mmo-wasm/) | WebAssembly player bindings |
| [`langes`](https://crates.io/crates/langes) | Separate `no_std` localization and UI library (crates.io) |
| [`android/`](android/) | Native Android application project |
## Quick Start

### Prerequisites

- Rust stable and Cargo
- Linux X11, Windows, or Android SDK/NDK for the corresponding target
- A `wgpu`-compatible graphics backend
- TLS certificate and private key to run the multiplayer server

### Start the server

The server requires a PEM-encoded certificate and private key:

```bash
export WFO_TLS_CERT=/absolute/path/to/fullchain.pem
export WFO_TLS_KEY=/absolute/path/to/private-key.pem
export WFO_DB_PATH=/absolute/path/to/wfo-server.sqlite3

cargo run --release -p mmo-server
```

The default listeners are:

| Service | Address | Purpose |
| --- | --- | --- |
| Native multiplayer | `0.0.0.0:7878` | TLS game traffic |
| WebSocket multiplayer | `0.0.0.0:3443` | Secure WebSocket game traffic |
| HTTP | `0.0.0.0:3000` | Status and static web assets |

### Start the native player

```bash
cargo run --release -p mmo-player -- --server 127.0.0.1:7878
```

You can also set the server address through the environment:

```bash
export WFO_SERVER=127.0.0.1:7878
cargo run --release -p mmo-player
```

If the server uses a private certificate authority:

```bash
export WFO_CA_CERT=/absolute/path/to/ca.pem
cargo run --release -p mmo-player -- --server 127.0.0.1:7878
```

For publicly trusted certificates, no CA environment variable is required.

### Gameplay controls

- `WASD` / arrow keys: move; `1`-`3`: use skills; `E`: start/cancel crystal gathering; Shift: dodge
- `M`: place or remove a personal marker; `B`: open or close the bestiary
- Enter the chat commands `/saludar`, `/celebrar`, `/reir`, or `/bailar` to emote

### Run the test suite

```bash
cargo test --workspace
```

## Configuration

### Server variables

| Variable | Default | Description |
| --- | --- | --- |
| `WFO_TLS_CERT` | — | Required path to the PEM server certificate |
| `WFO_TLS_KEY` | — | Required path to the PEM private key |
| `WFO_DB_PATH` | `wfo-server.sqlite3` | SQLite account database path |
| `WFO_TLS_ADDR` | `0.0.0.0:7878` | Native TLS listener address |
| `WFO_WSS_ADDR` | `0.0.0.0:3443` | Secure WebSocket listener address |
| `WFO_WEB_ADDR` | `0.0.0.0:3000` | HTTP listener address |

### Player variables

| Variable | Default | Description |
| --- | --- | --- |
| `WFO_SERVER` | `127.0.0.1:7878` | Multiplayer server address |
| `WFO_CA_CERT` | — | Optional PEM certificate for a private CA |
| `WFO_PROFILE_PATH` | Platform-dependent | Path to the local player profile |
| `WFO_LAN_NAME` | `Wayfarer LAN` | Name advertised for LAN play |

Command-line syntax:

```text
wfo-player [--server HOST:PORT]
```

## Android

The Android project uses `NativeActivity`, Vulkan-backed `wgpu`, landscape orientation, and touch controls.

Install Android SDK/NDK 27 or newer, then export the NDK toolchain location:

```bash
export ANDROID_NDK_ROOT=/absolute/path/to/Android/Sdk/ndk/27.*/toolchains/llvm/prebuilt/linux-x86_64
export JAVA_HOME=/absolute/path/to/jdk

./android/build-android.sh
cd android
gradle assembleDebug
```

For a private TLS CA, place the PEM certificate at `wfo-ca.pem` in the application's private data directory.

## Data and Security

- Online account class, level, XP, and gold are persisted in SQLite.
- Online adventurer rank points are persisted in SQLite; local rank progress is saved with the offline profile.
- Player position, health, mana, shields, and active world state are session-only.
- Local profile data and offline progression are separate from online accounts.
- Passwords are stored as Argon2id hashes; plaintext passwords are not persisted.
- The native game protocol and secure WebSocket connections use TLS.
- The HTTP listener serves status and static files only; it is not a game transport.

Use certificates issued by a trusted CA for public deployments. Keep the TLS private key outside the repository and restrict access to the account database.

## Renderer

The shared renderer is organized around these responsibilities:

- `engine.rs` - client state and world simulation
- `input.rs` - keyboard, mouse, touch, and camera input
- `render.rs` - scene drawing coordination
- `hud.rs` - lobby, game HUD, chat, and overlays
- `raster3d.rs` - framebuffer primitives and CPU compatibility renderer

The native player rasterizes projected scene triangles and depth on the GPU through `wgpu`, while the existing CPU-rendered HUD is composited as a transparent overlay. The CPU framebuffer renderer remains available to WASM and as a compatibility path.

## Repository Structure

```text
WFO/
├── android/              Android application project
├── crates/
│   ├── mmo-protocol/     Shared network and world types
│   ├── mmo-renderer/     Shared game and rendering systems
│   ├── mmo-server/       Multiplayer server
│   ├── mmo-player/       Native desktop/Android player
│   └── mmo-wasm/         WebAssembly bindings
```

## License

Wayfarers Online is proprietary software owned by **Aethravox Studios**. All rights are reserved unless explicitly authorized otherwise. Do not publish, redistribute, sublicense, or reuse source code, assets, or protocol details without permission.

The external [`langes`](https://crates.io/crates/langes) dependency is licensed under MIT or Apache-2.0.
