# Android Rust (bead-ffi) build

Status: **VERIFIED.** The `libbead_ffi.so` per-ABI build + jniLibs packaging +
on-device load + four-screen flow all run on an Android emulator (Pixel_10,
Android 17 / API 37, arm64-v8a). All three ABIs compile.

## What is in place

- `app/src/main/jniLibs/{arm64-v8a,armeabi-v7a,x86_64}/` — per-ABI
  `libbead_ffi.so` produced by `crates/bead-ffi/scripts/build-android.sh`.
  The `.so` files are gitignored (built artifacts); the dirs are kept.
- `crates/bead-ffi/scripts/build-android.sh` — the verified build path
  (mirrors `build-ios.sh`). Builds the cdylib for each ABI via `cargo-ndk`
  and copies it into jniLibs.
- `lib/infrastructure/bead_ffi_loader.dart` loads it by name via
  `ExternalLibrary.open('libbead_ffi.so')` (task 7.2).

## Prerequisites

1. Android SDK + NDK installed (e.g. via Android Studio), `local.properties`
   pointing at the SDK, and `ANDROID_NDK_HOME` (or `ANDROID_NDK_ROOT`) set
   to the NDK path.
2. The three Android rust targets:
   ```bash
   rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
   ```
3. `cargo-ndk` (wires the NDK's standalone toolchain into cargo per-target):
   ```bash
   cargo install cargo-ndk
   ```

## Build

From the repo root:

```bash
ANDROID_NDK_HOME="$HOME/Library/Android/sdk/ndk/<version>" \
  bash crates/bead-ffi/scripts/build-android.sh
```

Run with no args to build all three ABIs, or pass specific ABIs
(`arm64-v8a` / `armeabi-v7a` / `x86_64`) as args. The script handles the
`rustup target add` + the Homebrew/rustup cargo shadowing (same dance as
`build-ios.sh` — a Homebrew `cargo` in PATH shadows the rustup one, so the
script runs through `rustup run $TC` with the sysroot prepended).

## ABI → rust target → jniLibs dir

| Android ABI   | rust target               | output dir                          |
| ------------- | ------------------------- | ----------------------------------- |
| `arm64-v8a`   | `aarch64-linux-android`   | `app/src/main/jniLibs/arm64-v8a/`   |
| `armeabi-v7a` | `armv7-linux-androideabi` | `app/src/main/jniLibs/armeabi-v7a/` |
| `x86_64`      | `x86_64-linux-android`    | `app/src/main/jniLibs/x86_64/`      |

## NDK version note

`build-android.sh` builds with whatever NDK `ANDROID_NDK_HOME` points at
(verified with NDK 30.0.14904198). The Flutter Android Gradle plugin
declares its own `ndkVersion = flutter.ndkVersion` in `app/build.gradle.kts`
(auto-installed as 28.2.13676358 on first build) — Gradle does NOT compile
native code for this app (the `.so` is prebuilt in jniLibs), so the two NDK
versions do not conflict; Gradle's NDK is only there for ABI packaging.

## Verification (done)

- *Automated* — `flutter test integration_test/engine_on_android_test.dart
  -d emulator-5554`: the `libbead_ffi.so` ships in the APK and loads via
  `ExternalLibrary.open('libbead_ffi.so')` (7.2), `generate` returns a
  `GenerateOutput`, and the structural invariants hold — cell count =
  width×height, stats schema, summary format (6.2, Android half).
  `generate_ios_regression_test.dart` (no `@TestOn`, runs on any booted
  device) also passes on Android.
- *Manual* — the full six-step flow (pick → crop → set size → generate →
  preview/counts → copy summary) was run end-to-end on the Pixel_10
  emulator via `flutter run -d emulator-5554`. No exceptions in the Flutter
  log.
- *Host-side, automated* — `cargo test`, `flutter analyze`, and the CLI ==
  FFI byte-exact determinism gate (`crates/bead-ffi/dart`) — unchanged
  (6.2 host half; the Android path uses the same engine code).

> **`@TestOn` gotcha:** do NOT add `@TestOn('android')` to integration tests
> — `flutter test` pre-filters the selector at the host level (macOS) and
> silently skips the test ("No tests were found") without building. The
> `engine_on_android_test.dart` guards with a runtime `Platform.isAndroid`
> check instead. See `generate_ios_regression_test.dart`'s header for the
> same root cause on iOS.

## Gradle auto-build hook (not wired)

`build-android.sh` is the verified path, run manually before
`flutter run` / `flutter test`. A Gradle `preBuild` hook that auto-runs the
cargo build is intentionally NOT wired: it would slow every Gradle build
and add config complexity for little gain on a small cdylib. Run the script
after editing Rust code, same as the iOS workflow runs `build-ios.sh`.
