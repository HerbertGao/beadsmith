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
  // Multi-color fixture (committed; regen via test/gen_rich_fixture.py) for the
  // option-forwarding cases: gradient.png collapses to a single color at the gate
  // sizes, which would make --max-colors/--despeckle/--generator no-ops (vacuous).
  final richPng =
      File('$repoRoot/crates/bead-ffi/dart/test/rich_fixture.png');
  final paletteJsonFile = File('$repoRoot/palettes/artkal_s.json');
  final dylib = _hostDylib(repoRoot);
  final cargo = _which('cargo');

  final missing = <String>[
    if (!gradientPng.existsSync()) 'samples/gradient.png',
    if (!richPng.existsSync()) 'crates/bead-ffi/dart/test/rich_fixture.png',
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

  final paletteJson = paletteJsonFile.readAsStringSync();

  /// Run one gate case: FFI `generate` with the given options, same-machine
  /// `bead-cli generate` with the equivalent flags, then raw-byte compare the
  /// four named files and self-check `pattern_json` vs the FRB structured fields.
  /// [label] tags failures; [inputFile] is the source image (gradient for the
  /// unset default path, the rich fixture for the option cases); [cliFlags] are
  /// the extra CLI flags mirroring the FFI options (empty for the unset path).
  /// When [assertDiffersFromUnset] is set, the option-carrying output is compared
  /// against the SAME fixture run with options unset (null/null/staged) and MUST
  /// differ — the anti-vacuity guard: if an option were dropped the outputs would
  /// match and this fails, so the case can never silently go vacuous.
  Future<void> runGate({
    required String label,
    required File inputFile,
    required int width,
    required int height,
    int? maxColors,
    int? despeckle,
    required GeneratorKind generator,
    required List<String> cliFlags,
    bool assertDiffersFromUnset = false,
  }) async {
    final imageBytes = inputFile.readAsBytesSync();

    // 1. FFI generate.
    final out = await generate(
      imageBytes: imageBytes,
      paletteJson: paletteJson,
      width: width,
      height: height,
      maxColors: maxColors,
      despeckle: despeckle,
      generator: generator,
    );

    // Anti-vacuity backstop: the option-carrying output must differ from the
    // unset (null/null/staged) output on the SAME fixture, proving at least one
    // option took effect (so the fixture isn't degenerate). Per-option isolation
    // — a single dropped option — is caught by the byte-for-byte FFI-vs-CLI
    // compare below (identical flags) and the Rust forwarding tests, not here.
    if (assertDiffersFromUnset) {
      final unset = await generate(
        imageBytes: imageBytes,
        paletteJson: paletteJson,
        width: width,
        height: height,
        maxColors: null,
        despeckle: null,
        generator: GeneratorKind.staged,
      );
      expect(out.patternJson, isNot(equals(unset.patternJson)),
          reason: 'options-set output must differ from unset ($label); '
              'equality here means a forwarded option was ignored (vacuous)');
    }

    // 2. Same-machine CLI run into an isolated temp dir.
    final tmp = Directory.systemTemp.createTempSync('beadsmith_gate_$label');
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
        inputFile.path,
        '--palette',
        paletteJsonFile.path,
        '--width',
        '$width',
        '--height',
        '$height',
        '--output',
        tmp.path,
        ...cliFlags,
      ],
      workingDirectory: repoRoot,
    );
    expect(
      cliResult.exitCode,
      0,
      reason: 'bead-cli generate ($label) failed:\n'
          'stdout: ${cliResult.stdout}\nstderr: ${cliResult.stderr}',
    );

    // 3. RAW-BYTE compare the four named files (no newline normalization).
    //    summary.txt: CLI writes result.summary verbatim (core already put the
    //    trailing newline there) — Dart must NOT append one.
    _expectBytesEqual(
      File('${tmp.path}/pattern.json').readAsBytesSync(),
      utf8.encode(out.patternJson),
      'pattern.json ($label)',
    );
    _expectBytesEqual(
      File('${tmp.path}/summary.txt').readAsBytesSync(),
      utf8.encode(out.summary),
      'summary.txt ($label)',
    );
    _expectBytesEqual(
      File('${tmp.path}/preview.png').readAsBytesSync(),
      out.previewPng,
      'preview.png ($label)',
    );
    _expectBytesEqual(
      File('${tmp.path}/grid.png').readAsBytesSync(),
      out.gridPng,
      'grid.png ($label)',
    );

    // (cross-aspect-ratio sanity: the structured size really tracks the call)
    expect(out.pattern.width, width,
        reason: 'bridge must forward width ($label)');
    expect(out.pattern.height, height,
        reason: 'bridge must forward height ($label)');

    // 4. Parse patternJson and self-check 5 fields against the STRUCTURED values
    //    Dart received via FRB. JSON-side values come from PARSING the string,
    //    NOT from re-serializing the structured arrays.
    final parsed = jsonDecode(out.patternJson) as Map<String, dynamic>;

    expect(parsed['width'], out.pattern.width,
        reason: 'patternJson.width vs structured ($label)');
    expect(parsed['height'], out.pattern.height,
        reason: 'patternJson.height vs structured ($label)');
    expect(parsed['brand'], out.brand,
        reason: 'patternJson.brand vs structured ($label)');

    // cells: parsed JSON int list == structured Uint16List, element-wise.
    final parsedCells = (parsed['cells'] as List).map((e) => e as int).toList();
    expect(parsedCells, out.pattern.cells.toList(),
        reason: 'patternJson.cells vs structured ($label)');

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
        reason: 'stats length mismatch ($label)');
    for (var i = 0; i < parsedStats.length; i++) {
      final pj = parsedStats[i];
      final st = out.stats[i];
      expect(pj['code'], st.code,
          reason: 'stats[$i].code vs structured ($label)');
      expect(pj['name'], st.name,
          reason: 'stats[$i].name vs structured ($label)');
      expect(pj['count'], st.count,
          reason: 'stats[$i].count vs structured ($label)');
    }
  }

  // The pre-existing unset-path gate at both default sizes. Three widened
  // options unset (null/null/staged) is field-identical to the engine default,
  // so this must still align the CLI default path (no --matcher/--max-colors/
  // --despeckle/--generator flags) byte-for-byte — the gate does not regress.
  for (final size in _sizes) {
    final w = size.width;
    final h = size.height;
    test('CLI == FFI byte-for-byte @ ${w}x$h (unset defaults)', () async {
      await runGate(
        label: '${w}x$h',
        inputFile: gradientPng,
        width: w,
        height: h,
        maxColors: null,
        despeckle: null,
        generator: GeneratorKind.staged,
        cliFlags: const [],
      );
    });
  }

  // Options-set case: max_colors + despeckle on the default `staged` path must
  // align `bead-cli generate --max-colors 8 --despeckle 2` byte-for-byte
  // (integer despeckle path + host-stable f32 reduction), proving both options
  // are forwarded and unchanged by the bridge. Runs on the rich fixture (>8
  // matched colors + a <=2-bead speckle) so both options are non-vacuous, and
  // asserts the output differs from the unset run on the same fixture. The
  // structured-vs-pattern_json self-check inside runGate applies here too.
  test('CLI == FFI byte-for-byte @ 16x20 (max_colors=8, despeckle=2)', () async {
    await runGate(
      label: '16x20_mc8_ds2',
      inputFile: richPng,
      width: 16,
      height: 20,
      maxColors: 8,
      despeckle: 2,
      generator: GeneratorKind.staged,
      cliFlags: const ['--max-colors', '8', '--despeckle', '2'],
      assertDiffersFromUnset: true,
    );
  });

  // generator=gerstner: same-machine alignment with `bead-cli generate
  // --generator gerstner` (f32 path, host-canonical only — not cross-target
  // byte-exact). Proves the mirror enum maps and forwards into GenerateOptions.
  // Runs on the rich fixture and asserts gerstner output differs from the unset
  // (staged) run on the same fixture, so a gerstner→staged mis-map cannot pass.
  test('CLI == FFI byte-for-byte @ 16x20 (generator=gerstner)', () async {
    await runGate(
      label: '16x20_gerstner',
      inputFile: richPng,
      width: 16,
      height: 20,
      maxColors: null,
      despeckle: null,
      generator: GeneratorKind.gerstner,
      cliFlags: const ['--generator', 'gerstner'],
      assertDiffersFromUnset: true,
    );
  });
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
