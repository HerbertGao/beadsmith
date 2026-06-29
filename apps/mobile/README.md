# Beadsmith mobile (apps/mobile)

Flutter shell over the `bead-core` Rust engine, called through `bead-ffi`. The
app is local-first and fully offline: pick image → crop → set size → generate →
preview/counts → copy summary. All generation runs in Rust; the Flutter layers
(`presentation` / `application` / `infrastructure`) hold no image logic.

iOS is the verified target. Android is scaffold-only (see below).

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

## Android (scaffold only — deferred)

Android is **not** a "same architecture, different folder" of iOS: it uses a
`cdylib` `.so` under `jniLibs` loaded via `ExternalLibrary.open(...)`, not a
linked `staticlib`. This milestone ships only the minimal declarative scaffold
(jniLibs ABI dirs + a commented Gradle hook + the loader branch). It has **not**
been compiled or run.

To build it you need the Android SDK + NDK and the Android rust targets. Full
steps live in [`android/RUST_BUILD_TODO.md`](android/RUST_BUILD_TODO.md).

## Verification status

**Verified** on a booted iOS Simulator (iOS 26.5 / iPhone 17):

- *Automated* — `flutter test integration_test/engine_on_ios_test.dart`: the
  `bead-ffi` staticlib links into the Runner and loads via
  `ExternalLibrary.process()` (3.1 link + `-force_load`), `generate` returns a
  `GenerateOutput` (3.2), and the iOS structural invariants hold — cell count =
  width×height, stats schema, summary format (6.2, iOS half).
- *Manual* — the full six-step flow (pick → crop → set size → generate →
  preview/counts → copy summary) was run end-to-end on the simulator. The system
  photo picker and the interactive crop gesture can't be driven from
  `integration_test`, so this step is confirmed by a one-time manual run, **not**
  an automated widget test.
- *Host-side, automated* — `cargo test`, `flutter analyze`, and the CLI == FFI
  byte-exact determinism gate (`crates/bead-ffi/dart`) — 6.2 host half.

**Deferred** (out of scope this milestone):

- App Store / Play signing + store upload (needs a paid developer account).
- Android on-device — scaffold only; needs Android SDK + NDK (see
  [`android/RUST_BUILD_TODO.md`](android/RUST_BUILD_TODO.md)).
