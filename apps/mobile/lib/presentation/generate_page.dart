import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../application/providers.dart';
import 'session_providers.dart';

/// Step 3: user sets width × height (preset or numeric) → generate via the
/// GeneratePattern use case → ResultPage. A bridge failure shows its flattened
/// message instead of crashing.
class GeneratePage extends ConsumerStatefulWidget {
  const GeneratePage({super.key});

  @override
  ConsumerState<GeneratePage> createState() => _GeneratePageState();
}

class _GeneratePageState extends ConsumerState<GeneratePage> {
  final _width = TextEditingController(text: '40');
  final _height = TextEditingController(text: '40');
  bool _busy = false;
  String? _error;

  // aspect = width / height, locked to the crop frame's ratio (default square).
  late double _aspect;

  // Base long sides; the paired side is derived from _aspect so no preset can
  // violate the lock (e.g. no 80×100 under square).
  static const _presetBases = [40, 58, 80];

  static int _clampSide(int v) => v.clamp(1, 1000);

  @override
  void initState() {
    super.initState();
    _aspect = ref.read(cropAspectProvider);
    // Seed height from the 40-bead default width so the initial pair obeys the
    // lock even for a non-square crop.
    _height.text = '${_clampSide((40 / _aspect).round())}';
  }

  @override
  void dispose() {
    _width.dispose();
    _height.dispose();
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

  // Editing one side auto-derives the other from _aspect. Programmatic
  // controller writes here do NOT retrigger onChanged, so no loop guard needed.
  void _onWidthChanged(String v) {
    final w = int.tryParse(v.trim());
    if (w != null) _height.text = '${_clampSide((w / _aspect).round())}';
  }

  void _onHeightChanged(String v) {
    final h = int.tryParse(v.trim());
    if (h != null) _width.text = '${_clampSide((h * _aspect).round())}';
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

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('设置尺寸')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                Expanded(
                  child: TextField(
                    controller: _width,
                    keyboardType: TextInputType.number,
                    onChanged: _onWidthChanged,
                    decoration: const InputDecoration(labelText: '宽 (豆)'),
                  ),
                ),
                const SizedBox(width: 16),
                Expanded(
                  child: TextField(
                    controller: _height,
                    keyboardType: TextInputType.number,
                    onChanged: _onHeightChanged,
                    decoration: const InputDecoration(labelText: '高 (豆)'),
                  ),
                ),
              ],
            ),
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
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Text('生成'),
            ),
          ],
        ),
      ),
    );
  }
}
