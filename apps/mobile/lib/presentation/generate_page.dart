import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show FilteringTextInputFormatter;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../application/providers.dart';
import '../infrastructure/bead_bridge.dart' show GeneratorKind;
import 'platform_segment.dart';
import 'session_providers.dart';

/// Fit a width×height pair to `aspect` (= width/height) with both sides in
/// 1..1000, anchored to the user's edited `value`. If honoring `value` would
/// push the paired side past 1000, the WHOLE pair scales down uniformly so the
/// locked ratio is preserved — never clamp one side while the other stays (that
/// silently breaks the lock and lets the engine re-crop, the thing the lock
/// prevents). Pure + host-testable.
(int, int) lockedGridPair(int value, double aspect, {required bool valueIsWidth}) {
  var w = (valueIsWidth ? value : value * aspect).toDouble();
  var h = (valueIsWidth ? value / aspect : value).toDouble();
  final longest = w > h ? w : h;
  if (longest > 1000) {
    w = w * 1000 / longest;
    h = h * 1000 / longest;
  }
  return (w.round().clamp(1, 1000), h.round().clamp(1, 1000));
}

/// Step 3: user sets width × height (preset or numeric) → generate via the
/// GeneratePattern use case → ResultPage. A bridge failure shows its flattened
/// message instead of crashing.
class GeneratePage extends ConsumerStatefulWidget {
  const GeneratePage({super.key});

  @override
  ConsumerState<GeneratePage> createState() => _GeneratePageState();
}

class _GeneratePageState extends ConsumerState<GeneratePage> {
  final _width = TextEditingController(text: '50');
  final _height = TextEditingController(text: '50');
  bool _busy = false;
  String? _error;

  // Engine options as local state (like _width/_height) — settings and generate
  // live in the same widget, so no session provider (design D3). Defaults are
  // field-identical to the old hardcoded path: null / null / staged.
  GeneratorKind _generator = GeneratorKind.staged;
  bool _limitColors = false; // off ⇒ maxColors null (no limit)
  final _maxColors = TextEditingController(text: '24');
  bool _despeckleOn = false; // off ⇒ despeckle null
  final _despeckle = TextEditingController(text: '2');

  // Largest u32 — number inputs are constrained to non-negative and ≤ this so
  // FRB's putUint32 can encode them; business validity (≤N beads, etc.) is the
  // engine's call, not the shell's.
  static const _u32Max = 4294967295;

  static final _digitsOnly = FilteringTextInputFormatter.digitsOnly;

  // Empty ⇒ null; otherwise clamp into representable u32 range.
  static int? _readU32OrNull(String text) {
    final t = text.trim();
    if (t.isEmpty) return null;
    final v = int.tryParse(t);
    if (v == null) return null;
    return v.clamp(0, _u32Max);
  }

  // aspect = width / height, locked to the crop frame's ratio (default square).
  late double _aspect;

  // Base long sides; the paired side is derived from _aspect so no preset can
  // violate the lock (e.g. no 80×100 under square).
  static const _presetBases = [50, 80, 100];

  static int _clampSide(int v) => v.clamp(1, 1000);

  @override
  void initState() {
    super.initState();
    _aspect = ref.read(cropAspectProvider);
    // Seed height from the 50-bead default width so the initial pair obeys the
    // lock even for a non-square crop.
    _height.text = '${_clampSide((50 / _aspect).round())}';
  }

  @override
  void dispose() {
    _width.dispose();
    _height.dispose();
    _maxColors.dispose();
    _despeckle.dispose();
    super.dispose();
  }

  List<(int, int)> get _presets {
    final seen = <(int, int)>{};
    for (final w in _presetBases) {
      seen.add((w, _clampSide((w / _aspect).round())));
    }
    return seen.toList();
  }

  void _applyPreset(int w, int h) {
    _width.text = '$w';
    _height.text = '$h';
  }

  // Tap −/+ to step a side by 1 (clamped 1..1000); re-derives the paired side
  // through the same aspect-lock as typing.
  void _step(TextEditingController c, int delta, void Function(String) onChanged) {
    final next = _clampSide((int.tryParse(c.text.trim()) ?? 0) + delta);
    c.text = '$next';
    onChanged('$next');
  }

  Widget _sizeStepper(
    TextEditingController c,
    String label,
    void Function(String) onChanged,
  ) {
    return Row(
      children: [
        IconButton.outlined(
          onPressed: () => _step(c, -1, onChanged),
          icon: const Icon(Icons.remove),
        ),
        Expanded(
          child: TextField(
            controller: c,
            keyboardType: TextInputType.number,
            textAlign: TextAlign.center,
            onChanged: onChanged,
            decoration: InputDecoration(labelText: label),
          ),
        ),
        IconButton.outlined(
          onPressed: () => _step(c, 1, onChanged),
          icon: const Icon(Icons.add),
        ),
      ],
    );
  }

  // Editing one side auto-derives the other from _aspect, rebasing BOTH to stay
  // in-bounds when the input is infeasible (so the locked ratio never breaks).
  // Programmatic controller writes here do NOT retrigger onChanged, so no loop
  // guard needed.
  void _onWidthChanged(String v) {
    final w = int.tryParse(v.trim());
    if (w == null) return;
    final (wi, hi) = lockedGridPair(w, _aspect, valueIsWidth: true);
    _height.text = '$hi';
    if (wi != w) _width.text = '$wi'; // infeasible at this aspect+cap → rebased
  }

  void _onHeightChanged(String v) {
    final h = int.tryParse(v.trim());
    if (h == null) return;
    final (wi, hi) = lockedGridPair(h, _aspect, valueIsWidth: false);
    _width.text = '$wi';
    if (hi != h) _height.text = '$hi'; // infeasible at this aspect+cap → rebased
  }

  Future<void> _generate() async {
    final cropped = ref.read(croppedImageProvider);
    if (cropped == null) {
      setState(() => _error = '没有裁剪后的图片，请返回重新选图');
      return;
    }
    final width = int.tryParse(_width.text.trim());
    final height = int.tryParse(_height.text.trim());
    if (width == null || height == null) {
      setState(() => _error = '请输入有效的宽和高');
      return;
    }
    // ponytail: cap at 1000 beads/side — bead-core has no upper bound, so a huge
    // value eager-allocs w·h·3 bytes in image::resize → uncatchable alloc abort.
    if (width < 1 || width > 1000 || height < 1 || height > 1000) {
      setState(() => _error = '宽和高需在 1–1000 之间');
      return;
    }
    // A toggled-on option with an empty/invalid field would silently send null
    // (= off) with no feedback — reject it. `0` is valid (reaches the engine).
    if (_limitColors && _readU32OrNull(_maxColors.text) == null) {
      setState(() => _error = '开了「限制颜色数」请填一个有效数值');
      return;
    }
    if (_despeckleOn && _readU32OrNull(_despeckle.text) == null) {
      setState(() => _error = '开了「去斑」请填一个有效阈值');
      return;
    }
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final paletteJson = await ref.read(paletteJsonProvider.future);
      final output = await ref.read(generatePatternProvider).call(
            imageBytes: cropped,
            paletteJson: paletteJson,
            width: width,
            height: height,
            maxColors: _limitColors ? _readU32OrNull(_maxColors.text) : null,
            despeckle: _despeckleOn ? _readU32OrNull(_despeckle.text) : null,
            generator: _generator,
          );
      if (!mounted) return;
      ref.read(generateResultProvider.notifier).set(output);
      context.push('/result');
    } catch (e) {
      // Flattened bridge exception message — show it, never crash (spec).
      if (mounted) setState(() => _error = e.toString());
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  // Engine-option controls (task 3.1). Styled off theme roles only (4.3) — no
  // hardcoded colors.
  List<Widget> _optionControls(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final labelStyle = Theme.of(context)
        .textTheme
        .titleSmall
        ?.copyWith(color: scheme.onSurface);
    return [
      Text('生成模式', style: labelStyle),
      const SizedBox(height: 8),
      platformSegment<GeneratorKind>(
        context: context,
        value: _generator,
        options: const [
          (GeneratorKind.staged, '常规'),
          (GeneratorKind.gerstner, '照片'),
        ],
        onChanged: (v) => setState(() => _generator = v),
      ),
      const SizedBox(height: 16),
      SwitchListTile.adaptive(
        contentPadding: EdgeInsets.zero,
        title: const Text('限制颜色数'),
        activeTrackColor: scheme.primary, // brand pill on iOS; M3 default on Android
        value: _limitColors,
        onChanged: (v) => setState(() => _limitColors = v),
      ),
      if (_limitColors)
        TextField(
          controller: _maxColors,
          keyboardType: TextInputType.number,
          inputFormatters: [_digitsOnly],
          decoration: const InputDecoration(labelText: '最大颜色数'),
        ),
      const SizedBox(height: 16),
      SwitchListTile.adaptive(
        contentPadding: EdgeInsets.zero,
        title: const Text('去斑'),
        subtitle: const Text('清除孤立的杂色点'),
        activeTrackColor: scheme.primary, // brand pill on iOS; M3 default on Android
        value: _despeckleOn,
        onChanged: (v) => setState(() => _despeckleOn = v),
      ),
      if (_despeckleOn)
        TextField(
          controller: _despeckle,
          keyboardType: TextInputType.number,
          inputFormatters: [_digitsOnly],
          decoration: const InputDecoration(
            labelText: '阈值（豆）',
            helperText: '把不超过这么多豆的孤立同色小块，并入相邻主色',
          ),
        ),
    ];
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('设置')),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            _sizeStepper(_width, '宽 (豆)', _onWidthChanged),
            const SizedBox(height: 12),
            _sizeStepper(_height, '高 (豆)', _onHeightChanged),
            const SizedBox(height: 12),
            Wrap(
              spacing: 8,
              children: [
                for (final (w, h) in _presets)
                  ActionChip(
                    label: Text('$w×$h'),
                    onPressed: () => _applyPreset(w, h),
                  ),
              ],
            ),
            const SizedBox(height: 24),
            ..._optionControls(context),
            const SizedBox(height: 24),
            if (_error != null)
              Padding(
                padding: const EdgeInsets.only(bottom: 12),
                child: Text(
                  _error!,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
              ),
            FilledButton(
              onPressed: _busy ? null : _generate,
              child: _busy
                  ? const SizedBox(
                      height: 20,
                      width: 20,
                      child: CircularProgressIndicator.adaptive(strokeWidth: 2),
                    )
                  : const Text('生成'),
            ),
          ],
        ),
      ),
    );
  }
}
