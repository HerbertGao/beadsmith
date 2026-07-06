import 'dart:typed_data';

import '../infrastructure/album_service.dart';

/// Use case: persist a generated image to the device album so it survives a
/// background kill (the user can switch apps without losing the result).
///
/// Thin wrapper over [AlbumService] — keeps the application layer free of
/// `gal` and the presentation layer free of persistence details.
class SaveToAlbum {
  const SaveToAlbum(this._album);

  final AlbumService _album;

  Future<void> call(Uint8List pngBytes) => _album.saveImage(pngBytes);
}
