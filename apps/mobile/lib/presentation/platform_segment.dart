import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';

/// Platform-adaptive segmented control, generic over the value type so the two
/// call sites (generate-mode `GeneratorKind`, crop orientation `bool`) share one
/// code path. iOS renders a `CupertinoSlidingSegmentedControl`; every other
/// platform keeps Material's `SegmentedButton` (unchanged look). Platform is
/// read from `Theme.of(context).platform` so widget tests can override it — never
/// `dart:io Platform` (that's the host OS and isn't test-controllable).
///
/// Values flow identically on both branches: [onChanged] fires with the picked
/// value. Cupertino's `onValueChanged` is nullable (T?), so it falls back to the
/// current [value] to keep the write well-defined.
Widget platformSegment<T extends Object>({
  required BuildContext context,
  required T value,
  required List<(T, String)> options,
  required ValueChanged<T> onChanged,
}) {
  final scheme = Theme.of(context).colorScheme;
  if (Theme.of(context).platform == TargetPlatform.iOS) {
    return CupertinoSlidingSegmentedControl<T>(
      groupValue: value,
      // pegboard tokens: brand-tinted track, card-colored sliding thumb, ink
      // labels — legible in both light and dark, no hardcoded colors.
      backgroundColor: scheme.primary.withValues(alpha: 0.12),
      thumbColor: scheme.surfaceContainerHighest,
      children: {
        for (final (v, label) in options)
          v: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
            child: Text(label, style: TextStyle(color: scheme.onSurface)),
          ),
      },
      onValueChanged: (v) => onChanged(v ?? value),
    );
  }
  return SegmentedButton<T>(
    segments: [
      for (final (v, label) in options)
        ButtonSegment(value: v, label: Text(label)),
    ],
    selected: {value},
    onSelectionChanged: (s) => onChanged(s.first),
  );
}
