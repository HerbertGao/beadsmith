## 1. i18n 脚手架与依赖

- [x] 1.1 `apps/mobile/pubspec.yaml`：`dependencies` 加 `flutter_localizations: {sdk: flutter}`
  **与 `intl`**（gen-l10n 生成码顶部恒 `import 'package:intl/intl.dart'`,`plural`/`placeholders`
  必用;不声明为直依赖会触发 `flutter_lints` 的 `depend_on_referenced_packages` → `flutter analyze`
  报未声明包 → 阻断 6.1）。`intl` **不 exact-pin**：用 `intl: any`（或 `flutter pub deps` 查出
  `flutter_localizations` 解析版本再钉),pin 注释注明「SDK 耦合、跟随 flutter_localizations,勿盲目
  升级」。`flutter:` 段加 `generate: true`；`dev_dependencies` 加 `flutter_launcher_icons`（精确 pin）
- [x] 1.2 `apps/mobile/l10n.yaml`：`arb-dir: lib/l10n`、`template-arb-file: app_zh.arb`、
  `output-localization-file: app_localizations.dart`、`output-class: AppLocalizations`、
  `nullable-getter: false`、**`preferred-supported-locales: [zh, en]`**（gen-l10n 默认按 ARB 文件名
  排序 → `app_en.arb` 会排在 `app_zh.arb` 前,非中英系统会落 en;此键把 zh 钉为首选回落,兑现「默认中文」）
- [x] 1.3 `apps/mobile/lib/l10n/app_zh.arb` + `app_en.arb`：建骨架（`@@locale` + `appTitle`），
  `flutter gen-l10n` 跑通生成 `AppLocalizations`

## 2. 中文定名「拼豆匠」+ 原生本地化（全本地化：显示名/权限随系统语言）

- [x] 2.1 **Android 启动器名本地化**：`AndroidManifest.xml` `android:label="beadsmith"` →
  `android:label="@string/app_name"`；新增 `android/app/src/main/res/values/strings.xml`
  （`app_name` = **拼豆匠**,默认/非英文回落）+ `res/values-en/strings.xml`（`app_name` = **Beadsmith**）
- [x] 2.2 **iOS 显示名 + 权限文案本地化**（受影响文件含 **`ios/Runner.xcodeproj/project.pbxproj`**）：
  - `ios/Runner/Info.plist` 加 `CFBundleLocalizations = [zh-Hans, en]`；**把 iOS 无交集回落语言设为
    `zh-Hans`**——iOS 对偏好语言与声明本地化集**无交集**的设备(如法/德机)回落到 development region;
    保持 en 则原生显示名/权限弹窗落**英文**、破需求「无 zh/en 回落中文/两端一致」(Android 默认桶=中文,须对称)。
    **实操(去宏歧义)**:`Info.plist:8` 现为 `CFBundleDevelopmentRegion = $(DEVELOPMENT_LANGUAGE)`,该宏由
    `DEVELOPMENT_LANGUAGE` **构建设置**(pbxproj 未显式设 → Xcode 缺省 `en`)展开,**与 pbxproj
    `developmentRegion` 属性(`:196`,现 en)相互独立、不自动同步**;故**只改 `developmentRegion` 不够**。
    取**去歧义硬改**:`Info.plist` 的 `CFBundleDevelopmentRegion` 值由 `$(DEVELOPMENT_LANGUAGE)` **直接
    写死 `zh-Hans`**(无论宏跟谁,产物值都确定)+ 同步把 pbxproj `developmentRegion` 改 `zh-Hans`(工程一致)。
    「中文」= **简中 zh-Hans**(不含繁中 zh-Hant,如需另加)
  - 新增 `ios/Runner/en.lproj/InfoPlist.strings` 与 `zh-Hans.lproj/InfoPlist.strings`,各含
    `CFBundleDisplayName`(Beadsmith / 拼豆匠)、`CFBundleName`(同步本地化,别只本地化 DisplayName)、
    `NSPhotoLibraryAddUsageDescription`、`NSPhotoLibraryUsageDescription`(现 `Info.plist:29/31` 硬编码
    中文相册权限文案 → 两语各一份)。base `Info.plist` 的这几个键**保留**(App Store 期望键在 plist 出现);
    **base `CFBundleDisplayName` 与 `CFBundleName` 值均改中文「拼豆匠」**（现 `CFBundleName=beadsmith` 小写占位）
    ——dev region=zh-Hans 时最终兜底应中文(lproj 缺失时也不落英文/占位),lproj 再 per-语言覆写
  - **把两份 `InfoPlist.strings` 注册进 Xcode 工程**:`project.pbxproj` 建 `PBXVariantGroup` 并加入
    Runner 的 Resources build phase、`knownRegions` **补 `zh-Hans`**(`en` 已在 `(en, Base)` 中,别写重复)
    ——**光在磁盘建 `.lproj` 文件不会被打进 `.app`**(`flutter build ios` 按 pbxproj 打包),漏此步则本地化
    **静默不生效**(可用 Xcode「Add localization」流程自动写)
- [x] 2.3 `apps/mobile/lib/main.dart`：`MaterialApp.router` 的 `title: 'Beadsmith'` 改为
  `onGenerateTitle: (ctx) => AppLocalizations.of(ctx).appTitle`（Android 任务切换器标题随语言）

## 3. i18n 接线 App 根

- [x] 3.1 `apps/mobile/lib/main.dart`：`MaterialApp.router` 加 `localizationsDelegates:
  AppLocalizations.localizationsDelegates`、`supportedLocales: AppLocalizations.supportedLocales`
  ——**必须用生成的聚合 getter,禁止手列 delegates**（手列易漏 `GlobalCupertinoLocalizations`,
  而 App 用 `CupertinoSlidingSegmentedControl` 等 Cupertino 控件,漏了会抛「No CupertinoLocalizations found」）
- [x] 3.2 两处启动失败 fallback 屏（`main.dart:16/31`）保持硬编码中文,加 `// ponytail:` 注明
  「pre-l10n 崩溃兜底屏,不纳入本地化」（设计 D3）

## 4. 抽取硬编码中文 → ARB 键（真·用户可见 UI 串 ≈50 条 / 5 文件；注释不抽）

> 计数口径：仅 Dart **字符串字面量**里的用户可见文案（不同人数法 49~52,**近似值,实现时以甄别为准、
> 不作硬阈值断言**——6.2 自检查「无残留」而非「恰 N 条」)。中文 `///`/`//` 注释不算（本仓 presentation
> 层中文注释多,勿按注释去「抽」出不存在的键）。

- [x] 4.1 `presentation/generate_page.dart`（~20 条）→ ARB 键 + `AppLocalizations.of(context)`
- [x] 4.2 `presentation/result_page.dart`（~14 条）
- [x] 4.3 `presentation/crop_frame.dart`（~6）+ `presentation/crop_page.dart`（~6）
- [x] 4.4 `presentation/home_page.dart`（~3）
- [x] 4.5 **不抽**：`bead_grid_view.dart`、`application/providers.dart`、
  `infrastructure/palette_registry.dart`、`presentation/crop_geometry.dart` 的中文**均为 `///`/`//`
  注释**,无可抽 UI 串;`palette_registry` 的 `brand` 是品牌名（**不译**）。抽取前甄别:非 UI 串
  （日志/key/断言/注释）不进 ARB
- [x] 4.6 带数量/尺寸的文案用 ARB `placeholders`/`plural`（如「N 色」「N×M 格」),不用字符串拼接

## 5. 占位启动图标（自适应双层 + iOS 无 alpha）

- [x] 5.1 生成**三份**占位源图（拼豆网格 motif）到 `apps/mobile/assets/icon/`：① `ic_foreground.png`
  （Android 自适应前景:主体居中,**留安全区**——内容限中心 ~66%、四周透明,防系统圆/方遮罩裁掉 motif）
  ② 背景为纯色 hex（`adaptive_icon_background`,或 `ic_background.png`）③ `app_icon_ios.png`
  （iOS 满幅、**无 alpha**、主体别贴边）。**不复用单张满幅图**（iOS 满幅 vs Android 前景留白视觉互斥）
- [x] 5.2 `pubspec.yaml` 配 `flutter_launcher_icons`：**`image_path`（base 满幅方图,供 Android
  legacy `mipmap-*/ic_launcher.png` 覆盖 API<26 旧机 + iOS 兜底）**、`adaptive_icon_foreground`、
  `adaptive_icon_background`、`image_path_ios`（iOS 满幅无 alpha 图）、**`remove_alpha_ios: true`**
  （iOS App Store 拒收带 alpha 图标,本地 debug 构建不报,须显式去 alpha）。**只配 `adaptive_icon_*`
  不给 base `image_path` → 仅生成 anydpi-v26 自适应、legacy mipmap 仍是旧默认图（API<26 旧机不换图,
  或工具报缺 image_path）**;跑 `dart run flutter_launcher_icons` 生成 `mipmap-*`（legacy）+
  `mipmap-anydpi-v26/ic_launcher.xml`（自适应）/ `AppIcon.appiconset` 全尺寸
- [x] 5.3 源图 + 生成产物纳入版本控制；源图仅 build-time 供 flutter_launcher_icons,**不**列入
  `flutter: assets`（不触发既有 `palette_assets_test` 的严格 asset 断言）
- [x] 5.4 **产出「图标设计 prompt」**（供用户喂文生图 AI 生成候选**正式**图标,占位图之外的独立
  产物）→ `apps/mobile/assets/icon/ICON_PROMPT.md`。**实现时必须先与用户交互敲定视觉想法**
  （motif / 配色 / 风格基调 / 留白 / 是否含品牌字）,再落笔。prompt 须**分图层描述清楚**:
  ① Android 自适应——`foreground`（主体居中 + 安全区,四周透明）与 `background`（满幅纯色/简纹）
  **各自内容分开写**;② iOS——**无 alpha、满幅正方形、系统加圆角**（主体别贴边）。输出可直投文生图
  模型的完整 prompt（英文主 + 中文注释）,含正/负向提示、尺寸约束

## 6. 验证

- [x] 6.1 `flutter gen-l10n` 生成成功、`flutter analyze` **无新增告警**（含 `intl` 直依赖已声明,
  无 `depend_on_referenced_packages`）、`flutter test` 全绿
- [x] 6.2 i18n 覆盖自检测试（`apps/mobile/test/i18n_coverage_test.dart`）——**机制明确**,二选一:
  (a) 对关键屏 Widget 断言其文案取自 `AppLocalizations`（推荐,少假阳);或 (b) 扫 `lib/presentation`
  时**先剥离** `//`/`///`/`/* */` 注释、**只匹配字符串字面量内**的中文,并 allowlist（品牌名、
  `Key('...')`/`assert`/`debugPrint` 等非 UI 串、已声明例外的 fallback 屏）。**禁用**裸行级 `[一-龥]`
  扫描(会被中文注释永久假阳)
- [x] 6.3 语言切换 widget test：`Localizations.override(locale: en)` 断言英文、`zh` 断言中文；
  **回落语义测试(用 `supportedLocales` 解析而非仅单 locale override)**:偏好列表 `[fr, de]`（无 zh/en）
  → 解析中文(守 `preferred-supported-locales`);`[fr, en]` → **英文**（en 在列表即命中,**非**中文——
  这是跨平台一致的正确行为,见 spec 需求2 回落语义）。**注**:widget 层 locale 解析测不到 iOS 原生
  locale 协商(见 6.4)
- [x] 6.4 构建冒烟 + **构建产物**断言(非源树):`flutter build apk --debug` / iOS assemble 通过;
  断言 `mipmap-anydpi-v26/ic_launcher.xml` **存在**（证自适应,非仅「mipmap 非空」恒真）+ legacy
  `mipmap-hdpi/ic_launcher.png` 哈希 ≠ 库内默认（证 API<26 也换图）;**iOS 断言对 `Runner.app` 产物**:
  `en.lproj/InfoPlist.strings` 与 `zh-Hans.lproj/InfoPlist.strings` **确在 `.app` 内**（证已注册进
  pbxproj、非仅源树在盘）、`AppIcon.appiconset` **无 alpha**、`CFBundleDevelopmentRegion==zh-Hans`;
  **iOS 原生回落须真机/模拟器切非中非英系统语言观察显示名/权限弹窗**（widget test 不作此场景通过依据）
- [x] 6.5 **与引擎无关对账**：确认零改 `crates/`、零改 golden、`cargo test` 仍全绿（防误触）
