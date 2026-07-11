import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show FilteringTextInputFormatter;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../application/generate_settings.dart';
import '../application/providers.dart';
import '../infrastructure/bead_bridge.dart' show GeneratorKind;
import '../infrastructure/palette_registry.dart';
import '../l10n/app_localizations.dart';
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
  final _width = TextEditingController();
  final _height = TextEditingController();
  bool _busy = false;
  String? _error;

  // Engine options + width now live in the cross-launch persisted settings model
  // (design D4): the segment/toggles read `generateSettingsProvider` in build and
  // write via its setters; these controllers hold the numeric field values and
  // are seeded from persisted settings in initState. The despeckle/maxColors text
  // is still read from the controller at generate time so the "toggled-on but
  // empty ⇒ reject" guard can see an empty field (persisted value is never null).
  final _maxColors = TextEditingController();
  final _despeckle = TextEditingController();

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
    final s = ref.read(generateSettingsProvider);
    _maxColors.text = '${s.maxColors}';
    _despeckle.text = '${s.despeckle}';
    // Seed width from the persisted value, then re-derive height for THIS crop's
    // aspect via the lock (design D5). If the persisted width overflows under a
    // narrower aspect, lockedGridPair scales the whole pair down — so the seeded
    // width may not equal the persisted value. This entry re-derive/rebase must
    // NOT write back (no setWidth): it's not a user edit of the width preference.
    final (wi, hi) = lockedGridPair(s.width, _aspect, valueIsWidth: true);
    _width.text = '$wi';
    _height.text = '$hi';
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
    // A preset tap is a deliberate width choice → persist it (design D5). Height
    // stays derived/transient. (Presets bake in ≤1000 widths, so no rebase.)
    ref.read(generateSettingsProvider.notifier).setWidth(w);
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
    // User explicitly edited the width field → persist the LANDED legal width
    // (post aspect-lock / overflow rebase), per design D5's write-back rule.
    ref.read(generateSettingsProvider.notifier).setWidth(wi);
  }

  void _onHeightChanged(String v) {
    final h = int.tryParse(v.trim());
    if (h == null) return;
    final (wi, hi) = lockedGridPair(h, _aspect, valueIsWidth: false);
    _width.text = '$wi';
    if (hi != h) _height.text = '$hi'; // infeasible at this aspect+cap → rebased
  }

  Future<void> _generate() async {
    final l10n = AppLocalizations.of(context);
    final cropped = ref.read(croppedImageProvider);
    if (cropped == null) {
      setState(() => _error = l10n.generateNoCroppedImage);
      return;
    }
    final width = int.tryParse(_width.text.trim());
    final height = int.tryParse(_height.text.trim());
    if (width == null || height == null) {
      setState(() => _error = l10n.generateInvalidSize);
      return;
    }
    // ponytail: cap at 1000 beads/side — bead-core has no upper bound, so a huge
    // value eager-allocs w·h·3 bytes in image::resize → uncatchable alloc abort.
    if (width < 1 || width > 1000 || height < 1 || height > 1000) {
      setState(() => _error = l10n.generateSizeRange);
      return;
    }
    final settings = ref.read(generateSettingsProvider);
    // A toggled-on option with an empty/invalid field would silently send null
    // (= off) with no feedback — reject it. `0` is valid (reaches the engine).
    if (settings.limitColors && _readU32OrNull(_maxColors.text) == null) {
      setState(() => _error = l10n.generateMaxColorsRequired);
      return;
    }
    if (settings.despeckleOn && _readU32OrNull(_despeckle.text) == null) {
      setState(() => _error = l10n.generateDespeckleRequired);
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
            maxColors:
                settings.limitColors ? _readU32OrNull(_maxColors.text) : null,
            despeckle:
                settings.despeckleOn ? _readU32OrNull(_despeckle.text) : null,
            generator: settings.generator,
          );
      if (!mounted) return;
      // Pin the exact palette JSON we passed to generate (design D6) — do NOT
      // re-read the provider, or a later palette switch would drift the result.
      ref
          .read(generateResultProvider.notifier)
          .set(GenerateResult(output: output, paletteJson: paletteJson));
      context.push('/result');
    } catch (e) {
      // Flattened bridge exception message — show it, never crash (spec).
      if (mounted) setState(() => _error = e.toString());
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  // Engine-option controls. Read from the persisted settings model and write via
  // its setters (design D4); styled off theme roles only — no hardcoded colors.
  List<Widget> _optionControls(BuildContext context, GenerateSettings settings) {
    final scheme = Theme.of(context).colorScheme;
    final labelStyle = Theme.of(context)
        .textTheme
        .titleSmall
        ?.copyWith(color: scheme.onSurface);
    final notifier = ref.read(generateSettingsProvider.notifier);
    final l10n = AppLocalizations.of(context);
    return [
      Text(l10n.generateModeLabel, style: labelStyle),
      const SizedBox(height: 8),
      platformSegment<GeneratorKind>(
        context: context,
        value: settings.generator,
        options: [
          (GeneratorKind.staged, l10n.generateModeStaged),
          (GeneratorKind.gerstner, l10n.generateModeGerstner),
        ],
        onChanged: notifier.setGenerator,
      ),
      const SizedBox(height: 16),
      SwitchListTile.adaptive(
        contentPadding: EdgeInsets.zero,
        title: Text(l10n.generateLimitColors),
        activeTrackColor: scheme.primary, // brand pill on iOS; M3 default on Android
        value: settings.limitColors,
        onChanged: notifier.setLimitColors,
      ),
      if (settings.limitColors)
        TextField(
          controller: _maxColors,
          keyboardType: TextInputType.number,
          inputFormatters: [_digitsOnly],
          decoration: InputDecoration(labelText: l10n.generateMaxColors),
          // Persist a valid value; an empty field is transient (persisted value
          // is non-null) and left alone so the "toggled-on + empty ⇒ reject"
          // guard still works at generate time.
          onChanged: (v) {
            final n = _readU32OrNull(v);
            if (n != null) notifier.setMaxColors(n);
          },
        ),
      const SizedBox(height: 16),
      SwitchListTile.adaptive(
        contentPadding: EdgeInsets.zero,
        title: Text(l10n.generateDespeckle),
        subtitle: Text(l10n.generateDespeckleSubtitle),
        activeTrackColor: scheme.primary, // brand pill on iOS; M3 default on Android
        value: settings.despeckleOn,
        onChanged: notifier.setDespeckleOn,
      ),
      if (settings.despeckleOn)
        TextField(
          controller: _despeckle,
          keyboardType: TextInputType.number,
          inputFormatters: [_digitsOnly],
          decoration: InputDecoration(
            labelText: l10n.generateThresholdLabel,
            helperText: l10n.generateThresholdHelper,
          ),
          onChanged: (v) {
            final n = _readU32OrNull(v);
            if (n != null) notifier.setDespeckle(n);
          },
        ),
    ];
  }

  /// Palette row (task 4.2): the current brand from the registry (synchronous),
  /// tap opens a Material bottom sheet listing every built-in palette.
  Widget _paletteRow(BuildContext context, GenerateSettings settings) {
    return ListTile(
      contentPadding: EdgeInsets.zero,
      title: Text(AppLocalizations.of(context).generatePalette),
      subtitle: Text(paletteEntryOrDefault(settings.paletteId).brand),
      trailing: const Icon(Icons.chevron_right),
      onTap: _showPaletteSheet,
    );
  }

  // Plain Material bottom sheet (design D2): Flutter has no adaptive bottom-sheet
  // constructor, and crop/result sheets already use this — two-end consistent.
  Future<void> _showPaletteSheet() {
    return showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      builder: (sheetCtx) => SafeArea(
        child: Consumer(
          builder: (ctx, ref, _) {
            final selectedId =
                ref.watch(generateSettingsProvider.select((s) => s.paletteId));
            final counts = ref.watch(paletteColorCountsProvider);
            return ListView(
              shrinkWrap: true,
              children: [
                for (final e in paletteRegistry)
                  ListTile(
                    title: Text(e.brand),
                    // "N 色" lazily parsed; parse failure / not-yet-loaded → "—".
                    subtitle: Text(AppLocalizations.of(ctx)
                        .paletteColorCount('${counts.asData?.value[e.id] ?? '—'}')),
                    trailing: e.id == selectedId
                        ? Icon(Icons.check,
                            color: Theme.of(ctx).colorScheme.primary)
                        : null,
                    onTap: () {
                      ref
                          .read(generateSettingsProvider.notifier)
                          .setPaletteId(e.id);
                      Navigator.pop(ctx);
                    },
                  ),
              ],
            );
          },
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final settings = ref.watch(generateSettingsProvider);
    final l10n = AppLocalizations.of(context);
    return Scaffold(
      appBar: AppBar(title: Text(l10n.generateTitle)),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            _sizeStepper(_width, l10n.generateWidth, _onWidthChanged),
            const SizedBox(height: 12),
            _sizeStepper(_height, l10n.generateHeight, _onHeightChanged),
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
            _paletteRow(context, settings),
            const SizedBox(height: 8),
            ..._optionControls(context, settings),
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
                  : Text(l10n.generateSubmit),
            ),
          ],
        ),
      ),
    );
  }
}
