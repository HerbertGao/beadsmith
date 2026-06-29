import 'bead_bridge.dart' as ffi;

/// Thin wrapper over the M8 bridge `generate`.
///
/// CLAUDE rule 4: the shell holds NO resize / match / stats / render logic — it
/// only forwards the (already-cropped) bytes + size to the single engine entry
/// and returns the `GenerateOutput` verbatim.
class PatternEngine {
  const PatternEngine();

  Future<ffi.GenerateOutput> generate({
    required List<int> imageBytes,
    required String paletteJson,
    required int width,
    required int height,
  }) =>
      ffi.generate(
        imageBytes: imageBytes,
        paletteJson: paletteJson,
        width: width,
        height: height,
      );
}
