import 'dart:typed_data';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../infrastructure/bead_bridge.dart' show GenerateOutput;

/// Holds a nullable image-byte buffer for one navigation hop.
class ImageBytesNotifier extends Notifier<Uint8List?> {
  @override
  Uint8List? build() => null;

  void set(Uint8List? value) => state = value;
}

/// Raw bytes picked on HomePage, consumed by CropPage.
final pickedImageProvider =
    NotifierProvider<ImageBytesNotifier, Uint8List?>(ImageBytesNotifier.new);

/// Cropped bytes from CropPage, consumed by GeneratePage.
///
/// Carried via session state, NOT `go_router` extra — survives rebuilds /
/// deep-links (design D5 / task 4.4).
final croppedImageProvider =
    NotifierProvider<ImageBytesNotifier, Uint8List?>(ImageBytesNotifier.new);

/// The crop frame's aspect (width / height) chosen on CropPage, consumed by
/// GeneratePage to lock the bead-grid aspect. Default = square (1.0) so a
/// deep-link straight to /generate (bypassing crop) is still well-defined and
/// the legacy 40×40 default stays legal.
class CropAspectNotifier extends Notifier<double> {
  @override
  double build() => 1.0;

  void set(double value) => state = value;
}

final cropAspectProvider =
    NotifierProvider<CropAspectNotifier, double>(CropAspectNotifier.new);

/// A generation result pinned to the palette JSON actually passed to `generate`
/// (design D6). ResultPage parses THIS palette — never the live selection — so
/// changing the palette after generating can't recolor an existing result.
/// A Dart-side wrapper; the FFI `GenerateOutput` type is untouched.
class GenerateResult {
  const GenerateResult({required this.output, required this.paletteJson});

  final GenerateOutput output;
  final String paletteJson;
}

/// The last successful result (output + pinned palette), consumed by ResultPage.
class GenerateResultNotifier extends Notifier<GenerateResult?> {
  @override
  GenerateResult? build() => null;

  void set(GenerateResult? value) => state = value;
}

final generateResultProvider =
    NotifierProvider<GenerateResultNotifier, GenerateResult?>(
  GenerateResultNotifier.new,
);
