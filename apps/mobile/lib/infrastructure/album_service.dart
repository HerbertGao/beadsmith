import 'dart:typed_data';

import 'package:gal/gal.dart';

/// Saves image bytes to the device photo album via `gal`. iOS needs
/// `NSPhotoLibraryAddUsageDescription` in Info.plist (write-only access);
/// Android 13+ needs no permission, older versions are handled by `gal`.
class AlbumService {
  const AlbumService();

  /// Save [pngBytes] to the album as `beadsmith-<timestamp>.png`.
  ///
  /// Requests access first if not already granted; throws [AlbumAccessDenied]
  /// if the user declines so the UI can show a meaningful message.
  Future<void> saveImage(Uint8List pngBytes) async {
    if (!await Gal.hasAccess(toAlbum: true)) {
      final granted = await Gal.requestAccess(toAlbum: true);
      if (!granted) throw AlbumAccessDenied();
    }
    await Gal.putImageBytes(
      pngBytes,
      name: 'beadsmith-${DateTime.now().millisecondsSinceEpoch}',
    );
  }
}

/// Raised when the user declined the album-access prompt.
class AlbumAccessDenied implements Exception {
  AlbumAccessDenied();
  @override
  String toString() => 'AlbumAccessDenied: user declined photo album access';
}
