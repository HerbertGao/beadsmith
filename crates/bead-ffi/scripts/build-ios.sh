#!/usr/bin/env bash
# Cross-compile bead-ffi to iOS static libs (libbead_ffi.a) for linking into
# the apps/mobile iOS Runner. Wraps tasks.md §1.2 (packaging only — no bridge
# logic change). Run with no args to build all three triples, or pass specific
# triples as args.
#
# Why the toolchain dance: a Homebrew `rustc` in PATH shadows the rustup-managed
# one, but only the rustup toolchain has the iOS std installed (via `rustup
# target add`). Compiling rustup's iOS std rlibs with Homebrew's rustc fails
# ("can't find crate for core") on a metadata-version mismatch, so we force the
# rustup toolchain's own rustc by prepending its sysroot/bin to PATH.
#
# Note: the simulator triples (aarch64-apple-ios-sim / x86_64-apple-ios) need
# the iphonesimulator SDK; only the device triple (aarch64-apple-ios) is
# strictly required to link a real-device build.
set -euo pipefail

if [ "$#" -gt 0 ]; then
  TARGETS=("$@")
else
  TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios)
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

rustup target add "${TARGETS[@]}"

TC="$(rustup show active-toolchain | cut -d' ' -f1)"
SYSROOT="$(rustup run "$TC" rustc --print sysroot)"

cd "$REPO_ROOT"
for t in "${TARGETS[@]}"; do
  PATH="$SYSROOT/bin:$PATH" rustup run "$TC" \
    cargo build -p bead-ffi --release --target "$t"
  echo "built: $REPO_ROOT/target/$t/release/libbead_ffi.a"
done
