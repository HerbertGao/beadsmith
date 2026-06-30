import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../application/providers.dart';
import 'session_providers.dart';

/// Step 4: show preview / color counts / summary — ALL taken verbatim from the
/// `GenerateOutput` (never recomputed from the rendered image, spec). Copy goes
/// through the CopySummary use case → ClipboardService (keeps the layering).
class ResultPage extends ConsumerWidget {
  const ResultPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final output = ref.watch(generateResultProvider);
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
        children: [
          Image.memory(output.previewPng),
          const Divider(),
          for (final s in output.stats)
            ListTile(
              dense: true,
              title: Text('${s.code} · ${s.name}'),
              trailing: Text('${s.count}'),
            ),
          const Divider(),
          Padding(
            padding: const EdgeInsets.all(16),
            child: SelectableText(output.summary),
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
