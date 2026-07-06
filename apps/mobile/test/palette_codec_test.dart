import 'dart:ui' show Color;

import 'package:beadsmith/infrastructure/palette_codec.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('parsePalette', () {
    test('preserves JSON colors order and parses #RRGGBB hex', () {
      const json = '{"brand":"Test","colors":['
          '{"code":"S01","name":"White","rgb":"#EAEEF3"},'
          '{"code":"S02","name":"Black","rgb":"#1C1830"},'
          '{"code":"S05","name":"Violet","rgb":"#6C4BF4"}'
          ']}';
      final palette = parsePalette(json);
      expect(palette.length, 3);
      expect(palette[0].code, 'S01');
      expect(palette[0].name, 'White');
      expect(palette[0].rgb, const Color(0xFFEAEEF3));
      expect(palette[1].code, 'S02');
      expect(palette[1].rgb, const Color(0xFF1C1830));
      expect(palette[2].rgb, const Color(0xFF6C4BF4));
    });

    test('expands #RGB shorthand to #RRGGBB', () {
      const json = '{"brand":"T","colors":['
          '{"code":"R","name":"Red","rgb":"#F00"},'
          '{"code":"G","name":"Green","rgb":"#0F0"},'
          '{"code":"B","name":"Blue","rgb":"#00F"}'
          ']}';
      final palette = parsePalette(json);
      expect(palette[0].rgb, const Color(0xFFFF0000));
      expect(palette[1].rgb, const Color(0xFF00FF00));
      expect(palette[2].rgb, const Color(0xFF0000FF));
    });

    test('list index == the index BeadPattern.cells points at', () {
      // The single load-bearing invariant for the grid view: cells[i] indexes
      // this list in the same order the engine's load_palette preserves.
      const json = '{"brand":"T","colors":['
          '{"code":"A","name":"a","rgb":"#111111"},'
          '{"code":"B","name":"b","rgb":"#222222"},'
          '{"code":"C","name":"c","rgb":"#333333"}'
          ']}';
      final palette = parsePalette(json);
      // If the engine emits cells = [0, 2, 1, 0], the UI must read:
      expect(palette[0].code, 'A');
      expect(palette[2].code, 'C');
      expect(palette[1].code, 'B');
    });
  });
}
