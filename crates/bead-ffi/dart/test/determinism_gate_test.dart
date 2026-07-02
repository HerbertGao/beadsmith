// Determinism gate (M8's done-when): prove the FFI bridge produces byte-identical
// output to `bead-cli` on the SAME machine, for the SAME input.
//
// For each of two cross-aspect-ratio sizes (16×20 and 30×24) we:
//   1. call the FFI `generate(...)` on M7's fixed fixtures
//      (samples/gradient.png + palettes/artkal_s.json),
//   2. run `bead-cli generate` on the same machine / same input into a temp dir,
//   3. RAW-BYTE compare the four named files (pattern.json / summary.txt /
//      preview.png / grid.png) against the FFI output (no newline normalization),
//   4. PARSE the returned `patternJson` string and assert its
//      width/height/cells/stats/brand equal the STRUCTURED fields Dart received
//      via FRB (parsed value vs structured value — never a re-serialization).
//
// Why these two sizes (per spec / D-Test): 16×20 is the fixture's 4:5 line;
// 30×24 is 5:4 (off-4:5) AND non-square. A bridge that hardcodes a size, derives
// height from width (×5/4), or swaps width↔height fails at least one of them.
// 30×24 also forces a real non-identity center-crop + Triangle resize.
//
// The symbols this test imports (`generate`, `GenerateOutput`, `BeadPattern`,
// `ColorStat`, `BeadFfi`) ARE the stable import surface M9's PatternEngine
// reuses — a regen/rename would make this test fail to compile.
//
// If the host toolchain is unavailable (no built dylib, no cargo, missing
// fixtures), the test SKIPS with a logged reason — it never silently passes.

import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:bead_ffi/src/api.dart';
import 'package:bead_ffi/src/frb_generated.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:test/test.dart';

/// The two sizes the determinism gate is fixed to run (spec 5.1 / D-Test).
const _sizes = <({int width, int height})>[
  (width: 16, height: 20), // fixture's 4:5
  (width: 30, height: 24), // 5:4, off-4:5, non-square
];

/// Walk up from [start] until a dir containing a workspace `Cargo.toml` is found.
Directory? _findRepoRoot(Directory start) {
  var dir = start;
  while (true) {
    final cargo = File('${dir.path}/Cargo.toml');
    if (cargo.existsSync() && cargo.readAsStringSync().contains('[workspace]')) {
      return dir;
    }
    final parent = dir.parent;
    if (parent.path == dir.path) return null; // reached filesystem root
    dir = parent;
  }
}

/// Resolve the freshly-built host dylib for the current platform, or null.
File? _hostDylib(String repoRoot) {
  // bead-ffi declares crate-type cdylib; debug profile is what `cargo build`
  // produces by default and what the gate compares against.
  final candidates = <String>[
    if (Platform.isMacOS) '$repoRoot/target/debug/libbead_ffi.dylib',
    if (Platform.isLinux) '$repoRoot/target/debug/libbead_ffi.so',
    if (Platform.isWindows) '$repoRoot/target/debug/bead_ffi.dll',
  ];
  for (final p in candidates) {
    final f = File(p);
    if (f.existsSync()) return f;
  }
  return null;
}

void main() {
  final repoRootDir = _findRepoRoot(Directory.current);

  // ---- toolchain / fixture availability (5.4: skip + log, never false-green) --
  if (repoRootDir == null) {
    // Emit the reason BEFORE registering — a `test(skip:)` body never runs, so a
    // print inside it would be dead code (the runner shows `skip:` either way).
    // ignore: avoid_print
    print('SKIP: could not locate workspace Cargo.toml from '
        '${Directory.current.path}; cannot run the gate.');
    test('CLI == FFI determinism gate', () {}, skip: 'workspace root not found');
    return;
  }
  final repoRoot = repoRootDir.path;

  final gradientPng = File('$repoRoot/samples/gradient.png');
  final paletteJsonFile = File('$repoRoot/palettes/artkal_s.json');
  final dylib = _hostDylib(repoRoot);
  final cargo = _which('cargo');

  final missing = <String>[
    if (!gradientPng.existsSync()) 'samples/gradient.png',
    if (!paletteJsonFile.existsSync()) 'palettes/artkal_s.json',
    if (dylib == null) 'host dylib (cargo build -p bead-ffi)',
    if (cargo == null) 'cargo on PATH',
  ];
  if (missing.isNotEmpty) {
    // ignore: avoid_print
    print('SKIP: host toolchain/fixtures unavailable: '
        '${missing.join(", ")}. Gate not run.');
    test('CLI == FFI determinism gate', () {},
        skip: 'unavailable: ${missing.join(", ")}');
    return;
  }

  // ---- one-time FRB init against the host dylib --------------------------------
  setUpAll(() async {
    await BeadFfi.init(externalLibrary: ExternalLibrary.open(dylib!.path));
  });

  final imageBytes = gradientPng.readAsBytesSync();
  final paletteJson = paletteJsonFile.readAsStringSync();

  for (final size in _sizes) {
    final w = size.width;
    final h = size.height;

    test('CLI == FFI byte-for-byte @ ${w}x$h', () async {
      // 1. FFI generate.
      final out = await generate(
        imageBytes: imageBytes,
        paletteJson: paletteJson,
        width: w,
        height: h,
      );

      // 2. Same-machine CLI run into an isolated temp dir.
      final tmp = Directory.systemTemp.createTempSync('beadsmith_gate_${w}x$h');
      addTearDown(() => tmp.deleteSync(recursive: true));

      final cliResult = Process.runSync(
        cargo!,
        [
          'run',
          '-q',
          '-p',
          'bead-cli',
          '--',
          'generate',
          '--input',
          gradientPng.path,
          '--palette',
          paletteJsonFile.path,
          '--width',
          '$w',
          '--height',
          '$h',
          '--output',
          tmp.path,
        ],
        workingDirectory: repoRoot,
      );
      expect(
        cliResult.exitCode,
        0,
        reason: 'bead-cli generate @ ${w}x$h failed:\n'
            'stdout: ${cliResult.stdout}\nstderr: ${cliResult.stderr}',
      );

      // 3. RAW-BYTE compare the four named files (no newline normalization).
      //    summary.txt: CLI writes result.summary verbatim (core already put the
      //    trailing newline there) — Dart must NOT append one.
      _expectBytesEqual(
        File('${tmp.path}/pattern.json').readAsBytesSync(),
        utf8.encode(out.patternJson),
        'pattern.json @ ${w}x$h',
      );
      _expectBytesEqual(
        File('${tmp.path}/summary.txt').readAsBytesSync(),
        utf8.encode(out.summary),
        'summary.txt @ ${w}x$h',
      );
      _expectBytesEqual(
        File('${tmp.path}/preview.png').readAsBytesSync(),
        out.previewPng,
        'preview.png @ ${w}x$h',
      );
      _expectBytesEqual(
        File('${tmp.path}/grid.png').readAsBytesSync(),
        out.gridPng,
        'grid.png @ ${w}x$h',
      );

      // (cross-aspect-ratio sanity: the structured size really tracks the call)
      expect(out.pattern.width, w, reason: 'bridge must forward width @ ${w}x$h');
      expect(out.pattern.height, h,
          reason: 'bridge must forward height @ ${w}x$h');

      // 4. Parse patternJson and self-check 5 fields against the STRUCTURED values
      //    Dart received via FRB. JSON-side values come from PARSING the string,
      //    NOT from re-serializing the structured arrays.
      final parsed = jsonDecode(out.patternJson) as Map<String, dynamic>;

      expect(parsed['width'], out.pattern.width,
          reason: 'patternJson.width vs structured @ ${w}x$h');
      expect(parsed['height'], out.pattern.height,
          reason: 'patternJson.height vs structured @ ${w}x$h');
      expect(parsed['brand'], out.brand,
          reason: 'patternJson.brand vs structured @ ${w}x$h');

      // cells: parsed JSON int list == structured Uint16List, element-wise.
      final parsedCells =
          (parsed['cells'] as List).map((e) => e as int).toList();
      expect(parsedCells, out.pattern.cells.toList(),
          reason: 'patternJson.cells vs structured @ ${w}x$h');

      // stats: parsed {code,name,count} list == structured ColorStat list.
      // Index-paired (not keyed by code) on purpose: stats order is load-bearing
      // — pattern.json is byte-compared above, so its stats order is pinned to
      // the engine's `Vec` order, and the FRB-structured `out.stats` is that same
      // Vec marshalled. A divergent marshalling order would mismatch here and FAIL
      // (it can't be masked); a consistent order is exactly what the gate requires.
      final parsedStats = (parsed['stats'] as List)
          .map((e) => e as Map<String, dynamic>)
          .toList();
      expect(parsedStats.length, out.stats.length,
          reason: 'stats length mismatch @ ${w}x$h');
      for (var i = 0; i < parsedStats.length; i++) {
        final pj = parsedStats[i];
        final st = out.stats[i];
        expect(pj['code'], st.code,
            reason: 'stats[$i].code vs structured @ ${w}x$h');
        expect(pj['name'], st.name,
            reason: 'stats[$i].name vs structured @ ${w}x$h');
        expect(pj['count'], st.count,
            reason: 'stats[$i].count vs structured @ ${w}x$h');
      }
    });
  }
}

/// Locate an executable on PATH, returning its name (Process.run resolves it) or
/// null if not found. Uses `command -v` via the shell to honor PATH.
String? _which(String exe) {
  try {
    // Windows has no `/bin/sh`; use `where`. POSIX honors PATH via `command -v`.
    final r = Platform.isWindows
        ? Process.runSync('where', [exe])
        : Process.runSync('/bin/sh', ['-c', 'command -v $exe']);
    if (r.exitCode == 0 && (r.stdout as String).trim().isNotEmpty) return exe;
  } catch (_) {/* fall through */}
  return null;
}

/// Assert two byte sequences are identical, with a length + first-diff message.
void _expectBytesEqual(List<int> actual, List<int> expected, String label) {
  if (actual.length != expected.length) {
    fail('$label: length ${actual.length} (CLI file) != '
        '${expected.length} (FFI). CLI != FFI is a BLOCKER, not a mismatch to '
        'normalize away.');
  }
  for (var i = 0; i < actual.length; i++) {
    if (actual[i] != expected[i]) {
      fail('$label: first byte diff at index $i: '
          'CLI=${actual[i]} vs FFI=${expected[i]}. CLI != FFI is a BLOCKER.');
    }
  }
  // Use a typed-data equality as the recorded assertion too.
  expect(Uint8List.fromList(actual), equals(Uint8List.fromList(expected)),
      reason: '$label: CLI file bytes must equal FFI output bytes');
}
