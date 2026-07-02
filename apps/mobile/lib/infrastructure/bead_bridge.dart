// Local facade over the M8 pure-Dart glue (`bead_ffi`), which ships no public
// barrel — only `lib/src/*`. Importing another package's `src/` trips the
// `implementation_imports` lint, so it is suppressed here, in ONE place, and the
// rest of the app imports clean symbols from this file.
//
// ponytail: a thin re-export, not a wrapper — we cannot add a barrel to the
// crate (out of scope). Drop this file the day `bead_ffi` exposes `bead_ffi.dart`.
// ignore_for_file: implementation_imports
export 'package:bead_ffi/src/api.dart'
    show generate, GenerateOutput, BeadPattern, ColorStat, GeneratorKind;
export 'package:bead_ffi/src/frb_generated.dart' show BeadFfi;
