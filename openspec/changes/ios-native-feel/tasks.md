## 1. 自适应控件(iOS→Cupertino / Android→Material)

- [x] 1.1 `generate_page.dart`:两处 `SwitchListTile` → `SwitchListTile.adaptive`(「限制颜色数」「去斑」);iOS 出 `CupertinoSwitch`。为保品牌,激活色显式取 `colorScheme.primary`(必要时传 `activeColor`/`activeTrackColor`)。
- [x] 1.2 **两处** `CircularProgressIndicator` → `.adaptive`(iOS 出菊花):①`generate_page.dart:312`(生成按钮 loading);②`crop_page.dart:118`(读尺寸 loading)——两处都换,免得 iOS 上一处菊花一处圆环。
- [x] 1.3 全仓扫 `showDialog(` / `AlertDialog(`:若有则换 `showAdaptiveDialog` / `AlertDialog.adaptive`;当前四屏错误走页面内文本、无弹窗,则记录为 no-op(spec 已留「未来弹窗须 adaptive」约束)。

## 2. 分段控件——平台分支(两处)

- [x] 2.1 抽一个小 helper 按 `Theme.of(context).platform == TargetPlatform.iOS` 分:**iOS** 用 `CupertinoSlidingSegmentedControl<T>`(需 `import 'package:flutter/cupertino.dart';`;children Map、`groupValue`、`onValueChanged`;注意回调传 `T?` 可空,回写时按当前值兜底;`thumbColor`/`backgroundColor` 取 tokens 保品牌);**Android** 保留 `SegmentedButton`。**禁止**用 `dart:io` `Platform.isIOS`(用 `Theme.of(context).platform`,可被测试覆写)。
- [x] 2.2 用 2.1 的 helper 改**两处** `SegmentedButton`:①`generate_page.dart` 生成模式(常规/照片,写 `_generator`);②`crop_frame.dart` 裁剪比例菜单纵/横(写朝向)。取值语义与既有一致,不改任何契约。

## 3. 转场

- [x] 3.1 转场手感在 iOS 目标下 `MaterialApp` 默认已是 `CupertinoPageTransitionsBuilder`(横滑 + 滑动返回),**默认即满足需求,可不动 `theme.dart`**;仅当为「防将来默认变动」而显式固化时,才在 `pageTransitionsTheme` 写**完整 builder map**(`iOS`/`macOS`→`CupertinoPageTransitionsBuilder`、`android`→当前 SDK 默认 `PredictiveBackPageTransitionsBuilder`、`windows`/`linux`→`ZoomPageTransitionsBuilder`),**禁止**只写 iOS+Android 两项(会让其它平台回退变化),**禁止** `platform: TargetPlatform.iOS`。

## 4. 验证

- [x] 4.1 `flutter analyze` 无新增告警;`flutter test` 通过——既有 `option_forwarding_test`(widget 测试默认 `TargetPlatform.android` → 走 Material 分支)必须仍绿(finder 命中 `SwitchListTile`/`SegmentedButton`)。
- [x] 4.2 新增 iOS 分支 widget 测试(`debugDefaultTargetPlatformOverride = TargetPlatform.iOS`,`addTearDown` 复位):不止断言渲染出 `CupertinoSwitch` / `CupertinoSlidingSegmentedControl`(存在性),**还必须点选 iOS 的 Cupertino 生成模式分段(如「照片」)、经替身桥(`_FakeBridge` 复用)断言选中的 `generator` 原样抵达 `ffi.generate`**——iOS 分段是新代码路径,只测存在会漏掉转发接错。
- [x] 4.3 iOS 模拟器目测:开关药丸、两处 loading 菊花、两处分段 iOS 滑块、页面横滑 + 滑动返回;深/浅色 + 品牌配色仍在。Android(切目标或安卓模拟器)目测仍 Material、无回归。确定性/选项透传/比例锁定不受影响。
