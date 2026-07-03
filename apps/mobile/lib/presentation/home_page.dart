import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:image_picker/image_picker.dart';

import 'session_providers.dart';

/// Step 1: pick an image (gallery) → route to CropPage.
class HomePage extends ConsumerWidget {
  const HomePage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    return Scaffold(
      appBar: AppBar(title: const Text('Beadsmith')),
      body: Center(
        child: Padding(
          padding: const EdgeInsets.all(32),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                padding: const EdgeInsets.all(24),
                decoration: BoxDecoration(
                  color: scheme.primary.withValues(alpha: 0.12),
                  shape: BoxShape.circle,
                ),
                child: Icon(Icons.grid_on, size: 56, color: scheme.primary),
              ),
              const SizedBox(height: 24),
              Text('拼豆图纸生成器', style: theme.textTheme.headlineSmall),
              const SizedBox(height: 8),
              Text(
                '选一张图片，裁剪后生成拼豆图纸',
                style: theme.textTheme.bodyMedium
                    ?.copyWith(color: scheme.onSurfaceVariant),
              ),
              const SizedBox(height: 32),
              FilledButton.icon(
                icon: const Icon(Icons.photo_library),
                label: const Text('选择图片'),
                onPressed: () => _pick(context, ref),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _pick(BuildContext context, WidgetRef ref) async {
    final file = await ImagePicker().pickImage(source: ImageSource.gallery);
    if (file == null) return;
    final bytes = await file.readAsBytes();
    if (!context.mounted) return;
    ref.read(pickedImageProvider.notifier).set(bytes);
    context.push('/crop');
  }
}
