# Beadsmith mobile (apps/mobile)

Flutter shell over the `bead-core` Rust engine, called through `bead-ffi`. The
app is local-first and fully offline: pick image → crop → set size → generate →
preview/counts → copy summary. All generation runs in Rust; the Flutter layers
(`presentation` / `application` / `infrastructure`) hold no image logic.

iOS and Android are both verified targets.

## Prerequisites

- **Flutter** (stable; built against 3.44.x) and **Xcode** with command-line
  tools.
- **Rust** via rustup, with the iOS targets:
  ```bash
  rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
  ```
  (`scripts/build-ios.sh` runs this for you.)
- An **iOS Simulator runtime**, to `flutter run` on iOS. Plugins integrate via
  **Swift Package Manager** (no Podfile / CocoaPods needed for this plugin set).
  Install a runtime on a fresh machine via Xcode > Settings > Components, or:
  ```bash
  xcodebuild -downloadPlatform iOS
  ```
- For **Android**: Android Studio (SDK + NDK + an emulator AVD), the three
  Android rust targets, and `cargo-ndk`:
  ```bash
  rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
  cargo install cargo-ndk
  ```
  Full Android setup lives in [`android/RUST_BUILD.md`](android/RUST_BUILD.md).
  (What is verified — and how — is listed under **Verification status** below.)

## Build & run (iOS)

1. **Build the native static library** from the repo root:
   ```bash
   crates/bead-ffi/scripts/build-ios.sh
   ```
   This cross-compiles `bead-ffi` to `libbead_ffi.a` for device
   (`aarch64-apple-ios`) and both simulator triples. No bridge logic changes —
   the crate just gained a `staticlib` crate-type.

   > PATH caveat (handled by the script): a Homebrew `rustc` on PATH shadows the
   > rustup one, but only the rustup toolchain has the iOS std installed.
   > `build-ios.sh` prepends the rustup toolchain's `sysroot/bin` to PATH so the
   > right `rustc` compiles. If you build manually, do the same.

2. **Run the app** (with a simulator booted):
   ```bash
   cd apps/mobile
   flutter pub get
   flutter run
   ```
   The static library links into the Runner and is loaded at runtime via FRB's
   `ExternalLibrary.process()` (no `dlopen` path — symbols are linked in, kept
   from dead-strip with `-force_load`).

## Build & run (Android)

Android is **not** a "same architecture, different folder" of iOS: it ships a
`cdylib` `.so` under `jniLibs` loaded via `ExternalLibrary.open(...)`, not a
linked `staticlib`. The `.so` is built by `cargo-ndk` (which wires the NDK
toolchain into cargo) and packaged into the APK at build time.

1. **Build the native shared libraries** from the repo root:
   ```bash
   ANDROID_NDK_HOME="$HOME/Library/Android/sdk/ndk/<version>" \
     crates/bead-ffi/scripts/build-android.sh
   ```
   This cross-compiles `bead-ffi` to `libbead_ffi.so` for all three ABIs
   (`arm64-v8a`, `armeabi-v7a`, `x86_64`) and copies them into
   `android/app/src/main/jniLibs/<abi>/`. The same Homebrew/rustup PATH caveat
   as iOS applies and is handled by the script. Details:
   [`android/RUST_BUILD.md`](android/RUST_BUILD.md).

2. **Run the app** (with an emulator booted):
   ```bash
   cd apps/mobile
   flutter pub get
   flutter run -d emulator-5554
   ```
   The `.so` is loaded at runtime via FRB's `ExternalLibrary.open('libbead_ffi.so')`
   (the system resolves the ABI-correct copy from the APK's jniLibs).

Re-run `build-android.sh` after editing Rust code, before the next
`flutter run`/`flutter test` — same cadence as the iOS workflow.

## Verification status

**Verified** on a booted iOS Simulator (iOS 26.5 / iPhone 17) and a booted
Android emulator (Pixel_10, Android 17 / API 37):

- *Automated* —
  `flutter test integration_test/engine_on_ios_test.dart` (iOS) and
  `flutter test integration_test/engine_on_android_test.dart -d emulator-5554`
  (Android): the `bead-ffi` native lib links/ships and loads via the
  platform-correct `ExternalLibrary` call (3.1/7.2), `generate` returns a
  `GenerateOutput` (3.2), and the structural invariants hold — cell count =
  width×height, stats schema, summary format (6.2, iOS + Android halves).
  `generate_ios_regression_test.dart` (no `@TestOn`, runs on any booted
  device) passes on both.
- *Manual* — the full six-step flow (pick → crop → set size → generate →
  preview/counts → copy summary) was run end-to-end on both the iOS simulator
  and the Android emulator. The system photo picker and the interactive crop
  gesture can't be driven from `integration_test`, so this step is confirmed
  by a one-time manual run per platform, **not** an automated widget test.
- *Host-side, automated* — `cargo test`, `flutter analyze`, and the CLI == FFI
  byte-exact determinism gate (`crates/bead-ffi/dart`) — 6.2 host half.

**Deferred** (out of scope this milestone):

- App Store / Play signing + store upload (needs a paid developer account).
