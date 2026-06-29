import 'package:flutter/services.dart';

/// Wraps the platform clipboard so presentation never touches it directly
/// (keeps the layering: presentation → application → infrastructure).
class ClipboardService {
  const ClipboardService();

  Future<void> copy(String text) => Clipboard.setData(ClipboardData(text: text));
}
