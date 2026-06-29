# Android Rust (bead-ffi) build — TODO / scaffold only

Status: **SCAFFOLD, NOT COMPILED OR VERIFIED.** This milestone (M9) ships
Android as a minimal declarative scaffold only. iOS is the verified path; the
Android toolchain is absent on the build machine, so nothing here has been run.

## What is in place

- `app/src/main/jniLibs/{arm64-v8a,armeabi-v7a,x86_64}/` — empty ABI dirs
  (`.gitkeep`) where the per-ABI `libbead_ffi.so` must land.
- A commented-out Gradle hook in `app/build.gradle.kts` sketching the cargo
  build + copy. It is commented because it has not been validated against
  Gradle and must not break the existing Flutter Android config.

## Prerequisites (deferred to the user's environment)

1. Android SDK + NDK installed (e.g. via Android Studio), `local.properties`
   pointing at the SDK.
2. The three Android rust targets:
   ```
   rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
   ```
3. NDK linkers reachable by cargo — easiest via
   [`cargo-ndk`](https://github.com/bbqsrc/cargo-ndk) or a `.cargo/config.toml`
   with per-target `linker = ...` entries.

## ABI → rust target → jniLibs dir

| Android ABI   | rust target               | output dir                       |
| ------------- | ------------------------- | -------------------------------- |
| `arm64-v8a`   | `aarch64-linux-android`   | `app/src/main/jniLibs/arm64-v8a/`   |
| `armeabi-v7a` | `armv7-linux-androideabi` | `app/src/main/jniLibs/armeabi-v7a/` |
| `x86_64`      | `x86_64-linux-android`    | `app/src/main/jniLibs/x86_64/`      |

## Manual build (until the Gradle hook is enabled and verified)

From the repo root, per target (here using `cargo-ndk`):

```bash
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 \
  -o apps/mobile/android/app/src/main/jniLibs \
  build --release -p bead-ffi
```

or plain cargo per target, then copy
`target/<target>/release/libbead_ffi.so` into the matching jniLibs dir.

The Dart side loads it via `ExternalLibrary.open("libbead_ffi.so")` (task 7.2,
owned by another group). The `.so` is built from the `cdylib` crate-type
already declared in `crates/bead-ffi/Cargo.toml`.

## Verification (NOT done this milestone)

Building, packaging, and on-device run of the Android app are explicitly
deferred until the user installs the SDK + NDK. This does not block iOS.
