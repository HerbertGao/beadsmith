#!/usr/bin/env bash
# Cross-compile bead-ffi to Android shared libs (libbead_ffi.so) and drop them
# into apps/mobile/android/app/src/main/jniLibs/<abi>/. Mirrors build-ios.sh.
# See apps/mobile/android/RUST_BUILD.md for the full picture.
#
# Prerequisites:
#   - Android NDK installed (ANDROID_NDK_HOME or ANDROID_NDK_ROOT pointing at it)
#   - cargo-ndk on PATH:  cargo install cargo-ndk
#   - rust targets:  rustup target add aarch64-linux-android \
#                       armv7-linux-androideabi x86_64-linux-android
#
# Why cargo-ndk: the NDK's standalone toolchain (CC_*, linker) has to be wired
# into cargo per-target — raw `cargo build --target` can't do it without a
# hand-written .cargo/config.toml. cargo-ndk handles the env for us.
#
# Why the rustup dance: a Homebrew `cargo` in PATH shadows the rustup-managed
# one (same caveat as build-ios.sh), so we prepend the rustup sysroot/bin and
# run through `rustup run $TC` to guarantee the android std rlibs are found.
#
# Run with no args to build all three ABIs, or pass specific ABIs
# (arm64-v8a / armeabi-v7a / x86_64) as args.
set -euo pipefail

if [ "$#" -gt 0 ]; then
  ABIS=("$@")
else
  ABIS=(arm64-v8a armeabi-v7a x86_64)
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
JNI_DIR="$REPO_ROOT/apps/mobile/android/app/src/main/jniLibs"

# Resolve the NDK (prefer ANDROID_NDK_HOME; fall back to ANDROID_NDK_ROOT).
if [ -z "${ANDROID_NDK_HOME:-}" ] && [ -z "${ANDROID_NDK_ROOT:-}" ]; then
  echo "$0: set ANDROID_NDK_HOME (or ANDROID_NDK_ROOT) to the NDK path" >&2
  exit 2
fi
: "${ANDROID_NDK_HOME:=$ANDROID_NDK_ROOT}"
export ANDROID_NDK_HOME

# rustup toolchain + sysroot (Homebrew shadowing — see header).
TC="$(rustup show active-toolchain | cut -d' ' -f1)"
SYSROOT="$(rustup run "$TC" rustc --print sysroot)"

# Make sure the rust side is installed for every ABI we're building.
TARGETS=()
for abi in "${ABIS[@]}"; do
  case "$abi" in
    arm64-v8a)    TARGETS+=(aarch64-linux-android);;
    armeabi-v7a)  TARGETS+=(armv7-linux-androideabi);;
    x86_64)       TARGETS+=(x86_64-linux-android);;
    *) echo "$0: unknown ABI '$abi'" >&2; exit 2;;
  esac
done
rustup target add "${TARGETS[@]}"

cd "$REPO_ROOT"
ndk_args=()
for abi in "${ABIS[@]}"; do ndk_args+=( -t "$abi" ); done

PATH="$SYSROOT/bin:$PATH" rustup run "$TC" \
  cargo ndk "${ndk_args[@]}" -o "$JNI_DIR" \
  build --release -p bead-ffi

echo "built:"
for abi in "${ABIS[@]}"; do
  echo "  $JNI_DIR/$abi/libbead_ffi.so"
done
