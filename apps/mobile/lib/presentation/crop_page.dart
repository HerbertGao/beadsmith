import 'package:crop_your_image/crop_your_image.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import 'session_providers.dart';

/// Step 2: interactive crop (crop_your_image). The cropped bytes — NOT a crop
/// rect — are the only thing handed onward (design D5); the engine still does
/// its own crop_center + resize. No pixel-level crop algorithm lives here.
class CropPage extends ConsumerStatefulWidget {
  const CropPage({super.key});

  @override
  ConsumerState<CropPage> createState() => _CropPageState();
}

class _CropPageState extends ConsumerState<CropPage> {
  final _controller = CropController();

  @override
  Widget build(BuildContext context) {
    final image = ref.watch(pickedImageProvider);
    return Scaffold(
      appBar: AppBar(
        title: const Text('裁剪'),
        actions: [
          IconButton(
            icon: const Icon(Icons.check),
            tooltip: '确认裁剪',
            onPressed: image == null ? null : _controller.crop,
          ),
        ],
      ),
      body: image == null
          ? const Center(child: Text('没有图片，请返回重新选择'))
          : Crop(
              image: image,
              controller: _controller,
              interactive: true,
              onCropped: _onCropped,
            ),
    );
  }

  void _onCropped(CropResult result) {
    if (!mounted) return;
    switch (result) {
      case CropSuccess(:final croppedImage):
        ref.read(croppedImageProvider.notifier).set(croppedImage);
        context.push('/generate');
      case CropFailure():
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('裁剪失败，请重试')),
        );
    }
  }
}
