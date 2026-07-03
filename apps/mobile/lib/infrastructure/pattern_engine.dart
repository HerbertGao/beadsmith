import 'bead_bridge.dart' as ffi;
import 'bead_bridge.dart' show GenerateOutput, GeneratorKind;

/// The bridge `generate` signature — matches `ffi.generate` exactly so a fake
/// can be injected in tests (the only seam that lets assertions land on the
/// bridge's own args, not on a wholesale-replaced [PatternEngine]).
typedef GenerateFn = Future<GenerateOutput> Function({
  required List<int> imageBytes,
  required String paletteJson,
  required int width,
  required int height,
  int? maxColors,
  int? despeckle,
  required GeneratorKind generator,
});

/// Thin wrapper over the M8 bridge `generate`.
///
/// CLAUDE rule 4: the shell holds NO resize / match / stats / render logic — it
/// only forwards the (already-cropped) bytes + size + options to the single
/// engine entry and returns the `GenerateOutput` verbatim.
class PatternEngine {
  const PatternEngine({this.gen = ffi.generate});

  final GenerateFn gen;

  Future<GenerateOutput> generate({
    required List<int> imageBytes,
    required String paletteJson,
    required int width,
    required int height,
    int? maxColors,
    int? despeckle,
    GeneratorKind generator = GeneratorKind.staged,
  }) =>
      gen(
        imageBytes: imageBytes,
        paletteJson: paletteJson,
        width: width,
        height: height,
        maxColors: maxColors,
        despeckle: despeckle,
        generator: generator,
      );
}
