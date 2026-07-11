// Task 6.2 — i18n coverage self-check (mechanism b, per spec 需求2 场景
// "界面无残留硬编码中文").
//
// Scans every lib/presentation/*.dart source, STRIPS comments (// /// and
// /* */) and preserves string literals, then asserts no CJK ideograph survives.
// After comment removal, any CJK in valid Dart source can only sit inside a
// string literal (identifiers/operators are never CJK here) — so a single
// leftover CJK char == a hardcoded Chinese UI string. This is NOT a naive
// line-level `[一-龥]` grep (spec forbids it): comments are dropped first, and a
// `//` or `/*` INSIDE a string does not fool the stripper (see self-check).
//
// No allowlist is needed: brand names live in infrastructure/, and the two
// pre-l10n crash fallback screens live in main.dart — neither is under
// lib/presentation. If a legitimate CJK literal is ever added here, add its
// file/substring to an allowlist rather than weakening the scan.
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

final _cjk = RegExp(r'[㐀-鿿豈-﫿]');

/// Returns [src] with `//`/`/* */` comments removed and string literals kept
/// verbatim. A hand-rolled scanner (Dart ships no lexer): it tracks string
/// state so a comment marker inside a string is copied through, and CJK inside
/// a comment is dropped.
String stripComments(String src) {
  final out = StringBuffer();
  final n = src.length;
  var i = 0;
  while (i < n) {
    final c = src[i];
    // String literal — copy through so a // or /* inside it isn't a comment,
    // and its CJK (a real violation) is preserved for the check.
    if (c == "'" || c == '"') {
      final quote = c;
      final raw = i > 0 && (src[i - 1] == 'r' || src[i - 1] == 'R');
      final triple =
          i + 2 < n && src[i + 1] == quote && src[i + 2] == quote;
      final close = triple ? quote * 3 : quote;
      out.write(close);
      i += close.length;
      while (i < n) {
        if (!raw && src[i] == r'\' && i + 1 < n) {
          out.write(src[i]);
          out.write(src[i + 1]);
          i += 2;
          continue;
        }
        if (src.startsWith(close, i)) {
          out.write(close);
          i += close.length;
          break;
        }
        out.write(src[i]);
        i++;
      }
      continue;
    }
    if (c == '/' && i + 1 < n && src[i + 1] == '/') {
      while (i < n && src[i] != '\n') {
        i++;
      }
      continue;
    }
    if (c == '/' && i + 1 < n && src[i + 1] == '*') {
      i += 2;
      while (i + 1 < n && !(src[i] == '*' && src[i + 1] == '/')) {
        i++;
      }
      i += 2;
      continue;
    }
    out.write(c);
    i++;
  }
  return out.toString();
}

void main() {
  // Self-check: the stripper drops comment CJK, keeps string CJK, isn't fooled
  // by markers inside strings. If this breaks, the scan below is meaningless.
  test('stripComments keeps string CJK, drops comment CJK', () {
    expect(_cjk.hasMatch(stripComments('// 注释')), isFalse);
    expect(_cjk.hasMatch(stripComments('/// 文档注释')), isFalse);
    expect(_cjk.hasMatch(stripComments('/* 块\n注释 */')), isFalse);
    expect(_cjk.hasMatch(stripComments("Text('中文')")), isTrue);
    // A `//` inside a string must NOT truncate it (else we'd miss the CJK).
    expect(_cjk.hasMatch(stripComments("Uri('a//b 路径')")), isTrue);
    // Code followed by a comment: string CJK survives, comment CJK gone.
    expect(stripComments("x('英') // 中").contains('英'), isTrue);
    expect(stripComments("x('en') // 中").contains('中'), isFalse);
  });

  test('no hardcoded CJK string literals in lib/presentation', () {
    final dir = Directory('lib/presentation');
    expect(dir.existsSync(), isTrue, reason: 'run from apps/mobile');
    final offenders = <String>[];
    for (final f in dir.listSync().whereType<File>()) {
      if (!f.path.endsWith('.dart')) continue;
      final stripped = stripComments(f.readAsStringSync());
      if (_cjk.hasMatch(stripped)) {
        final hits = _cjk
            .allMatches(stripped)
            .map((m) => m.group(0))
            .toSet()
            .join();
        offenders.add('${f.path}: $hits');
      }
    }
    expect(offenders, isEmpty,
        reason: 'hardcoded CJK found — move it into lib/l10n/*.arb:\n'
            '${offenders.join('\n')}');
  });
}
