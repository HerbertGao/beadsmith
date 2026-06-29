import '../infrastructure/clipboard_service.dart';

/// Use case: copy the `GenerateOutput.summary` text verbatim to the clipboard.
class CopySummary {
  const CopySummary(this._clipboard);

  final ClipboardService _clipboard;

  Future<void> call(String summary) => _clipboard.copy(summary);
}
