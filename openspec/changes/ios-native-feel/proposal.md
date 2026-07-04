## 为什么

四屏重塑(`four-screen-restyle`)用的是 Material 组件,在 iOS 上一股 Android 味:开关是 Material
方形拨钮而非 iOS 药丸、生成 loading 是 Material 圆环而非 iOS 菊花、分段控件是 Material 样式。目标是
让 App 在 **iOS** 上更贴近系统手感,同时**保留 pegboard 品牌配色、Android 不受影响**。

采「iOS 交互手感 + 自有配色」路线(最小侵入档):只换少数高信号控件为平台自适应,不做纯系统皮、
不引重型 platform-widgets 抽象层、不换 `CupertinoApp`。

## 变更内容

- **`.adaptive` 构造器**(iOS 渲染 Cupertino、Android 仍 Material,自动分平台):
  - `Switch` → `Switch.adaptive`(设置页「限制颜色数」「去斑」两个开关)。
  - `CircularProgressIndicator` → `CircularProgressIndicator.adaptive`(生成中的 loading,以及**裁剪屏
    读尺寸的 loading**——两处都换,免得 iOS 上一处菊花一处圆环)。
  - 若有 `AlertDialog`/`showDialog` → `AlertDialog.adaptive`/`showAdaptiveDialog`(当前四屏未用弹窗,
    确认后按需)。
- **分段控件**(两处:设置页「生成模式」+ 裁剪比例菜单的「纵/横」):iOS 用 `CupertinoSlidingSegmentedControl`
  (`package:flutter/cupertino.dart`),Android 保留 `SegmentedButton`——按 **`Theme.of(context).platform ==
  TargetPlatform.iOS`** 分支(`CupertinoSlidingSegmentedControl` 无 `.adaptive`,且在 Android 上是非原生外观,
  故须手动分平台;**用 Theme 平台而非 `dart:io Platform.isIOS`,以便 widget 测试可覆写**)。两处二者取值语义一致。
- **页面转场 / 手势**:`MaterialApp` 在 iOS 目标下**默认**已用 `CupertinoPageTransitionsBuilder`(横滑 +
  边缘滑动返回)、列表默认 `BouncingScrollPhysics`——故转场手感**多半已是 iOS,本变更主要是确认**;如显式
  钉 `ThemeData.pageTransitionsTheme` 固化,必须**保留完整 builder map**(iOS/macOS→Cupertino、Android→
  当前 SDK 默认 `PredictiveBackPageTransitionsBuilder`、桌面→Zoom),**禁止**只写 iOS+Android 两项(会让其它
  平台回退变化),也**禁止**用 `platform: TargetPlatform.iOS`(那会把 iOS 行为套到 Android)。

保留:pegboard tokens / light+dark 主题、`MaterialApp.router` 骨架(Cupertino 组件可嵌入其中)、
豆号/豆数 mono、所有既有交互契约(比例锁定、选项透传、裁剪几何)。**明确留 Material**(本轮不动):AppBar、
`TextField` / 尺寸步进器、`ActionChip`、`FilledButton`、`SnackBar`(iOS 无对应或非高信号,配色已套主题)。

### 非目标

- 不改 `bead-core` / `bead-ffi` / 引擎 / 确定性。
- **不做四屏纯 Cupertino 重写**、不换 `CupertinoApp`、不引 `flutter_platform_widgets`。
- 不改配色 tokens、不改裁剪几何、不改选项透传/比例锁定契约。
- 不追求「像素级 iOS 系统外观」——品牌配色优先,只取 iOS 的**交互手感**(控件形态 + 转场 + 手势)。

## 功能 (Capabilities)

### 新增功能
<!-- 无新增 capability:修改既有 mobile-app 契约。 -->

### 修改功能
- `mobile-app`:**新增**「iOS 上采用平台自适应控件与转场」需求——在 iOS 目标上,开关/进度/弹窗用
  `.adaptive`、生成模式分段用 Cupertino sliding、转场用 iOS 横滑 + 滑动返回;**Android 保持 Material、
  无回归**;纯表现层,配色/分层/确定性不变。

## 影响

- **代码(仅 `apps/mobile` presentation)**:
  - `generate_page.dart`:两处 `SwitchListTile`→`.adaptive`;生成 loading→`.adaptive`;生成模式分段按
    `Theme.of(context).platform == TargetPlatform.iOS` 分 `CupertinoSlidingSegmentedControl` / `SegmentedButton`
    (加 `package:flutter/cupertino.dart` 导入)。
  - `crop_page.dart`(读尺寸 loading)/`crop_frame.dart`(纵/横分段):同样换 `.adaptive` / iOS Cupertino sliding。
  - `theme.dart`(可选):若钉 `pageTransitionsTheme` 须保留完整 builder map(iOS→Cupertino、Android→SDK 默认、
    桌面→Zoom),仅把 iOS 显式化;否则不动(iOS 默认已是 Cupertino 转场)。
- **`bead-core` / `bead-ffi`**:零改动。确定性 / 「CLI == FFI」不受影响(纯表现层)。
- **依赖**:不新增(`Switch.adaptive` / `CupertinoSlidingSegmentedControl` / `CupertinoPageTransitionsBuilder`
  均在 Flutter SDK 内)。
- **里程碑**:Post-M9 之后的 UI 润色(独立于四屏重塑)。
