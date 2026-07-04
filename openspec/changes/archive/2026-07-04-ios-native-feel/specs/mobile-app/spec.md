# mobile-app 规范（增量）

## ADDED Requirements

### 需求:iOS 上采用平台自适应控件与转场

App **必须**在 **iOS** 目标上以接近系统的手感呈现少数高信号控件,同时 **Android 保持 Material、无回归**,
且**不改**配色 tokens、分层、确定性与任何交互契约(比例锁定、选项透传、裁剪几何)。**表现层**做控件/转场的
平台分支时**必须**用 `Theme.of(context).platform`(或 `.adaptive` 构造器内建的等价判断),**禁止**用 `dart:io`
`Platform.isIOS`——前者可被 widget 测试覆写、后者取宿主 OS 且测试不可控。**此禁令仅限表现层**:infrastructure
层按真实宿主 OS 选原生库/能力(如 `bead_ffi_loader` 用 `Platform.isIOS` 选原生库)**不受此限**。具体:所有
**开关**与**进度指示器**(设置页两开关、生成 loading、**裁剪屏读尺寸 loading**)**必须**用 `.adaptive`
(iOS→Cupertino、Android→Material);所有**分段控件**(设置页「生成模式」、裁剪比例菜单「纵/横」)在 iOS
**必须**用 `CupertinoSlidingSegmentedControl`(来自 `package:flutter/cupertino.dart`)、Android **必须**保留
`SegmentedButton`(按平台分支,二者取值语义一致);页面转场在 iOS **必须**为横滑 + 边缘滑动返回
(`CupertinoPageTransitionsBuilder`——`MaterialApp` iOS 默认即此,故默认已满足;**若**显式钉
`pageTransitionsTheme` 固化,**必须保留完整 builder map**含 Android 当前默认,**禁止**只写 iOS+Android 两项、
**禁止**用 `platform: TargetPlatform.iOS` 把 iOS 行为强加到 Android)。Cupertino 控件**必须**喂入 pegboard
tokens(激活色/thumb/背景取 `colorScheme`)以保品牌一致。骨架保留 `MaterialApp.router`(不换 `CupertinoApp`)。
未来若引入弹窗,**必须**用 `.adaptive` / `showAdaptiveDialog`(不得裸用 Material `AlertDialog`)。

#### 场景:iOS 呈现自适应/Cupertino 控件
- **当** 在 iOS(`Theme.of(context).platform == TargetPlatform.iOS`)呈现各屏
- **那么** 开关必须经 `SwitchListTile.adaptive` 呈现 iOS 自适应外观(`Switch.adaptive` 只保证 iOS 呈现,**不
  保证**具体 `CupertinoSwitch` widget 类型——故契约锁「自适应呈现 + 取值转发」的行为,不锁 widget 类型)、
  生成与裁剪读尺寸的 loading 必须是 iOS 菊花(经
  `.adaptive`)、两处分段(生成模式、纵/横)必须是 `CupertinoSlidingSegmentedControl`;页面切换为横滑且支持
  边缘滑动返回

#### 场景:Android 保持 Material 无回归
- **当** 在 Android(`TargetPlatform.android`)呈现同样的屏
- **那么** 开关/进度必须是 Material、两处分段必须是 `SegmentedButton`、转场保持 SDK 默认——与本变更前一致,无回归

#### 场景:iOS 分段的选值必须真的抵达引擎(不止渲染)
- **当** 在 iOS 分支点选 `CupertinoSlidingSegmentedControl`(如生成模式选「照片」)并生成
- **那么** 该选值(`generator`)**必须**原样抵达桥(经既有替身桥断言),验收**不得只测「iOS 渲染出 Cupertino
  分段」的存在性**——iOS 分段是一条新代码路径(`onValueChanged(T?)` → 回写),只测存在会漏掉转发接错,重蹈
  「死控件」覆辙

#### 场景:控件皮肤不影响交互契约
- **当** 用户在任一平台的自适应控件上操作(切生成模式/纵横、开关限色与去斑、调尺寸)
- **那么** 选项透传(设定值抵达桥)、尺寸比例锁定、裁剪比例、错误展示等既有契约**必须**保持不变——换的只是
  控件外观,取值语义与数据流不变

#### 场景:纯表现层,引擎与确定性不受影响
- **当** 实施本变更
- **那么** `bead-core`/`bead-ffi` 必须零改动,「CLI == FFI 逐字节」与确定性不受影响,配色 tokens 不变
