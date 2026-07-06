/// Explicit registry of the built-in palettes (design D7).
///
/// One entry per bundled palette: `{id, brand, asset}`. The list order IS the
/// display order in the palette bottom sheet (MARD first, then Artkal S/A/C/M/R,
/// Hama Midi/Maxi/Mini, Perler/Caps/Mini, Nabbi, Yant). Reorder or add a palette
/// by editing this one list (+ copying the asset + wiring pubspec).
///
/// `brand` lives here (not parsed from JSON) so the settings row / sheet can show
/// the current brand on the FIRST frame, synchronously — `parsePalette` does not
/// produce `brand`, and awaiting a JSON parse would leave the label undefined for
/// a frame. `palette_registry_test` asserts each `brand` equals the matching
/// JSON's `brand` field so the two never drift; the "N 色" count stays lazily
/// parsed (see paletteColorCountsProvider) to avoid a hardcoded count drifting.
class PaletteEntry {
  const PaletteEntry({
    required this.id,
    required this.brand,
    required this.asset,
  });

  /// Stable id — also the persisted `paletteId` and the source filename stem.
  final String id;

  /// Display name, byte-equal to the JSON `brand` field (test-asserted).
  final String brand;

  /// rootBundle asset path.
  final String asset;
}

/// Default palette when the user has never chosen one (design D3).
const String kDefaultPaletteId = 'mard';

/// Fixed display order. Any file dropped into `assets/palettes/` that is NOT one
/// of these 14 ids is caught by `palette_assets_test` (design D8).
const List<PaletteEntry> paletteRegistry = [
  PaletteEntry(id: 'mard', brand: 'MARD', asset: 'assets/palettes/mard.json'),
  PaletteEntry(
      id: 'artkal_s', brand: 'Artkal S', asset: 'assets/palettes/artkal_s.json'),
  PaletteEntry(
      id: 'artkal_a', brand: 'Artkal A', asset: 'assets/palettes/artkal_a.json'),
  PaletteEntry(
      id: 'artkal_c', brand: 'Artkal C', asset: 'assets/palettes/artkal_c.json'),
  PaletteEntry(
      id: 'artkal_m', brand: 'Artkal M', asset: 'assets/palettes/artkal_m.json'),
  PaletteEntry(
      id: 'artkal_r', brand: 'Artkal R', asset: 'assets/palettes/artkal_r.json'),
  PaletteEntry(
      id: 'hama', brand: 'Hama Midi', asset: 'assets/palettes/hama.json'),
  PaletteEntry(
      id: 'hama_maxi',
      brand: 'Hama Maxi',
      asset: 'assets/palettes/hama_maxi.json'),
  PaletteEntry(
      id: 'hama_mini',
      brand: 'Hama Mini',
      asset: 'assets/palettes/hama_mini.json'),
  PaletteEntry(
      id: 'perler', brand: 'Perler', asset: 'assets/palettes/perler.json'),
  PaletteEntry(
      id: 'perler_caps',
      brand: 'Perler Caps',
      asset: 'assets/palettes/perler_caps.json'),
  PaletteEntry(
      id: 'perler_mini',
      brand: 'Perler Mini',
      asset: 'assets/palettes/perler_mini.json'),
  PaletteEntry(id: 'nabbi', brand: 'Nabbi', asset: 'assets/palettes/nabbi.json'),
  PaletteEntry(id: 'yant', brand: 'Yant', asset: 'assets/palettes/yant.json'),
];

/// The registry entry for [id], or `null` if [id] is not a built-in palette.
PaletteEntry? paletteEntryById(String id) {
  for (final e in paletteRegistry) {
    if (e.id == id) return e;
  }
  return null;
}

/// The registry entry for [id], falling back to the default ([kDefaultPaletteId])
/// when [id] is unknown (e.g. a persisted id whose palette was removed).
PaletteEntry paletteEntryOrDefault(String id) =>
    paletteEntryById(id) ?? paletteEntryById(kDefaultPaletteId)!;
