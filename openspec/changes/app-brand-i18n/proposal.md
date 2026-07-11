## 为什么

App 目前是「离线可用」但不「像成品」:显示名是占位 `beadsmith`(英文、非品牌）、
界面文案约 63 条中文**硬编码**散在 9 个 `.dart` 文件里(无法切换语言、也无法上架多区）、
且**无自有图标**(用 Flutter 默认图标）。这是 M10「中文定名 + i18n + 图标」工作线,
也是上架(第 4 条)的最低「像成品」前置——名字、图标、可切换语言是商店页与首启观感的
基本盘。

本变更**只动 `apps/mobile`**,不碰任何 crate(`bead-core`/`bead-cli`/`bead-ffi`)、
不改引擎输出、不动确定性、不动 golden。

## 变更内容

- **中文定名「拼豆匠」+ 全本地化显示名(随系统语言)**:显示名中文「拼豆匠」/ 英文「Beadsmith」
  随系统语言切换,覆盖两端所有可见落点——Android 启动器名(`android:label="@string/app_name"` +
  `values/`=拼豆匠、`values-en/`=Beadsmith)、iOS 显示名(`en.lproj`/`zh-Hans.lproj` 的
  `InfoPlist.strings` 覆写 `CFBundleDisplayName`/`CFBundleName` + `Info.plist` `CFBundleLocalizations`
  + **`CFBundleDevelopmentRegion`=`zh-Hans`**(令非中非英设备原生回落中文,对称 Android 默认桶)+
  **把 `InfoPlist.strings` 注册进 `Runner.xcodeproj`**(PBXVariantGroup/knownRegions,否则不进 `.app`))、
  `MaterialApp` 标题(`onGenerateTitle`)。**回落语义**(跨端一致):偏好列表含 en→英文,无 zh/en→中文。
  **不动**内部包名 `beadsmith`、`applicationId`、iOS bundleId——改它们等于换一个 app。
- **界面文案 i18n(Flutter 官方 `gen-l10n`,中英双语,默认中文)**:引入 `flutter_localizations`
  (SDK 自带)**与 `intl`**(gen-l10n 生成码 import `package:intl`,须声明为直依赖否则 analyze
  报未声明包;**不 exact-pin**,SDK 耦合)、`generate: true` + `l10n.yaml`(含
  `preferred-supported-locales: [zh, en]` 把 zh 钉为首选回落,否则默认 ARB 文件名序会落 en);
  新增 `app_zh.arb`/`app_en.arb`;把 **5 个文件约 49 条**用户可见中文抽成 `AppLocalizations.of(context)`
  键(注释不抽);`MaterialApp.router` 接**生成的聚合** `localizationsDelegates`(禁手列,含
  `GlobalCupertinoLocalizations`)+ `supportedLocales`。**OS 权限弹窗**(iOS 相册权限)经
  `InfoPlist.strings` 本地化(gen-l10n 管不到)。**例外**:`main()` 两处**启动失败 fallback 屏**
  (line 16/31)在 `AppLocalizations` 建立之前运行,保持硬编码中文(设计 D3)。
- **占位启动图标(拼豆网格概念,自适应双层 + iOS 无 alpha)**:生成**三份**占位源图——Android 自适应
  `foreground`(主体留安全区)+ `background`(纯色)、iOS 满幅**无 alpha** 图;引入
  `flutter_launcher_icons`(dev 依赖,配 `adaptive_icon_*` + `remove_alpha_ios`)生成 Android
  (`mipmap-*` + `mipmap-anydpi-v26/ic_launcher.xml` 自适应)/iOS(`AppIcon.appiconset`)全尺寸。
  占位性质:**上架前须换正式设计稿**(见非目标)。
- **附:图标设计 prompt 产物**:额外产出 `assets/icon/ICON_PROMPT.md`——一份可投喂文生图 AI
  的完整 prompt,供用户生成候选**正式**图标来挑选。实现时**先与用户交互敲定视觉想法**,并按
  平台规范**分图层描述**(Android 自适应 foreground/background、iOS 满幅无 alpha)。这是设计
  辅助产物,不改「App 有自有图标」这一运行时需求。

## 非目标

- **不做正式图标设计**:本次只出**占位**图标(可用但非终稿);正式视觉设计留到有设计稿时
  单独替换(flutter_launcher_icons 源图一换即可重生成)。
- **不加中英以外语言**:先中英双语(roadmap「至少中英双语」),其它语种后续增 ARB 即可。
- **不改包名 / applicationId / bundleId**:那是永久商店标识,本次只改用户可见显示名。
- **不做商店元数据 / 截图 / 隐私声明 / 签名上传**:那是 M10 第 4 条「上架」工作线。
- **不碰引擎 / CLI / FFI / 确定性 / golden**:纯 `apps/mobile` 前端改动。

## 功能 (Capabilities)

### 新增功能

- **mobile-app**:`App 显示名为中文品牌名「拼豆匠」`
- **mobile-app**:`界面文案经 gen-l10n 国际化(中英双语、默认中文)`
- **mobile-app**:`App 具备自有启动图标(占位,拼豆网格)`

### 修改功能

<!-- 无:三项均为对既有 mobile-app 的增量,不改既有需求语义 -->
