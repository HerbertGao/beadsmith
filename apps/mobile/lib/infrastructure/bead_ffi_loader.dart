import 'dart:io' show Platform;

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;

import 'bead_bridge.dart' show BeadFfi;

/// Initialize the `bead_ffi` bridge for this platform.
///
/// FRB 2.12.0 takes an `ExternalLibrary?` (NOT a `DynamicLibrary`). The M8
/// pure-Dart glue resolved a host dylib by file path; on device we inject the
/// platform-correct loader instead:
///   - iOS: the staticlib is linked into the Runner, so symbols live in the
///     process image — resolve them with `ExternalLibrary.process` (task 3.2).
///   - Android: the cdylib ships as `libbead_ffi.so` under jniLibs — load it by
///     name with `ExternalLibrary.open` (task 7.2).
Future<void> initBeadFfi() async {
  final ExternalLibrary library;
  if (Platform.isIOS) {
    // Static-linked into Runner; symbols are in-process.
    library = ExternalLibrary.process(iKnowHowToUseIt: true);
  } else if (Platform.isAndroid) {
    library = ExternalLibrary.open('libbead_ffi.so');
  } else {
    throw UnsupportedError(
      'bead_ffi loader supports iOS and Android only (got ${Platform.operatingSystem})',
    );
  }
  await BeadFfi.init(externalLibrary: library);
}
