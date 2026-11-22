#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ -z "${ANDROID_NDK_ROOT:-}" ]]; then
    echo "ANDROID_NDK_ROOT is required." >&2
    exit 2
fi

export ANDROID_NDK_ROOT

source "$HOME/.cargo/env"
cargo install cargo-ndk --locked

cargo ndk -t arm64-v8a -t armeabi-v7a -o android/jniLibs build --release -p mmo-player

echo "Native libraries copied to android/jniLibs"
