import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_en.dart';
import 'app_localizations_zh.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'l10n/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations)!;
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('zh'),
    Locale('en'),
  ];

  /// 应用显示名 / MaterialApp 标题
  ///
  /// In zh, this message translates to:
  /// **'拼豆匠'**
  String get appTitle;

  /// No description provided for @homeHeadline.
  ///
  /// In zh, this message translates to:
  /// **'拼豆图纸生成器'**
  String get homeHeadline;

  /// No description provided for @homeSubtitle.
  ///
  /// In zh, this message translates to:
  /// **'选一张图片，裁剪后生成拼豆图纸'**
  String get homeSubtitle;

  /// No description provided for @homePickImage.
  ///
  /// In zh, this message translates to:
  /// **'选择图片'**
  String get homePickImage;

  /// No description provided for @cropTitle.
  ///
  /// In zh, this message translates to:
  /// **'裁剪'**
  String get cropTitle;

  /// No description provided for @cropConfirm.
  ///
  /// In zh, this message translates to:
  /// **'确认裁剪'**
  String get cropConfirm;

  /// No description provided for @cropNoImage.
  ///
  /// In zh, this message translates to:
  /// **'没有图片，请返回重新选择'**
  String get cropNoImage;

  /// No description provided for @cropCannotReadImage.
  ///
  /// In zh, this message translates to:
  /// **'无法读取该图片，请返回重新选择'**
  String get cropCannotReadImage;

  /// No description provided for @cropCannotDecodeImage.
  ///
  /// In zh, this message translates to:
  /// **'无法解码该图片，请返回重新选择'**
  String get cropCannotDecodeImage;

  /// No description provided for @cropFailed.
  ///
  /// In zh, this message translates to:
  /// **'裁剪失败：{error}'**
  String cropFailed(String error);

  /// No description provided for @cropAspectSquare.
  ///
  /// In zh, this message translates to:
  /// **'正方形'**
  String get cropAspectSquare;

  /// No description provided for @cropOrientationPortrait.
  ///
  /// In zh, this message translates to:
  /// **'纵向'**
  String get cropOrientationPortrait;

  /// No description provided for @cropOrientationLandscape.
  ///
  /// In zh, this message translates to:
  /// **'横向'**
  String get cropOrientationLandscape;

  /// No description provided for @cropToolAspect.
  ///
  /// In zh, this message translates to:
  /// **'比例'**
  String get cropToolAspect;

  /// No description provided for @cropToolRotate.
  ///
  /// In zh, this message translates to:
  /// **'旋转'**
  String get cropToolRotate;

  /// No description provided for @cropToolFlip.
  ///
  /// In zh, this message translates to:
  /// **'翻转'**
  String get cropToolFlip;

  /// No description provided for @cropToolReset.
  ///
  /// In zh, this message translates to:
  /// **'重置'**
  String get cropToolReset;

  /// No description provided for @generateTitle.
  ///
  /// In zh, this message translates to:
  /// **'设置'**
  String get generateTitle;

  /// No description provided for @generateWidth.
  ///
  /// In zh, this message translates to:
  /// **'宽 (豆)'**
  String get generateWidth;

  /// No description provided for @generateHeight.
  ///
  /// In zh, this message translates to:
  /// **'高 (豆)'**
  String get generateHeight;

  /// No description provided for @generatePalette.
  ///
  /// In zh, this message translates to:
  /// **'色卡'**
  String get generatePalette;

  /// No description provided for @paletteColorCount.
  ///
  /// In zh, this message translates to:
  /// **'{count} 色'**
  String paletteColorCount(String count);

  /// No description provided for @generateModeLabel.
  ///
  /// In zh, this message translates to:
  /// **'生成模式'**
  String get generateModeLabel;

  /// No description provided for @generateModeStaged.
  ///
  /// In zh, this message translates to:
  /// **'常规'**
  String get generateModeStaged;

  /// No description provided for @generateModeGerstner.
  ///
  /// In zh, this message translates to:
  /// **'照片'**
  String get generateModeGerstner;

  /// No description provided for @generateLimitColors.
  ///
  /// In zh, this message translates to:
  /// **'限制颜色数'**
  String get generateLimitColors;

  /// No description provided for @generateMaxColors.
  ///
  /// In zh, this message translates to:
  /// **'最大颜色数'**
  String get generateMaxColors;

  /// No description provided for @generateDespeckle.
  ///
  /// In zh, this message translates to:
  /// **'去斑'**
  String get generateDespeckle;

  /// No description provided for @generateDespeckleSubtitle.
  ///
  /// In zh, this message translates to:
  /// **'清除孤立的杂色点'**
  String get generateDespeckleSubtitle;

  /// No description provided for @generateThresholdLabel.
  ///
  /// In zh, this message translates to:
  /// **'阈值（豆）'**
  String get generateThresholdLabel;

  /// No description provided for @generateThresholdHelper.
  ///
  /// In zh, this message translates to:
  /// **'把不超过这么多豆的孤立同色小块，并入相邻主色'**
  String get generateThresholdHelper;

  /// No description provided for @generateSubmit.
  ///
  /// In zh, this message translates to:
  /// **'生成'**
  String get generateSubmit;

  /// No description provided for @generateNoCroppedImage.
  ///
  /// In zh, this message translates to:
  /// **'没有裁剪后的图片，请返回重新选图'**
  String get generateNoCroppedImage;

  /// No description provided for @generateInvalidSize.
  ///
  /// In zh, this message translates to:
  /// **'请输入有效的宽和高'**
  String get generateInvalidSize;

  /// No description provided for @generateSizeRange.
  ///
  /// In zh, this message translates to:
  /// **'宽和高需在 1–1000 之间'**
  String get generateSizeRange;

  /// No description provided for @generateMaxColorsRequired.
  ///
  /// In zh, this message translates to:
  /// **'开了「限制颜色数」请填一个有效数值'**
  String get generateMaxColorsRequired;

  /// No description provided for @generateDespeckleRequired.
  ///
  /// In zh, this message translates to:
  /// **'开了「去斑」请填一个有效阈值'**
  String get generateDespeckleRequired;

  /// No description provided for @resultTitle.
  ///
  /// In zh, this message translates to:
  /// **'结果'**
  String get resultTitle;

  /// No description provided for @resultNoResult.
  ///
  /// In zh, this message translates to:
  /// **'没有结果'**
  String get resultNoResult;

  /// No description provided for @resultSaveToAlbum.
  ///
  /// In zh, this message translates to:
  /// **'保存到相册'**
  String get resultSaveToAlbum;

  /// No description provided for @resultCopySummary.
  ///
  /// In zh, this message translates to:
  /// **'复制汇总'**
  String get resultCopySummary;

  /// No description provided for @cellDetailCount.
  ///
  /// In zh, this message translates to:
  /// **'{count, plural, other{{count} 颗}}'**
  String cellDetailCount(int count);

  /// No description provided for @cellDetailPosition.
  ///
  /// In zh, this message translates to:
  /// **'位置：第 {row} 行 · 第 {col} 列'**
  String cellDetailPosition(int row, int col);

  /// No description provided for @resultHighlightSame.
  ///
  /// In zh, this message translates to:
  /// **'高亮所有同色格子'**
  String get resultHighlightSame;

  /// No description provided for @resultCancelHighlight.
  ///
  /// In zh, this message translates to:
  /// **'取消高亮同色'**
  String get resultCancelHighlight;

  /// No description provided for @resultSaveTip.
  ///
  /// In zh, this message translates to:
  /// **'建议保存到相册'**
  String get resultSaveTip;

  /// No description provided for @resultSave.
  ///
  /// In zh, this message translates to:
  /// **'保存'**
  String get resultSave;

  /// No description provided for @legendColorCount.
  ///
  /// In zh, this message translates to:
  /// **'配色 · {count, plural, other{{count} 色}}'**
  String legendColorCount(int count);

  /// No description provided for @resultCopiedSummary.
  ///
  /// In zh, this message translates to:
  /// **'已复制汇总'**
  String get resultCopiedSummary;

  /// No description provided for @resultSavedToAlbum.
  ///
  /// In zh, this message translates to:
  /// **'已保存到相册'**
  String get resultSavedToAlbum;

  /// No description provided for @albumPermissionDenied.
  ///
  /// In zh, this message translates to:
  /// **'相册权限被拒绝，请在系统设置中允许访问'**
  String get albumPermissionDenied;

  /// No description provided for @saveFailed.
  ///
  /// In zh, this message translates to:
  /// **'保存失败: {error}'**
  String saveFailed(String error);
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['en', 'zh'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'en':
      return AppLocalizationsEn();
    case 'zh':
      return AppLocalizationsZh();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
