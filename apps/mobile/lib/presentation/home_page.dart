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
    return Scaffold(
      appBar: AppBar(title: const Text('Beadsmith')),
      body: Center(
        child: ElevatedButton.icon(
          icon: const Icon(Icons.photo_library),
          label: const Text('选择图片'),
          onPressed: () => _pick(context, ref),
        ),
      ),
    );
  }

  Future<void> _pick(BuildContext context, WidgetRef ref) async {
    final file = await ImagePicker().pickImage(source: ImageSource.gallery);
    if (file == null) return;
    final bytes = await file.readAsBytes();
    ref.read(pickedImageProvider.notifier).set(bytes);
    if (context.mounted) context.push('/crop');
  }
}
