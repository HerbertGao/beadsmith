import 'dart:math' as math;
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../application/generate_settings.dart'
    show generateSettingsProvider, kMaxBorderRings;
import '../application/providers.dart';
import '../infrastructure/album_service.dart' show AlbumAccessDenied;
import '../infrastructure/bead_bridge.dart' show ColorStat, GenerateOutput;
import '../infrastructure/palette_codec.dart' show PaletteColor, parsePalette;
import '../l10n/app_localizations.dart';
import 'bead_grid_layout.dart';
import 'bead_grid_save.dart' show renderPatternPng;
import 'bead_grid_view.dart';
import 'session_providers.dart';
import 'theme.dart';

/// Transient border-ring count `k` for the CURRENT result. A `.family` keyed by
/// the [GenerateResult] identity: each generation is a fresh instance (it has no
/// `==` override), so a new result re-seeds `k` from the persisted default in
/// [_ResultBorderRings.build] instead of inheriting the k tweaked on the previous
/// result — and `autoDispose` reclaims it when the ResultPage pops. The seed
/// lives in `build`, so the sibling AppBar and body read the SAME seeded value on
/// first read (a plain provider seeded in initState would race). NOT written back
/// to the persisted default.
///
/// (Riverpod 3 makes `StateProvider` legacy; this codebase is all-`Notifier`, so
/// the equivalent family Notifier is used — same autoDispose/family/seed
/// semantics the design mandates, without a legacy import.)
final resultBorderRingsProvider = NotifierProvider.autoDispose
    .family<_ResultBorderRings, int, GenerateResult>(
  (result) => _ResultBorderRings(),
);

class _ResultBorderRings extends Notifier<int> {
  @override
  int build() =>
      // One-time seed from the default (ref.read, NOT watch): k here is a
      // transient per-result display state, so a later change to the default
      // must not reset the user's in-result adjustment. Re-seeding for a NEW
      // result is handled by the `.family` key (a fresh GenerateResult → a
      // fresh Notifier → build() runs again), not by a reactive dependency.
      ref.read(generateSettingsProvider).borderRings;

  void set(int k) => state = k;
}

/// Step 4 (redesign, qiaomu-design B): the grid is the primary view — a
/// full-screen zoomable bead grid the user can pan/pinch/tap to identify
/// each cell's bead. The preview image drops to an AppBar thumbnail (tap to
/// enlarge); the color legend collapses to a bottom bar (tap to expand). The
/// "汇总" text block is gone — copy lives in the AppBar.
///
/// All displayed data still comes verbatim from `GenerateOutput` (preview PNG,
/// `pattern.cells`, `stats`, `summary`) + the parsed palette — never
/// recomputed from the rendered image (hard rule 3).
class ResultPage extends ConsumerWidget {
  const ResultPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final result = ref.watch(generateResultProvider);
    if (result == null) {
      final l10n = AppLocalizations.of(context);
      return Scaffold(
        appBar: AppBar(title: Text(l10n.resultTitle)),
        body: Center(child: Text(l10n.resultNoResult)),
      );
    }
    // Parse the palette PINNED at generation time (design D6), synchronously —
    // the JSON is already in hand, so there's no loading/error state. Reading the
    // live `paletteJsonProvider` here would recolor the result if the user
    // switched palettes after generating.
    final palette = parsePalette(result.paletteJson);
    return Scaffold(
      appBar: _ResultAppBar(result: result, palette: palette),
      body: _ResultBody(result: result, palette: palette),
    );
  }
}

class _ResultAppBar extends ConsumerWidget implements PreferredSizeWidget {
  const _ResultAppBar({required this.result, required this.palette});
  final GenerateResult result;
  final List<PaletteColor> palette;

  @override
  Size get preferredSize => const Size.fromHeight(kToolbarHeight);

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final output = result.output;
    final k = ref.watch(resultBorderRingsProvider(result));
    return AppBar(
      title: Row(
        children: [
          // 48×48 hit target (a11y minimum) around a 30×30 visual thumbnail.
          InkWell(
            onTap: () => _showPreview(context, output.previewPng),
            borderRadius: BorderRadius.circular(6),
            child: SizedBox(
              width: 48,
              height: 48,
              child: Center(
                child: ClipRRect(
                  borderRadius: BorderRadius.circular(6),
                  child: Image.memory(
                    output.previewPng,
                    width: 30,
                    height: 30,
                    fit: BoxFit.cover,
                    gaplessPlayback: true,
                  ),
                ),
              ),
            ),
          ),
          Text(l10n.resultTitle),
        ],
      ),
      actions: [
        IconButton(
          icon: const Icon(Icons.save_alt),
          tooltip: l10n.resultSaveToAlbum,
          onPressed: () => _saveGridToAlbum(context, ref, output, palette, k),
        ),
        IconButton(
          icon: const Icon(Icons.copy),
          tooltip: l10n.resultCopySummary,
          onPressed: () => _copySummary(context, ref, output.summary),
        ),
      ],
    );
  }
}

class _ResultBody extends ConsumerStatefulWidget {
  const _ResultBody({required this.result, required this.palette});
  final GenerateResult result;
  final List<PaletteColor> palette;

  @override
  ConsumerState<_ResultBody> createState() => _ResultBodyState();
}

class _ResultBodyState extends ConsumerState<_ResultBody> {
  /// Palette index of the currently highlighted color (null = none). Set by
  /// the "高亮同色" action in the cell-detail or legend sheet; the grid
  /// strokes a ring on every cell sharing this index.
  int? _highlightedIndex;

  /// Top floating "建议保存到相册" tip — shown once per app session,
  /// auto-dismisses after 7s (5–10s band), dismissible via the × button.
  /// Not a bottom SnackBar: the user asked for a transient top tip, not a
  /// persistent bar. Session-level flag so it doesn't nag on every visit.
  bool _showSaveTip = false;
  static bool _saveHintShown = false;

  /// Whether the 配色 legend is expanded. Collapsed = thin bottom bar (grid
  /// centered with top/bottom whitespace). Expanded = the legend grows up to
  /// fill the grid's whitespace, and the grid area (an Expanded above it)
  /// shrinks to exactly the grid's height, so the grid rises to sit flush
  /// against the legend's top edge — no whitespace left.
  bool _legendExpanded = false;

  @override
  void initState() {
    super.initState();
    if (!_saveHintShown) {
      _saveHintShown = true;
      _showSaveTip = true;
      Future.delayed(const Duration(seconds: 7), () {
        if (mounted) setState(() => _showSaveTip = false);
      });
    }
  }

  void _onCellTap(int row, int col, int paletteIndex) {
    _showCellDetail(row, col, paletteIndex);
  }

  void _toggleHighlight(int paletteIndex) {
    setState(
      () => _highlightedIndex = _highlightedIndex == paletteIndex
          ? null
          : paletteIndex,
    );
  }

  void _dismissSaveTip() => setState(() => _showSaveTip = false);

  void _showCellDetail(int row, int col, int paletteIndex) {
    // paletteIndex comes from a Uint16List cell, so it's always >= 0; the full
    // bound is belt-and-suspenders for the unreachable out-of-range case.
    assert(paletteIndex >= 0 && paletteIndex < widget.palette.length);
    if (paletteIndex < 0 || paletteIndex >= widget.palette.length) return;
    final color = widget.palette[paletteIndex];
    final count = _countFor(paletteIndex);
    final isHighlighted = _highlightedIndex == paletteIndex;
    showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      builder: (sheetCtx) => SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Container(
                    width: 44,
                    height: 44,
                    decoration: BoxDecoration(
                      color: color.rgb,
                      borderRadius: BorderRadius.circular(10),
                      border: Border.all(
                        color: Theme.of(sheetCtx).colorScheme.outline,
                      ),
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          color.code,
                          style: monoTextStyle.copyWith(
                            color: Theme.of(sheetCtx).colorScheme.primary,
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                        Text(
                          color.name,
                          style: Theme.of(sheetCtx).textTheme.bodyMedium,
                        ),
                      ],
                    ),
                  ),
                  Text(
                    AppLocalizations.of(sheetCtx).cellDetailCount(count),
                    style: monoTextStyle.copyWith(
                      color: Theme.of(sheetCtx).colorScheme.onSurface,
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              Text(
                AppLocalizations.of(sheetCtx)
                    .cellDetailPosition(row + 1, col + 1),
                style: Theme.of(sheetCtx).textTheme.bodySmall,
              ),
              const SizedBox(height: 14),
              FilledButton.tonalIcon(
                onPressed: () {
                  _toggleHighlight(paletteIndex);
                  Navigator.pop(sheetCtx);
                },
                icon: Icon(
                  isHighlighted ? Icons.highlight_off : Icons.highlight,
                ),
                label: Text(isHighlighted
                    ? AppLocalizations.of(sheetCtx).resultCancelHighlight
                    : AppLocalizations.of(sheetCtx).resultHighlightSame),
              ),
            ],
          ),
        ),
      ),
    );
  }

  /// Count of beads for [paletteIndex] from `output.stats` (matched by code).
  int _countFor(int paletteIndex) {
    if (paletteIndex >= widget.palette.length) return 0;
    final code = widget.palette[paletteIndex].code;
    for (final s in widget.result.output.stats) {
      if (s.code == code) return s.count;
    }
    return 0;
  }

  @override
  Widget build(BuildContext context) {
    // No outer SafeArea: the 配色 panel's surface must extend to the physical
    // bottom edge (under the home indicator) so there's no dead strip below
    // it. The bottom inset is instead applied INSIDE the panel's content
    // padding (see _LegendSheet.bottomInset), so the surface is continuous
    // but the last row clears the home indicator. The grid area is
    // letterboxed with its own whitespace, so it needs no bottom inset.
    final bottomInset = MediaQuery.paddingOf(context).bottom;
    final k = ref.watch(resultBorderRingsProvider(widget.result));
    final output = widget.result.output;
    return LayoutBuilder(
      builder: (context, constraints) {
        final bodyH = constraints.maxHeight;
        final bodyAspect = constraints.maxWidth / bodyH;
        // Whitespace/letterbox must use the SAME canvas aspect the preview
        // letterboxes by: it includes the tick margin + the k border rings, so
        // it is NOT (W+2k)/(H+2k) (design D7 — one geometry source).
        final gridAspect = BeadGridLayout(
          width: output.pattern.width,
          height: output.pattern.height,
          borderRings: k,
          cellSize: 1,
        ).canvasAspect;
        // Grid height fraction under a contain fit (matches BeadGridView's
        // Center+AspectRatio behavior). Whitespace = the rest.
        final gridHF = gridAspect > bodyAspect
            ? bodyAspect /
                  gridAspect // width-constrained → letterboxed
            : 1.0; // height-constrained (already fills height)
        final whitespace = (1.0 - gridHF) * bodyH;
        // Collapsed bar = handle/title height + the bottom safe inset (so the
        // surface fills under the home indicator).
        final collapsedH = 60.0 + bottomInset;
        // Expanded legend takes the whole whitespace, so the grid area
        // (Expanded above) shrinks to exactly the grid's height → grid rises
        // flush against the legend, no top whitespace. Never taller than the
        // grid area itself (guard for very tall grids with little whitespace).
        final expandedFloor = bodyH * 0.35 + bottomInset;
        final expandedTarget = math.max(whitespace, expandedFloor);
        // FIX D folded in: use math.max for the clamp lower bound so a very short
        // body (bodyH < ~100, upper < collapsedH) can't invert the clamp range.
        final expandedH = expandedTarget
            .clamp(collapsedH, math.max(collapsedH, bodyH * 0.6 + bottomInset))
            .toDouble();
        final legendH = _legendExpanded ? expandedH : collapsedH;
        return Stack(
          children: [
            Column(
              children: [
                // Grid area: shrinks as the legend grows. The grid stays
                // centered inside it, so as the area shrinks toward the
                // grid's natural height, the top whitespace disappears.
                Expanded(
                  child: BeadGridView(
                    cells: output.pattern.cells,
                    width: output.pattern.width,
                    height: output.pattern.height,
                    palette: widget.palette,
                    borderRings: k,
                    highlightedIndex: _highlightedIndex,
                    onCellTap: _onCellTap,
                  ),
                ),
                AnimatedContainer(
                  duration: const Duration(milliseconds: 240),
                  curve: Curves.easeOutCubic,
                  height: legendH,
                  child: _LegendSheet(
                    expanded: _legendExpanded,
                    bottomInset: bottomInset,
                    stats: output.stats,
                    palette: widget.palette,
                    highlightedIndex: _highlightedIndex,
                    onToggle: () =>
                        setState(() => _legendExpanded = !_legendExpanded),
                    onHighlight: _toggleHighlight,
                  ),
                ),
              ],
            ),
            // Top floating save tip — auto-dismisses after 7s, dismissible.
            Positioned(
              top: 8,
              left: 12,
              right: 12,
              child: IgnorePointer(
                ignoring: !_showSaveTip,
                child: AnimatedOpacity(
                  opacity: _showSaveTip ? 1.0 : 0.0,
                  duration: const Duration(milliseconds: 220),
                  curve: Curves.easeOut,
                  child: _SaveTip(
                    onSave: () {
                      _saveGridToAlbum(
                          context, ref, output, widget.palette, k);
                      _dismissSaveTip();
                    },
                    onDismiss: _dismissSaveTip,
                  ),
                ),
              ),
            ),
            // Border-ring stepper for THIS preview only (design D3/D8). Floats
            // just above the legend and moves with it. Changing k updates the
            // shared provider → this body rebuilds and re-renders the preview
            // (and the save PNG) at the new k; it never calls _generate and is
            // never written back to the persisted default.
            Positioned(
              right: 12,
              bottom: legendH + 8,
              child: _BorderRingStepper(
                value: k,
                onChanged: (v) => ref
                    .read(resultBorderRingsProvider(widget.result).notifier)
                    .set(v),
              ),
            ),
          ],
        );
      },
    );
  }
}

/// A transient top-floating "建议保存到相册" tip pill. Not a SnackBar (those
/// sit at the bottom); this floats at the top, auto-dismisses, and has a
/// dismiss button. Shown once per session by [_ResultBodyState].
class _SaveTip extends StatelessWidget {
  const _SaveTip({required this.onSave, required this.onDismiss});
  final VoidCallback onSave;
  final VoidCallback onDismiss;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Material(
      elevation: 6,
      borderRadius: BorderRadius.circular(14),
      color: theme.colorScheme.primaryContainer,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
        child: Row(
          children: [
            Icon(
              Icons.save_alt,
              size: 18,
              color: theme.colorScheme.onPrimaryContainer,
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                AppLocalizations.of(context).resultSaveTip,
                style: theme.textTheme.bodyMedium?.copyWith(
                  color: theme.colorScheme.onPrimaryContainer,
                ),
              ),
            ),
            TextButton(
              onPressed: onSave,
              style: TextButton.styleFrom(
                foregroundColor: theme.colorScheme.primary,
                visualDensity: VisualDensity.compact,
                padding: const EdgeInsets.symmetric(horizontal: 8),
              ),
              child: Text(AppLocalizations.of(context).resultSave),
            ),
            IconButton(
              onPressed: onDismiss,
              icon: const Icon(Icons.close),
              iconSize: 16,
              visualDensity: VisualDensity.compact,
              constraints: const BoxConstraints(minWidth: 28, minHeight: 28),
              padding: EdgeInsets.zero,
              color: theme.colorScheme.onPrimaryContainer,
            ),
          ],
        ),
      ),
    );
  }
}

/// A compact "边框圈 k" stepper pinned over the result grid — sets the
/// **transient** border-ring count for this preview/save only (design D3/D8).
/// Gated to `0..kMaxBorderRings`; the hint rides along as a tooltip.
class _BorderRingStepper extends StatelessWidget {
  const _BorderRingStepper({required this.value, required this.onChanged});
  final int value;
  final ValueChanged<int> onChanged;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final l10n = AppLocalizations.of(context);
    return Tooltip(
      message: l10n.borderRingsResultHint,
      child: Material(
        elevation: 4,
        borderRadius: BorderRadius.circular(24),
        color: theme.colorScheme.surfaceContainerHighest,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 2),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(l10n.borderRingsLabel, style: theme.textTheme.labelLarge),
              const SizedBox(width: 4),
              IconButton(
                onPressed: value > 0 ? () => onChanged(value - 1) : null,
                icon: const Icon(Icons.remove),
                iconSize: 18,
                visualDensity: VisualDensity.compact,
                constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
              ),
              SizedBox(
                width: 16,
                child: Text('$value',
                    textAlign: TextAlign.center, style: monoTextStyle),
              ),
              IconButton(
                onPressed:
                    value < kMaxBorderRings ? () => onChanged(value + 1) : null,
                icon: const Icon(Icons.add),
                iconSize: 18,
                visualDensity: VisualDensity.compact,
                constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// The 配色 legend. Collapsed = a thin tappable header bar (arrow + title +
/// mini swatches). Expanded = the same header (arrow flips) over a scrollable
/// full color list. Tapping the header toggles [expanded] via [onToggle]; the
/// parent animates this widget's height between the two states, and the grid
/// area above shrinks/grows to match (so the grid rises flush when expanded).
class _LegendSheet extends StatelessWidget {
  const _LegendSheet({
    required this.expanded,
    required this.bottomInset,
    required this.stats,
    required this.palette,
    required this.highlightedIndex,
    required this.onToggle,
    required this.onHighlight,
  });

  final bool expanded;

  /// The device's bottom safe-area inset (home indicator). Added to the list's
  /// bottom padding so the surface extends under the indicator while the last
  /// row stays clear of it.
  final double bottomInset;
  final List<ColorStat> stats;
  final List<PaletteColor> palette;
  final int? highlightedIndex;
  final VoidCallback onToggle;
  final void Function(int paletteIndex) onHighlight;

  int? _indexFor(String code) {
    final i = palette.indexWhere((p) => p.code == code);
    return i < 0 ? null : i;
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    // Material (not a plain Container) so the ListTiles below paint their ink
    // splashes on it — a decorated Container between ListTile and Material
    // triggers "ink may be invisible" spam.
    return Material(
      color: theme.colorScheme.surface,
      borderRadius: const BorderRadius.vertical(top: Radius.circular(16)),
      elevation: 8,
      // ClipRect so the list doesn't paint outside the animated height.
      child: ClipRect(
        child: Column(
          children: [
            // Header bar — always visible, tappable to toggle.
            InkWell(
              onTap: onToggle,
              child: Padding(
                padding: const EdgeInsets.fromLTRB(16, 12, 16, 10),
                child: Row(
                  children: [
                    AnimatedRotation(
                      turns: expanded ? 0.5 : 0.0,
                      duration: const Duration(milliseconds: 240),
                      child: Icon(
                        Icons.keyboard_arrow_up,
                        size: 20,
                        color: theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                    const SizedBox(width: 6),
                    Text(
                      AppLocalizations.of(context).legendColorCount(stats.length),
                      style: theme.textTheme.titleMedium,
                    ),
                    const Spacer(),
                    if (!expanded)
                      for (final ci
                          in stats.take(5).map((s) => _indexFor(s.code)))
                        if (ci != null)
                          Padding(
                            padding: const EdgeInsets.only(left: 3),
                            child: Container(
                              width: 13,
                              height: 13,
                              decoration: BoxDecoration(
                                color: palette[ci].rgb,
                                borderRadius: BorderRadius.circular(3),
                                border: Border.all(
                                  color: theme.colorScheme.outline,
                                  width: 0.5,
                                ),
                              ),
                            ),
                          ),
                  ],
                ),
              ),
            ),
            // Scrollable color list — fills the remaining animated height.
            Expanded(
              child: ListView.builder(
                padding: EdgeInsets.only(bottom: 8 + bottomInset),
                itemCount: stats.length,
                itemBuilder: (ctx, i) {
                  final s = stats[i];
                  final idx = _indexFor(s.code);
                  return _LegendTile(
                    stat: s,
                    color: idx == null ? null : palette[idx],
                    highlighted: idx != null && highlightedIndex == idx,
                    onTap: idx == null ? null : () => onHighlight(idx),
                  );
                },
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _LegendTile extends StatelessWidget {
  const _LegendTile({
    required this.stat,
    required this.color,
    required this.highlighted,
    required this.onTap,
  });

  final ColorStat stat;
  final PaletteColor? color;
  final bool highlighted;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return ListTile(
      onTap: onTap,
      leading: Container(
        width: 28,
        height: 28,
        decoration: BoxDecoration(
          color: color?.rgb ?? theme.colorScheme.outline,
          borderRadius: BorderRadius.circular(7),
          border: Border.all(
            color: highlighted
                ? theme.colorScheme.primary
                : theme.colorScheme.outline,
            width: highlighted ? 2 : 0.5,
          ),
        ),
      ),
      title: Text(stat.name),
      subtitle: Text(
        stat.code,
        style: monoTextStyle.copyWith(color: theme.colorScheme.primary),
      ),
      trailing: Text(
        '${stat.count}',
        style: monoTextStyle.copyWith(color: theme.colorScheme.onSurface),
      ),
    );
  }
}

void _showPreview(BuildContext context, Uint8List png) {
  showDialog<void>(
    context: context,
    builder: (ctx) => Dialog(
      backgroundColor: Colors.transparent,
      insetPadding: const EdgeInsets.all(16),
      child: GestureDetector(
        onTap: () => Navigator.pop(ctx),
        child: InteractiveViewer(
          child: ClipRRect(
            borderRadius: BorderRadius.circular(16),
            child: Image.memory(png, gaplessPlayback: true),
          ),
        ),
      ),
    ),
  );
}

void _copySummary(BuildContext context, WidgetRef ref, String summary) {
  ref.read(copySummaryProvider).call(summary).then((_) {
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(AppLocalizations.of(context).resultCopiedSummary)),
      );
    }
  });
}

bool _albumSaveInFlight = false;

/// Render the grid to PNG at the CURRENT `k` (CPU raster, design D4 — never
/// `output.gridPng`, which is now a dead engine artifact on the App side), then
/// save it to the album. Any failure — render or album — falls through to the
/// same catch/snackbar so a bad save never crashes the page.
Future<void> _saveGridToAlbum(
  BuildContext context,
  WidgetRef ref,
  GenerateOutput output,
  List<PaletteColor> palette,
  int borderRings,
) async {
  if (_albumSaveInFlight) return;
  _albumSaveInFlight = true;
  try {
    // ponytail: synchronous CPU raster on the tap. The canvas is hard-clamped
    // to 4096px in renderPatternPng, so cost is bounded; move to `compute` only
    // if a huge pattern visibly janks the frame.
    final png = renderPatternPng(
      cells: output.pattern.cells,
      width: output.pattern.width,
      height: output.pattern.height,
      palette: palette,
      borderRings: borderRings,
    );
    await ref.read(saveToAlbumProvider).call(png);
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
            content: Text(AppLocalizations.of(context).resultSavedToAlbum)),
      );
    }
  } catch (e) {
    if (context.mounted) {
      final l10n = AppLocalizations.of(context);
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            e is AlbumAccessDenied
                ? l10n.albumPermissionDenied
                : l10n.saveFailed(e.toString()),
          ),
        ),
      );
    }
  } finally {
    _albumSaveInFlight = false;
  }
}
