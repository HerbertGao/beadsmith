// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get appTitle => 'Beadsmith';

  @override
  String get homeHeadline => 'Bead Pattern Generator';

  @override
  String get homeSubtitle =>
      'Pick an image, crop it, and generate a bead pattern';

  @override
  String get homePickImage => 'Pick image';

  @override
  String get cropTitle => 'Crop';

  @override
  String get cropConfirm => 'Confirm crop';

  @override
  String get cropNoImage => 'No image. Go back and pick again.';

  @override
  String get cropCannotReadImage =>
      'Can\'t read this image. Go back and pick again.';

  @override
  String get cropCannotDecodeImage =>
      'Can\'t decode this image. Go back and pick again.';

  @override
  String cropFailed(String error) {
    return 'Crop failed: $error';
  }

  @override
  String get cropAspectSquare => 'Square';

  @override
  String get cropOrientationPortrait => 'Portrait';

  @override
  String get cropOrientationLandscape => 'Landscape';

  @override
  String get cropToolAspect => 'Ratio';

  @override
  String get cropToolRotate => 'Rotate';

  @override
  String get cropToolFlip => 'Flip';

  @override
  String get cropToolReset => 'Reset';

  @override
  String get generateTitle => 'Settings';

  @override
  String get generateWidth => 'Width (beads)';

  @override
  String get generateHeight => 'Height (beads)';

  @override
  String get generatePalette => 'Palette';

  @override
  String paletteColorCount(String count) {
    return '$count colors';
  }

  @override
  String get generateModeLabel => 'Generation mode';

  @override
  String get generateModeStaged => 'Standard';

  @override
  String get generateModeGerstner => 'Photo';

  @override
  String get generateLimitColors => 'Limit colors';

  @override
  String get generateMaxColors => 'Max colors';

  @override
  String get generateDespeckle => 'Despeckle';

  @override
  String get generateDespeckleSubtitle => 'Remove isolated stray dots';

  @override
  String get generateThresholdLabel => 'Threshold (beads)';

  @override
  String get generateThresholdHelper =>
      'Merge isolated same-color blobs of at most this many beads into the neighboring main color';

  @override
  String get borderRingsLabel => 'Border rings';

  @override
  String get borderRingsHelper =>
      'Blank rings to align with a physical pegboard (default)';

  @override
  String get borderRingsResultHint =>
      'Border rings for this preview only (default unchanged)';

  @override
  String get generateSubmit => 'Generate';

  @override
  String get generateNoCroppedImage =>
      'No cropped image. Go back and pick one again.';

  @override
  String get generateInvalidSize => 'Enter a valid width and height';

  @override
  String get generateSizeRange => 'Width and height must be between 1 and 1000';

  @override
  String get generateMaxColorsRequired =>
      '\"Limit colors\" is on — enter a valid number';

  @override
  String get generateDespeckleRequired =>
      '\"Despeckle\" is on — enter a valid threshold';

  @override
  String get resultTitle => 'Result';

  @override
  String get resultNoResult => 'No result';

  @override
  String get resultSaveToAlbum => 'Save to album';

  @override
  String get resultCopySummary => 'Copy summary';

  @override
  String cellDetailCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count beads',
      one: '1 bead',
    );
    return '$_temp0';
  }

  @override
  String cellDetailPosition(int row, int col) {
    return 'Position: row $row, col $col';
  }

  @override
  String get resultHighlightSame => 'Highlight all same-color cells';

  @override
  String get resultCancelHighlight => 'Clear highlight';

  @override
  String get resultSaveTip => 'Save to album recommended';

  @override
  String get resultSave => 'Save';

  @override
  String legendColorCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count colors',
      one: '1 color',
    );
    return 'Colors · $_temp0';
  }

  @override
  String get resultCopiedSummary => 'Summary copied';

  @override
  String get resultSavedToAlbum => 'Saved to album';

  @override
  String get albumPermissionDenied =>
      'Album access denied. Allow it in system settings.';

  @override
  String saveFailed(String error) {
    return 'Save failed: $error';
  }
}
