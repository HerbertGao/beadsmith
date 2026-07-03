import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../application/providers.dart';
import 'session_providers.dart';
import 'theme.dart';

/// Step 4: show preview / color counts / summary — ALL taken verbatim from the
/// `GenerateOutput` (never recomputed from the rendered image, spec). Copy goes
/// through the CopySummary use case → ClipboardService (keeps the layering).
class ResultPage extends ConsumerWidget {
  const ResultPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final output = ref.watch(generateResultProvider);
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    if (output == null) {
      return Scaffold(
        appBar: AppBar(title: const Text('结果')),
        body: const Center(child: Text('没有结果')),
      );
    }
    return Scaffold(
      appBar: AppBar(
        title: const Text('结果'),
        actions: [
          IconButton(
            icon: const Icon(Icons.copy),
            tooltip: '复制 summary',
            onPressed: () => _copy(context, ref, output.summary),
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          // Preview derives from GenerateOutput.previewPng (never re-derived).
          Card(
            clipBehavior: Clip.antiAlias,
            child: Image.memory(output.previewPng),
          ),
          const SizedBox(height: 16),
          Text('配色', style: theme.textTheme.titleMedium),
          const SizedBox(height: 8),
          // stats/legend taken verbatim from output.stats — code & count mono.
          Card(
            child: Column(
              children: [
                for (final s in output.stats)
                  ListTile(
                    dense: true,
                    leading: Text(
                      s.code,
                      style: monoTextStyle.copyWith(color: scheme.primary),
                    ),
                    title: Text(s.name),
                    trailing: Text(
                      '${s.count}',
                      style: monoTextStyle.copyWith(color: scheme.onSurface),
                    ),
                  ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          Text('汇总', style: theme.textTheme.titleMedium),
          const SizedBox(height: 8),
          // summary taken verbatim from output.summary.
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: SelectableText(
                output.summary,
                style: monoTextStyle.copyWith(color: scheme.onSurface),
              ),
            ),
          ),
        ],
      ),
    );
  }

  Future<void> _copy(BuildContext context, WidgetRef ref, String summary) async {
    await ref.read(copySummaryProvider).call(summary);
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('已复制 summary')),
      );
    }
  }
}
