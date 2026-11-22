# Wayfarers Online (WFO)

Wayfarers Online is a Rust MMO prototype with native and WebAssembly players.
The repository contains:

- `crates/mmo-protocol`: shared packet and world types.
- `crates/mmo-server`: TCP and WebSocket game server.
- `crates/mmo-renderer`: client simulation and renderer modules.
- `crates/mmo-player`: native desktop client.
- `crates/mmo-wasm`: WebAssembly player bindings.
- `Lang`: independent no-std localization and UI library.

## Requirements

- Rust stable with Cargo.
- Linux X11 is supported by the native player.
- A GPU or software-backed `wgpu` backend.

## Run

The server stores accounts and progression in SQLite. Online sessions require a
TLS certificate and key; the password is only sent over the encrypted game
connection:

```bash
export WFO_TLS_CERT=/path/to/fullchain.pem
export WFO_TLS_KEY=/path/to/private-key.pem
export WFO_DB_PATH=/path/to/wfo-server.sqlite3
cargo run -p mmo-server
```

Start the native player. Add `WFO_CA_CERT` when the server uses a private CA;
publicly trusted certificates are verified using the built-in root list:

```bash
export WFO_CA_CERT=/path/to/ca.pem
cargo run -p mmo-player -- --server 127.0.0.1:7878
```

The lobby supports account creation and login. Passwords are stored as Argon2id
hashes. SQLite persists account class, level, XP and gold; position, health,
mana and world state remain session-only. The local profile stores its username, class, level, XP and gold, never the
password. Offline mode has its own playable world and saves that progress only
to the local profile; it is separate from online accounts and is never
synchronized. Position, health, mana and world state are not persisted.
The HTTP listener on port 3000 serves status/static files only, native game
traffic uses TLS on port 7878, and secure WebSockets use port 3443.

Run all workspace tests:

```bash
cargo test --workspace
```

The native client rasterizes projected scene triangles and depth on the GPU
through `wgpu`, then composites the existing CPU-rendered HUD as a transparent
overlay. The CPU framebuffer renderer remains available to WASM and as a
compatibility path; scene projection and shading are still computed on the CPU.

## Android

The Android project is in `android/`. It uses `NativeActivity`, Vulkan-backed
`wgpu`, landscape orientation and touch controls.

With Android SDK/NDK 27+ and Gradle installed:

```bash
export ANDROID_NDK_ROOT=/path/to/Android/Sdk/ndk/27.*/toolchains/llvm/prebuilt/linux-x86_64
./android/build-android.sh
cd android
gradle assembleDebug
```

For a private TLS CA on Android, place its PEM certificate at
`wfo-ca.pem` in the app's private data directory. No password is stored in the
local profile.

## Renderer layout

- `engine.rs`: client state and world simulation.
- `input.rs`: keyboard, mouse, touch and camera input.
- `render.rs`: scene drawing coordination.
- `hud.rs`: lobby, game HUD and overlays.
- `raster3d.rs`: framebuffer primitives and CPU compatibility renderer.

## Proprietary project

This project is proprietary. Contributors and integrators may review this
repository only according to the permissions granted by the project owner.
Do not publish, redistribute or reuse source code, assets or protocol details
without authorization.

The separate `Lang/` library is excluded from this proprietary licensing and
is licensed under either MIT or Apache-2.0, as declared in its own Cargo
workspace.
