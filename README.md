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

Start the server:

```bash
cargo run -p mmo-server
```

Start the native player:

```bash
cargo run -p mmo-player -- --server 127.0.0.1:7878
```

Run all workspace tests:

```bash
cargo test --workspace
```

The native client uses `wgpu` for presentation and no longer uses `softbuffer`.
The CPU raster framebuffer remains a compatibility and WASM rendering path while
the native 3D scene backend is completed.

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
