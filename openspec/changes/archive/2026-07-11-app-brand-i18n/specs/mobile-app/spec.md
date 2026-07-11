# mobile-app 规范增量

## 新增需求

### 需求:App 显示名为中文品牌名「拼豆匠」且随系统语言本地化
App 面向用户的**显示名** MUST 为中文「拼豆匠」/ 英文「Beadsmith」,**随系统语言切换**,覆盖两端所有可见落点:Android 启动器名（`android:label="@string/app_name"` + `res/values/strings.xml`=拼豆匠(默认桶)、`res/values-en/strings.xml`=Beadsmith）、iOS 显示名（`en.lproj`/`zh-Hans.lproj` 的 `InfoPlist.strings` 覆写 `CFBundleDisplayName`/`CFBundleName`,`Info.plist` 声明 `CFBundleLocalizations = [zh-Hans, en]`,且**无交集回落语言 = `zh-Hans`**（`Info.plist` 的 `CFBundleDevelopmentRegion` 直写 `zh-Hans` 去宏歧义 + pbxproj `developmentRegion=zh-Hans`）,并把 `InfoPlist.strings` 注册进 `Runner.xcodeproj`(`PBXVariantGroup` + `knownRegions` 补 `zh-Hans`)否则不进 `.app`）、以及 `MaterialApp` 应用标题（`onGenerateTitle`,见 Android 任务切换器）。**回落语义**（跨两端一致):偏好语言列表含 en → 英文;**既无 zh 又无 en** → 回落**中文**（Android 默认桶 = 中文、iOS `developmentRegion` = zh-Hans 对称保证）。**MUST NOT** 改动内部包名 `beadsmith`（`pubspec` `name:`）、Android `applicationId`（`com.beadsmith.beadsmith`）、iOS bundle identifier——那些是永久标识,改动等于换一个 app。

#### 场景:显示名随系统语言切换、两端一致
- **当** 系统语言为中文,查看 Android/iOS 桌面图标名、任务切换器
- **那么** 显示名为「拼豆匠」；系统语言为英文时为「Beadsmith」
- **且** 偏好语言列表既无 zh 又无 en(如 `[fr, de]`)时,两端均回落「拼豆匠」（Android 默认桶 + iOS developmentRegion=zh-Hans）
- **且** `applicationId` / bundle identifier / 内部包名保持不变

### 需求:界面文案经 gen-l10n 国际化（中英双语、默认中文），OS 对话框经 InfoPlist.strings 本地化
所有**用户可见**界面文案 MUST 支持**中文 zh 与英文 en**。**回落语义**:偏好语言列表含 en → 英文;**既无 zh 又无 en** → 回落**中文**（`preferred-supported-locales: [zh, en]` 把 `supportedLocales.first` 钉为 zh,`basicLocaleListResolution` 在「无任何匹配」时取首项;不能依赖 gen-l10n 默认的 ARB 文件名序,那会使无匹配时落 en）。注:偏好列表 `[fr, en]` 命中 en → 英文（尊重用户列出的英语,跨 Android/iOS/Flutter 一致）,**非**中文。**App 内**文案经 Flutter 官方 `gen-l10n`（ARB → `AppLocalizations`）提供,**MUST NOT** 在 Widget 里硬编码,改为 `AppLocalizations.of(context)` 键；带数量/尺寸的文案用 ARB `placeholders`/`plural`（不用字符串拼接）；`localizationsDelegates`/`supportedLocales` **必须用生成的聚合 getter**（禁手列,否则丢 `GlobalCupertinoLocalizations`）。**OS 对话框**文案（iOS 相册权限 `NSPhotoLibrary*UsageDescription`）不经 gen-l10n,MUST 经 `en.lproj`/`zh-Hans.lproj` 的 `InfoPlist.strings` 本地化。**例外**:`main()` 中在 `AppLocalizations` 建立之前运行的启动失败兜底屏（引擎/设置加载失败）**允许**保持硬编码中文；品牌名（调色板 `brand` 字段）**不翻译**。

#### 场景:切换系统语言，App 内文案与 OS 权限弹窗均随之变化
- **当** 系统语言为中文时打开各屏并触发相册权限
- **那么** App 内文案与相册权限弹窗均为中文；系统语言为英文时均为英文
- **且** 偏好语言列表既无 zh 又无 en 时回落中文；含 en 时为英文（跨端一致）

#### 场景:界面无残留硬编码中文（可测机制明确）
- **当** 以覆盖自检测试检查 `lib/presentation` 用户可见文案
- **那么** 无残留硬编码中文字面量——自检 MUST 用**可测机制**:关键屏 Widget 文案取自 `AppLocalizations`,或扫描时**先剥注释、只匹配字符串字面量**并 allowlist（品牌名/非 UI 串/已声明例外的兜底屏）；**禁**裸行级 CJK 扫描（会被中文注释假阳)

### 需求:App 具备自有启动图标（占位，自适应双层 + iOS 无 alpha）
App MUST 具备**自有启动图标**（不再用 Flutter 默认图标）,经 `flutter_launcher_icons` 生成:Android **自适应图标**（`adaptive_icon_foreground` 主体留安全区 + `adaptive_icon_background` 满幅底 → `mipmap-anydpi-v26/ic_launcher.xml`)与 iOS（`AppIcon.appiconset`,**无 alpha 通道**,`remove_alpha_ios`)。本阶段图标为**占位**（拼豆网格 motif,可用但非终稿）,上架前 MAY 用正式设计稿替换源图重生成,不改本需求。

#### 场景:两端有非默认自适应图标、iOS 无 alpha、构建通过
- **当** 生成图标并构建 Android / iOS 产物
- **那么** `mipmap-anydpi-v26/ic_launcher.xml` **存在**（证自适应已生成,非仅「mipmap 非空」——库中本有默认图标使「非空」恒真）、iOS `AppIcon.appiconset` 图标**无 alpha 通道**、构建通过
- **且** legacy `mipmap-hdpi/ic_launcher.png`（API<26 旧机）**≠ 库内 Flutter 默认图标**（经 base `image_path` 覆盖）——使「不再用 Flutter 默认图标」在全 API 段成立
