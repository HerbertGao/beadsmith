import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:image/image.dart' as img;

import 'crop_frame.dart';
import 'crop_geometry.dart';
import 'session_providers.dart';

/// Step 2: self-drawn crop. A fixed-aspect [CropFrame] frames the picked image;
/// on confirm we orient the decoded bytes (rotate THEN flip — design 决策 2),
/// crop the framed rect in ORIENTED-image space, re-encode PNG, and hand those
/// bytes onward. No `RepaintBoundary.toImage` (fails on the iOS simulator); the
/// engine still does its own crop_center + resize (design D5 / CLAUDE rule 4).
class CropPage extends ConsumerStatefulWidget {
  const CropPage({super.key});

  @override
  ConsumerState<CropPage> createState() => _CropPageState();
}

class _CropPageState extends ConsumerState<CropPage> {
  Size? _srcSize;
  String? _error;
  CropFrameState _state = CropFrameState.initial;

  @override
  void initState() {
    super.initState();
    _resolveSize();
  }

  Future<void> _resolveSize() async {
    final bytes = ref.read(pickedImageProvider);
    if (bytes == null) return;
    try {
      final decoded = await ui.instantiateImageCodec(bytes);
      final frame = await decoded.getNextFrame();
      final w = frame.image.width, h = frame.image.height;
      frame.image.dispose();
      if (mounted) setState(() => _srcSize = Size(w.toDouble(), h.toDouble()));
    } catch (_) {
      if (mounted) setState(() => _error = '无法读取该图片，请返回重新选择');
    }
  }

  void _confirm() {
    final bytes = ref.read(pickedImageProvider);
    if (bytes == null) return;
    try {
      final decoded = img.decodeImage(bytes);
      if (decoded == null) {
        _showError('无法解码该图片，请返回重新选择');
        return;
      }
      // Orient first (rotate THEN flip — pinned order), then crop in the
      // oriented image's pixel space (the space computeCropRect returns).
      var oriented = decoded;
      if (_state.quarterTurns != 0) {
        oriented = img.copyRotate(oriented, angle: _state.quarterTurns * 90);
      }
      if (_state.flipH) {
        oriented = img.flipHorizontal(oriented);
      }
      final rect = computeCropRect(
        srcWidth: decoded.width,
        srcHeight: decoded.height,
        frameAspect: _state.aspect,
        zoom: _state.zoom,
        panX: _state.panX,
        panY: _state.panY,
        quarterTurns: _state.quarterTurns,
      );
      final cropped = img.copyCrop(
        oriented,
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
      );
      final png = Uint8List.fromList(img.encodePng(cropped));
      ref.read(croppedImageProvider.notifier).set(png);
      ref.read(cropAspectProvider.notifier).set(_state.aspect);
      context.push('/generate');
    } catch (e) {
      _showError('裁剪失败：$e');
    }
  }

  void _showError(String msg) {
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(msg)));
  }

  @override
  Widget build(BuildContext context) {
    final bytes = ref.watch(pickedImageProvider);
    final ready = bytes != null && _srcSize != null && _error == null;
    return Scaffold(
      appBar: AppBar(
        title: const Text('裁剪'),
        actions: [
          IconButton(
            icon: const Icon(Icons.check),
            tooltip: '确认裁剪',
            onPressed: ready ? _confirm : null,
          ),
        ],
      ),
      body: bytes == null
          ? const Center(child: Text('没有图片，请返回重新选择'))
          : _error != null
              ? Center(child: Text(_error!))
              : _srcSize == null
                  ? const Center(child: CircularProgressIndicator())
                  : CropFrame(
                      imageBytes: bytes,
                      imageSize: _srcSize!,
                      onChanged: (s) => _state = s,
                    ),
    );
  }
}
