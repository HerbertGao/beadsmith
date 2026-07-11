## 决策

### D1:i18n 用 Flutter 官方 `gen-l10n`（ARB），不引第三方本地化库
文案键走 SDK 自带的 `flutter gen-l10n`：`generate: true` + `l10n.yaml` + `app_<locale>.arb`，
编译期生成 `AppLocalizations`。

- **替代方案:`easy_localization` / `slang` 等第三方库**。**否决理由**:gen-l10n 是平台原生
  能力,唯一新增运行时依赖是 `intl`(SDK 耦合、非第三方选型),类型安全键、编译期生成,与既有
  「精确 pin 依赖、少加库」的仓库风格一致。第三方库多为运行时 JSON + 动态 key,反而更重。
- **替代方案:只用 `intl` 手写 `Messages`**。**否决理由**:gen-l10n 就是 intl 的官方封装,
  省掉手写样板。
- **`intl` 必须声明为直依赖**:生成的 `app_localizations*.dart` 顶部 `import 'package:intl/intl.dart'`,
  `plural`/`placeholders` 必用;只加 `flutter_localizations` 会让 `intl` 停留在传递依赖 →
  `flutter_lints` 的 `depend_on_referenced_packages` 使 `flutter analyze` 报未声明包。故 pubspec 显式
  加 `intl`,但**不 exact-pin**(`intl: any` 或跟随 `flutter_localizations` 解析版本)——`flutter_localizations`
  对 intl 有 SDK 强约束,盲目精确 pin 会冲突。

### D2:回落语义——偏好列表含 en 则英文，否则回落中文（跨端一致）
支持 zh/en。**精确回落语义**（三端 Android/iOS/Flutter 一致）:用户偏好语言列表**含 en → 英文**
（尊重用户列出的英语);**既无 zh 又无 en → 回落中文**。这不是「主语言非中英就中文」(那需 override
三端默认行为),而是各端**原生默认行为**的自然一致结果——采纳它,零额外解析回调。

- **Flutter**:`basicLocaleListResolution` 遍历偏好列表找首个受支持 locale;`[fr, en]` 命中 en;
  `[fr, de]`(无匹配)取 `supportedLocales.first`。gen-l10n 默认按 ARB **文件名序**(`app_en` <
  `app_zh`)使 first=en → 无匹配落 en,违「默认中文」。故 `l10n.yaml` 加
  `preferred-supported-locales: [zh, en]` 把 first 钉为 zh。**`preferred-supported-locales` 只影响
  「无匹配→首项」这一档,不改变 `[fr, en]` 命中 en 的行为**(那是正确的,别误以为它会强制中文)。
- **Android**:资源解析同理——`values-fr`(无)→`values-en`(有)命中英文;`[fr, de]`→默认桶 `values/`=中文。
- **iOS**:见 D7——须 `developmentRegion = zh-Hans` 才使「无匹配」回落中文(否则回落 en)。

- **替代方案:主语言非中英即强制中文（`localeListResolutionCallback` 只看首选 locale）**。**否决理由**:
  会无视用户偏好列表里的英语(`[fr, en]` 用户懂英语却被塞中文),UX 更差,且要在三端各写覆盖逻辑;
  「含 en 即英文」是更友好且各端默认自洽的语义。
- **替代方案:默认 en**。**否决理由**:产品定位大陆用户,无匹配时首启应是中文;英文作可切换的第二语言。

### D3:两处启动失败 fallback 屏保持硬编码中文，不纳入 i18n
`main()` 在 `initBeadFfi()` / `SharedPreferences.getInstance()` 失败时,于 `ProviderScope`
与 `MaterialApp.router` 建立**之前** `runApp` 一个极简 `MaterialApp`（line 16/31）。此时无
`AppLocalizations` 上下文。

- **替代方案:给 fallback 屏也套 localizationsDelegates**。**否决理由**:为两条崩溃兜底文案
  单独搭一套本地化 delegate 是过度工程;这两屏是「引擎/设置加载失败」的极端边界,保持中文
  可读即可。标 `// ponytail:` 注明该例外。

### D4:显示名「拼豆匠」全本地化随系统语言，包名 / applicationId / bundleId 不动
显示名（中文拼豆匠 / 英文 Beadsmith）**随系统语言切换**,覆盖两端所有可见落点:Android 启动器名
（`android:label="@string/app_name"` + `values/`/`values-en/` strings.xml）、iOS 显示名
（`en.lproj`/`zh-Hans.lproj` 的 `InfoPlist.strings` + `Info.plist` `CFBundleLocalizations`）、
`MaterialApp` 标题（`onGenerateTitle`）。**不动** `pubspec name`/`applicationId`/`bundleId`。

- **替代方案:静态中文显示名(仅 App 标题本地化)**。**否决理由**:用户已选「全本地化」——英文机的
  桌面图标名/权限弹窗应显示英文,而非恒中文;静态方案会让「App 标题随语言」这句只在 Android 任务
  切换器成立、iOS 无落点,语义残缺。全本地化多几处原生资源但语义闭合、可测。
- **替代方案:连 `pubspec name` / `applicationId` 一起改**。**否决理由**:`pubspec name: beadsmith`
  是 Dart 包标识,改它要重写全部 `package:beadsmith/...` import;`applicationId`/`bundleId` 是永久
  商店主键,改动 = 新 app、丢失已有身份。定名只改用户可见显示名。

### D5:占位图标程序生成，接 `flutter_launcher_icons`（自适应双层 + iOS 无 alpha）
自绘拼豆网格 motif 占位图,`flutter_launcher_icons` 生成全尺寸。**须三份源图,不复用单张**:
① Android 自适应 `foreground`（主体居中 + 安全区,四周透明——否则满幅图被系统圆/方遮罩裁掉 motif）
② `background`（纯色 hex 或图）③ iOS 满幅**无 alpha** 图（`remove_alpha_ios: true`;iOS App Store
拒收带 alpha 图标,且本地 debug 构建不报、只在上架校验才炸）。iOS 满幅 vs Android 前景留白视觉互斥,
故不能一张通用。

- **替代方案:手工放各尺寸 PNG 到 `mipmap-*`/`AppIcon.appiconset`**。**否决理由**:尺寸多、易漏、
  易错;`flutter_launcher_icons` 是社区事实标准,配置生成全套(含 `mipmap-anydpi-v26` 自适应 XML)。
- **替代方案:单张满幅源图**。**否决理由**:自适应 foreground 需安全区留白、iOS 需满幅无 alpha,
  两者互斥,单图必有一端被裁或留白失衡。
- **占位定位**:图标是「可用占位」非终稿,上架前用正式设计稿替换源图重生成即可（非目标）。

### D6:附带产出「图标设计 prompt」(`ICON_PROMPT.md`),供文生图 AI 生成候选正式图标
除占位图外,额外产出一份可投喂文生图模型的 prompt,让用户生成多个候选来挑选终稿。**实现时
先与用户交互敲定视觉想法**(motif/配色/风格/留白/是否含字),再落笔;prompt **按平台分图层
描述清楚**——Android 自适应图标 `foreground`(主体+安全区)与 `background`(满幅底)分开、
iOS 满幅无 alpha 方形(系统加圆角)。

- **替代方案:直接一句话让 AI 出图**。**否决理由**:app 图标有硬性平台规范(自适应分层、安全
  区、无 alpha、别贴边),笼统 prompt 出的图往往不合规、被裁切或留白失衡;结构化 + 分图层的
  prompt 命中率高得多。
- **定位**:这是**设计辅助产物**,不是运行时能力,故只入 tasks/proposal,不设 spec 需求。

### D7:iOS 原生本地化面需显式配置（gen-l10n 只覆盖 App 内文案）
`gen-l10n`/`AppLocalizations` 只管 **App 内 Dart 渲染的文案**;iOS **原生面**（显示名、OS 权限弹窗、
locale 协商）不受其管,须显式配**四件**(缺一则本地化静默不生效或回落错语言):
- **`CFBundleLocalizations = [zh-Hans, en]`**:声明 App 本地化集。iOS 把用户偏好语言与此集求交后
  再报给 Flutter;缺则 App 内文案的 iOS locale 协商都可能错。
- **development region = `zh-Hans`**（现 en）:iOS 对偏好语言与本地化集**无交集**的设备(如 `[fr, de]`)
  回落 **development region**。设 en 则非中非英设备原生显示名/权限弹窗落**英文** + App 内文案也被顶成
  en(iOS 把上报 locale 替换成 en → Flutter 精确命中 en → `preferred-supported-locales` 永不触发)→
  破「无 zh/en 回落中文 / 两端一致」;设 zh-Hans 与 Android 默认桶(中文)对称,回落中文。
  **⚠ 去宏歧义**:`Info.plist:8` 的 `CFBundleDevelopmentRegion` 是宏 `$(DEVELOPMENT_LANGUAGE)`,由
  `DEVELOPMENT_LANGUAGE` **构建设置**(pbxproj 未设→Xcode 缺省 en)展开,**与 pbxproj `developmentRegion`
  属性(`:196`)相互独立、不自动同步**。故不能只改 `developmentRegion`——取**硬改**:`Info.plist` 的
  `CFBundleDevelopmentRegion` 直写 `zh-Hans`(产物值确定)+ 同步改 pbxproj `developmentRegion=zh-Hans`。
- **`InfoPlist.strings`(en/zh-Hans lproj)**:覆写 `CFBundleDisplayName`/`CFBundleName` 与两条相册
  权限文案(`NSPhotoLibrary(Add)UsageDescription`,现硬编码中文、用户可见)。base `Info.plist` 的这些
  键**保留**(App Store 期望键在 plist),**base `CFBundleDisplayName` 与 `CFBundleName` 均改中文
  「拼豆匠」**(与 dev region zh-Hans 一致,作 lproj 缺失时的中文兜底,兜底层不留英文/占位),lproj 再
  per-语言覆写。
- **注册进 Xcode 工程**:`project.pbxproj` 建 `PBXVariantGroup` 把 `InfoPlist.strings` 加入 Runner
  Resources build phase、`knownRegions` **补 `zh-Hans`**(现为 `(en, Base)`,`en` 已在、别写重复)。
  **光在磁盘建 `.lproj` 文件 ≠ 进 `.app`**——`flutter build ios` 按 pbxproj 打包,漏此步则本地化静默失效。

- **测试局限**:`Localizations.override` 是 widget 级,**测不出** iOS 原生 locale 协商/资源打包;验收须
  对**构建产物 `Runner.app`** 断言(`InfoPlist.strings` 确在 bundle 内)+ 真机/模拟器切系统语言观察,
  不能只断言源树在盘。

- **替代方案:只配 Flutter 侧 supportedLocales / 只建 lproj 不注册**。**否决理由**:iOS 原生面绕过
  Flutter、且资源须经 pbxproj 打包,漏配则「中文机看英文」「非中非英回落英文」「lproj 不进 bundle 静默
  失效」,与「显示名/文案随语言、无 zh/en 回落中文」需求直接矛盾。

## 确定性 / 引擎影响

零。本变更只在 `apps/mobile`：改 Android/iOS 配置、Dart 文案键、图标资源。不触任何 crate、
不改 `generate_pattern` 输出、不动 golden、CLI==FFI 契约不受影响（文案是纯展示层）。

## 验证

- `flutter gen-l10n` 生成成功、`flutter analyze` 无新增告警（含 `intl` 已声明,无
  `depend_on_referenced_packages`）、`flutter test` 全绿。
- i18n 覆盖自检（机制明确,非裸行级 CJK 扫描）:关键屏 Widget 文案取自 `AppLocalizations`,或扫描时
  剥注释 + 只匹配字符串字面量 + allowlist（品牌名/非 UI 串/兜底屏）,防「抽一半」又防注释假阳。
- 中英各跑关键屏（`Localizations.override`）+ **非中英 locale 回落中文**测试（守 `preferred-supported-locales`）。
- 图标:断言 `mipmap-anydpi-v26/ic_launcher.xml` 存在（自适应,非仅「非空」——库中已有默认图标使非空恒真）、
  iOS `AppIcon.appiconset` 无 alpha、构建通过。
- 原生本地化:plist/lproj 的 `CFBundleDisplayName`、`CFBundleLocalizations`、权限文案键值就位。
- 名称:构建产物 Android label / iOS CFBundleDisplayName == 「拼豆匠」。
