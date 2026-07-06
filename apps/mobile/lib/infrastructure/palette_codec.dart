import 'dart:convert';
import 'dart:ui' show Color;

/// One entry in the parsed palette. The list index == the index
/// `BeadPattern.cells` points at (same order as the engine's `load_palette`,
/// which preserves the JSON `colors` array order).
class PaletteColor {
  const PaletteColor({
    required this.code,
    required this.name,
    required this.rgb,
  });

  final String code;
  final String name;
  final Color rgb;
}

/// Parse the bundled palette JSON (the same string passed to `generate` as
/// `paletteJson`) into [PaletteColor]s in engine order.
///
/// `BeadPattern.cells[i]` is a `u16` index into this list, so the order here
/// MUST match `bead_core::palette::load_palette` — which preserves the JSON
/// `colors` array order. The engine also validates uniqueness of codes and
/// hex validity; this parser is a pure read-only mirror for the UI side and
/// trusts the same JSON the engine already accepted.
///
/// Schema: `{"brand": "...", "colors": [{"code": "S01", "name": "White", "rgb": "#EAEEF3"}, ...]}`.
List<PaletteColor> parsePalette(String json) {
  final root = jsonDecode(json) as Map<String, dynamic>;
  final colors = root['colors'] as List<dynamic>;
  return [
    for (final c in colors)
      PaletteColor(
        code: c['code'] as String,
        name: c['name'] as String,
        rgb: _parseHex(c['rgb'] as String),
      ),
  ];
}

/// Parse `#RRGGBB` or `#RGB` into an opaque [Color] (alpha = 0xFF).
Color _parseHex(String hex) {
  var s = hex.replaceFirst('#', '');
  if (s.length == 3) {
    s = s.split('').map((c) => '$c$c').join();
  }
  final value = int.parse(s, radix: 16);
  return Color(0xFF000000 | value);
}
