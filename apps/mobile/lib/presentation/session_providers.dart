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

/// The last successful `GenerateOutput`, consumed by ResultPage.
class GenerateResultNotifier extends Notifier<GenerateOutput?> {
  @override
  GenerateOutput? build() => null;

  void set(GenerateOutput? value) => state = value;
}

final generateResultProvider =
    NotifierProvider<GenerateResultNotifier, GenerateOutput?>(
  GenerateResultNotifier.new,
);
