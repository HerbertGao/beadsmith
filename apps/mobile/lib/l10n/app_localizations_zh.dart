// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Chinese (`zh`).
class AppLocalizationsZh extends AppLocalizations {
  AppLocalizationsZh([String locale = 'zh']) : super(locale);

  @override
  String get appTitle => '拼豆匠';

  @override
  String get homeHeadline => '拼豆图纸生成器';

  @override
  String get homeSubtitle => '选一张图片，裁剪后生成拼豆图纸';

  @override
  String get homePickImage => '选择图片';

  @override
  String get cropTitle => '裁剪';

  @override
  String get cropConfirm => '确认裁剪';

  @override
  String get cropNoImage => '没有图片，请返回重新选择';

  @override
  String get cropCannotReadImage => '无法读取该图片，请返回重新选择';

  @override
  String get cropCannotDecodeImage => '无法解码该图片，请返回重新选择';

  @override
  String cropFailed(String error) {
    return '裁剪失败：$error';
  }

  @override
  String get cropAspectSquare => '正方形';

  @override
  String get cropOrientationPortrait => '纵向';

  @override
  String get cropOrientationLandscape => '横向';

  @override
  String get cropToolAspect => '比例';

  @override
  String get cropToolRotate => '旋转';

  @override
  String get cropToolFlip => '翻转';

  @override
  String get cropToolReset => '重置';

  @override
  String get generateTitle => '设置';

  @override
  String get generateWidth => '宽 (豆)';

  @override
  String get generateHeight => '高 (豆)';

  @override
  String get generatePalette => '色卡';

  @override
  String paletteColorCount(String count) {
    return '$count 色';
  }

  @override
  String get generateModeLabel => '生成模式';

  @override
  String get generateModeStaged => '常规';

  @override
  String get generateModeGerstner => '照片';

  @override
  String get generateLimitColors => '限制颜色数';

  @override
  String get generateMaxColors => '最大颜色数';

  @override
  String get generateDespeckle => '去斑';

  @override
  String get generateDespeckleSubtitle => '清除孤立的杂色点';

  @override
  String get generateThresholdLabel => '阈值（豆）';

  @override
  String get generateThresholdHelper => '把不超过这么多豆的孤立同色小块，并入相邻主色';

  @override
  String get borderRingsLabel => '边框圈';

  @override
  String get borderRingsHelper => '对齐实体拼豆板的留白圈数（默认值）';

  @override
  String get borderRingsResultHint => '本次预览的边框圈数，不改默认';

  @override
  String get generateSubmit => '生成';

  @override
  String get generateNoCroppedImage => '没有裁剪后的图片，请返回重新选图';

  @override
  String get generateInvalidSize => '请输入有效的宽和高';

  @override
  String get generateSizeRange => '宽和高需在 1–1000 之间';

  @override
  String get generateMaxColorsRequired => '开了「限制颜色数」请填一个有效数值';

  @override
  String get generateDespeckleRequired => '开了「去斑」请填一个有效阈值';

  @override
  String get resultTitle => '结果';

  @override
  String get resultNoResult => '没有结果';

  @override
  String get resultSaveToAlbum => '保存到相册';

  @override
  String get resultCopySummary => '复制汇总';

  @override
  String cellDetailCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 颗',
    );
    return '$_temp0';
  }

  @override
  String cellDetailPosition(int row, int col) {
    return '位置：第 $row 行 · 第 $col 列';
  }

  @override
  String get resultHighlightSame => '高亮所有同色格子';

  @override
  String get resultCancelHighlight => '取消高亮同色';

  @override
  String get resultSaveTip => '建议保存到相册';

  @override
  String get resultSave => '保存';

  @override
  String legendColorCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 色',
    );
    return '配色 · $_temp0';
  }

  @override
  String get resultCopiedSummary => '已复制汇总';

  @override
  String get resultSavedToAlbum => '已保存到相册';

  @override
  String get albumPermissionDenied => '相册权限被拒绝，请在系统设置中允许访问';

  @override
  String saveFailed(String error) {
    return '保存失败: $error';
  }
}
