## 上下文

四屏已在 `four-screen-restyle` 用 Material 组件套 pegboard 主题。Material 开关/进度/分段在 iOS 上是
Android 观感。本变更只把少数高信号控件换成**平台自适应**,让 iOS 拿到系统手感,Android 不变,品牌配色
不变。`MaterialApp.router` + go_router 骨架保留(Cupertino 组件可直接嵌入 MaterialApp)。

## 目标 / 非目标

**目标:** iOS 上开关/进度用 `.adaptive`(→ Cupertino)、生成模式用 iOS sliding 分段、转场为 iOS 横滑 +
滑动返回;Android 保持 Material;配色/分层/确定性/交互契约全不变。

**非目标:** 纯 Cupertino 重写 / 换 `CupertinoApp`;引 `flutter_platform_widgets`;改 bead-core/ffi/引擎/
确定性;改配色 tokens、裁剪几何、选项透传/比例锁定契约。

## 决策

**决策 1:presentation 层平台判断用 `Theme.of(context).platform`,不用 `dart:io` `Platform.isIOS`。**
`.adaptive` 系构造器内部按 `Theme.of(context).platform` / `defaultTargetPlatform` 决定 Cupertino vs
Material。手动分支(分段控件)也统一用 `Theme.of(context).platform == TargetPlatform.iOS`——它**可被
widget 测试覆写**(`debugDefaultTargetPlatformOverride`),而 `Platform.isIOS`(宿主 OS)在测试里恒为
macOS=false、且不随 Theme 平台变。**限定范围**:此禁令只针对**表现层控件/转场的平台分支**;
**infrastructure 层按真实宿主 OS 选原生库/能力仍可用 `dart:io Platform`**(如 `bead_ffi_loader.dart`
用 `Platform.isIOS` 选 `.process`/`.so` —— 那里没有 `BuildContext`、要的正是宿主 OS,不在本禁令内)。

**决策 2:`.adaptive` 换开关与进度。**
- `SwitchListTile` → `SwitchListTile.adaptive`(「限制颜色数」「去斑」);iOS 出 `CupertinoSwitch`。
- 生成中的 `CircularProgressIndicator` → `.adaptive`;iOS 出菊花。
- 弹窗:四屏当前无 `showDialog`/`AlertDialog`(错误走页面内 `_error` 文本),故本轮无弹窗可换;若后续
  加弹窗须用 `.adaptive` / `showAdaptiveDialog`(写进 spec 作为约束)。
- 为保品牌:`CupertinoSwitch` 的激活色须取 `colorScheme.primary`(adaptive 会尽量沿用 theme,必要时显式
  传 `activeColor`/`activeTrackColor`)。

**决策 3:分段控件——iOS `CupertinoSlidingSegmentedControl`,Android `SegmentedButton`(手动分支),覆盖两处。**
`CupertinoSlidingSegmentedControl`(来自 `package:flutter/cupertino.dart`)无 `.adaptive`,且在 Android 上是非
原生外观,故按决策 1 的平台判断分两条。**App 有两处 `SegmentedButton` 都要改**,免得 iOS 上一处 Cupertino
一处 Material:①设置页「生成模式」(`generate_page.dart`);②裁剪比例菜单「纵/横」(`crop_frame.dart`)。
- iOS:`CupertinoSlidingSegmentedControl<T>`(children Map、`groupValue`、`onValueChanged` 回写;注意 API 回传
  `T?` 可空,回写时按当前值兜底),`thumbColor`/`backgroundColor` 取 tokens 保品牌。
- Android:保留既有 `SegmentedButton`。
可抽一个小 helper 复用于两处。两处取值语义与既有一致(生成模式写 `_generator`、纵/横写朝向),不改任何契约。

**决策 4:转场——iOS 默认已是 Cupertino,本变更主要「确认」;若固化须保留完整 builder map。**
`MaterialApp` 在 iOS 目标下默认已用 `CupertinoPageTransitionsBuilder`(横滑 + 边缘滑动返回)、列表默认
`BouncingScrollPhysics`——go_router 的 `MaterialPage` 走这套。故转场手感**大概率已经是 iOS**,`const
PageTransitionsTheme()` 默认即够;**默认路径就可满足本需求,可不动 `theme.dart`**。若为「防将来默认变动」
选择显式固化,则**必须写完整的 builder map**:`iOS`/`macOS` → `CupertinoPageTransitionsBuilder`、`android` →
**当前 SDK 默认 `PredictiveBackPageTransitionsBuilder`**(不是笼统「M3」)、`windows`/`linux` → `ZoomPageTransitionsBuilder`
——`PageTransitionsTheme` 无占位项,**只写 iOS+Android 两项会让其它平台回退变化**。**禁止**用 `platform:
TargetPlatform.iOS`(会把 iOS 行为强加到 Android)。

**决策 5:骨架不换。**
保留 `MaterialApp.router`;Cupertino 控件嵌入 Material 树完全合法。换 `CupertinoApp` 会丢 Material 主题化 +
是大改,超范围。

## 风险 / 权衡

- **测试稳定性**:`.adaptive` / 分段分支按 `Theme.of(context).platform` 决定。widget 测试默认
  `TargetPlatform.android` → 走 Material 分支,既有 `option_forwarding_test`(用 `SwitchListTile`/
  `SegmentedButton` 的 finder)**仍可用**(`SwitchListTile.adaptive` 仍是 `SwitchListTile` 类型,iOS-only 的
  Cupertino 分段不影响 android 分支)。
- **iOS 分段转发不能只测「存在」(防重新挖坑)**:iOS 用 `CupertinoSlidingSegmentedControl` 是一条**新代码
  路径**(`onValueChanged(T?)` → 回写 `_generator`);只断言它「渲染出来」会漏掉转发接错——正是仓库
  `option_forwarding_test` 存在的意义。故 iOS-override 测试**必须点选该 Cupertino 分段、断言选中的
  `generator` 值原样抵达替身桥**(复用 `_FakeBridge`),不止验存在性。
- **品牌一致**:Cupertino 控件自带配色,须显式喂 tokens(激活色/thumb/背景),否则会偏离 pegboard。
- **Android 回归**:每处改动都要保证 Android 分支 == 改动前(Material 原样)。
- **转场其实已 iOS**:决策 4 多为「确认」,实际视觉改动可能很小——这是诚实预期,不是缺陷。

## Open Questions

- `CupertinoSlidingSegmentedControl` 的 thumb/背景取哪几个 token(primary/surface 组合),实现时目测定。
- **macOS 落在两谓词之间(潜在,非本产品)**:`.adaptive` 把 macOS 也当 Cupertino,而分段手动分支只判
  `== TargetPlatform.iOS`——macOS 宿主上会出现「开关 Cupertino、分段 Material」。对 iOS/Android 产品无影响;
  若在意可让分段 helper 用 `{iOS, macOS}` 集合判断。记录备忘,不阻塞。

## 明确排除(本轮不做)

- **Material ripple 抑制**(`splashFactory: NoSplash`):属更广的 Material→iOS 外观迁移,**超出本轮范围**,
  留作后续;本变更不动 splash/ripple。
- AppBar→`CupertinoNavigationBar`、`TextField`→`CupertinoTextField`、`SnackBar`(iOS 无对应)等,均保持 Material。
