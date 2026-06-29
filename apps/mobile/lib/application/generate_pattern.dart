import '../infrastructure/bead_bridge.dart' show GenerateOutput;
import '../infrastructure/pattern_engine.dart';

/// Use case: generate a bead pattern from cropped bytes + palette + size.
///
/// MVP has no domain→GenerateOutput mapping (no persistence / SaveProject,
/// YAGNI): presentation consumes the returned `GenerateOutput` directly.
class GeneratePattern {
  const GeneratePattern(this._engine);

  final PatternEngine _engine;

  Future<GenerateOutput> call({
    required List<int> imageBytes,
    required String paletteJson,
    required int width,
    required int height,
  }) =>
      _engine.generate(
        imageBytes: imageBytes,
        paletteJson: paletteJson,
        width: width,
        height: height,
      );
}
