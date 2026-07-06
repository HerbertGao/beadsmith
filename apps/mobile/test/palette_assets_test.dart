// Task 1.3 + 2.1 — palette data integrity, run under `flutter test`.
//
// Guards two ship blockers (design D8):
//   * the bundled palette set must be EXACTLY the 14 clean registry ids —
//     no missing, no extra, and above all no _unlicensed AGPL file that a
//     `assets/palettes/` directory glob would silently ship;
//   * each bundled palette must be byte-identical to the top-level source
//     (`palettes/<id>.json`) so "CLI == FFI" holds.
// Plus a registry↔JSON brand-consistency check (2.1) and "test the test"
// decoys proving the set check is live, not vacuous.
import 'dart:convert' show jsonDecode;
import 'dart:io';

import 'package:beadsmith/infrastructure/palette_registry.dart';
import 'package:flutter/foundation.dart' show listEquals;
import 'package:flutter_test/flutter_test.dart';

/// On-disk `.json` stems under `assets/palettes` via dart:io — REAL files, so an
/// undeclared file dropped in (e.g. an `_unlicensed` AGPL palette) IS seen.
/// Deliberately NOT AssetManifest/rootBundle: those only surface DECLARED assets
/// and would miss exactly the stray file this check exists to catch.
Set<String> _diskPaletteIds() {
  return Directory('assets/palettes')
      .listSync()
      .whereType<File>()
      .map((f) => f.uri.pathSegments.last)
      .where((n) => n.endsWith('.json'))
      .map((n) => n.substring(0, n.length - '.json'.length))
      .toSet();
}

/// The bundled-set validation shared by BOTH the real assertion and the decoys,
/// so a passing decoy proves THIS exact logic is live. Returns `null` when [disk]
/// equals [required] (both directions); otherwise a human-readable reason.
String? _paletteSetProblem(Set<String> disk, Set<String> required) {
  final extra = disk.difference(required);
  final missing = required.difference(disk);
  if (extra.isEmpty && missing.isEmpty) return null;
  return 'extra=$extra missing=$missing';
}

void main() {
  final requiredIds = {for (final e in paletteRegistry) e.id};

  test('registry declares exactly 14 clean ids', () {
    expect(requiredIds.length, 14);
  });

  test('bundled palette set == the 14 registry ids (no missing/extra/_unlicensed)',
      () {
    expect(_paletteSetProblem(_diskPaletteIds(), requiredIds), isNull);
  });

  test('each bundled palette is byte-identical to the top-level source', () {
    for (final e in paletteRegistry) {
      final bundled = File('assets/palettes/${e.id}.json').readAsBytesSync();
      final source = File('../../palettes/${e.id}.json').readAsBytesSync();
      expect(listEquals(bundled, source), isTrue,
          reason: '${e.id}.json drifted from top-level palettes/${e.id}.json');
    }
  });

  test('registry brand matches each palette JSON brand field (2.1)', () {
    for (final e in paletteRegistry) {
      final json = File('assets/palettes/${e.id}.json').readAsStringSync();
      final brand = (jsonDecode(json) as Map<String, dynamic>)['brand'] as String;
      expect(e.brand, brand,
          reason: '${e.id}: registry brand "${e.brand}" != JSON brand "$brand"');
    }
  });

  // "Test the test": each decoy must make the SAME check fail, proving green ≠
  // vacuous. Both feed _paletteSetProblem — the function the real assertion uses.
  test('decoy (a): an EXTRA on-disk file fails the check (proves "no extra" + real disk)',
      () {
    final decoy = File('assets/palettes/_decoy_extra.json');
    decoy.writeAsStringSync('{}');
    addTearDown(() {
      if (decoy.existsSync()) decoy.deleteSync();
    });
    final disk = _diskPaletteIds();
    expect(disk.contains('_decoy_extra'), isTrue,
        reason: 'listSync must see the undeclared file a glob would ship');
    expect(_paletteSetProblem(disk, requiredIds), isNotNull);
  });

  test('decoy (b): a MISSING required id fails the check (proves "no missing")',
      () {
    final disk = _diskPaletteIds().difference({kDefaultPaletteId});
    expect(_paletteSetProblem(disk, requiredIds), isNotNull);
  });
}
