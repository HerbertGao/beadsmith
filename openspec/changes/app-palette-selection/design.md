## 上下文

`apps/mobile` 当前把 `artkal_s` 硬编码为唯一调色板:`providers.dart` 的 `paletteJsonProvider`
是一个**无参** `FutureProvider`,固定 `rootBundle.loadString('assets/palettes/artkal_s.json')`;
`generate_page._generate` 读它、传给引擎;`result_page` 又从派生的 `paletteProvider` 取解析后的
色表来渲染格子/配色面板。设置页的三项引擎选项(`generator` / `max_colors` / `despeckle`)以
`_GeneratePageState` 的局部 `setState` 字段持有,不跨启动。

M10 已在引擎侧铺好 14 个 MIT-clean 色卡(顶层 `palettes/*.json`),另有 4 个 AGPL 的国产牌
隔离在 `palettes/_unlicensed/`。引擎/CLI/FFI 早已参数化色卡(`generate(paletteJson)` /
`bead-cli --palette`),故本变更**纯 App 侧**:打包多色卡 + 选择 UI + 持久化 + 结果钉色卡,
`bead-core`/`bead-ffi` 零改动,确定性不受影响。

约束:守 ARCHITECTURE 硬规则(壳不含算法、统计/预览只从 `GenerateOutput`/`BeadPattern` 派生);
保持既有「设置页锁比例」「选项透传到桥」「CLI==FFI 逐字节」契约不回退。

## 目标 / 非目标

**目标:**
- 用户能在设置页从 14 个内置色卡中选一个,后续生成按所选色卡出图纸/色号。
- 色卡与设置页其它配置跨启动记住。
- 生成后改色卡不污染已有结果(颜色/色号/计数)。

**非目标:**
- 色块缩略预览、色卡搜索/分组导航(v1 YAGNI)。
- 接入 `_unlicensed/` 的 4 个国产牌(AGPL,须实测重采后另案)。
- CLI `palette list`、收藏/历史(M10 其它线)。
- 任何 `bead-core`/`bead-ffi`/CLI 改动。

## 决策

### D1 — 选色卡入口放在设置页,作为一行

色卡是「生成参数」,与尺寸/生成模式/限色/去斑同属生成前设定,复用既有 `_optionControls` 模式,
新增面最小。
- 替代:放**首页**(选图前先选牌)—— 否决:用户往往先想选图,色卡前置打断主流程。
- 替代:放**新建全局设置页**—— 否决:要新建整屏 + 导航入口,且色卡是高频切换项,藏进设置反而绕。

### D2 — 入口形式为底部弹窗(bottom sheet)列表

点色卡行弹底部弹窗,每项显示品牌名 + 「N 色」,选中打勾。比原生下拉好看、可承载副信息与未来
色块预览,又比整页路由轻。
- 替代:`DropdownButton`—— 否决:14 项原生菜单又长又难带副信息。
- 替代:整页路由 `/palette`—— 否决:14 项用不着整页,多一次跳转反而繁;需要搜索/分组时再升级。
- **平台呈现 = 普通 Material `showModalBottomSheet`(两端一致,不写「.adaptive」)**:Flutter **无** adaptive
  bottom-sheet 构造器,「.adaptive 底部弹窗」是不存在的 API。既有「iOS 平台自适应控件」需求的自适应枚举是
  **开关/进度/分段**,其弹窗条款针对**对话框**(`showAdaptiveDialog`)——底部弹窗不在其列;且
  `crop_frame.dart`/`result_page.dart` 已用普通 Material `showModalBottomSheet` 作先例。故 Material sheet
  两端合规、不违反该需求(可选 `showCupertinoModalPopup` 求更 iOS 味,非必需)。

### D3 — 默认色卡为 MARD

App 全中文、面向大陆用户,MARD 为「大陆最实用、图纸采购常用」的入门牌(M10 优先级第一)。
经核实所有 App 单元/集成测试都按**路径**显式读 `artkal_s` 当固定 fixture 或直接 override
`paletteJsonProvider`,**无一依赖「默认选中」是谁**,故改默认不碰测试。
- 替代:保持 `artkal_s`—— 有真实英文色名、面板更友好,但偏专业玩家向,非大陆主流入门。
- 替代:首次强制选一次—— 最贴合「用什么豆选什么卡」,但加首次摩擦;默认可改已够。
- 代价:MARD `name == code`,配色面板显示「A01 A01」略冗余(可后续优化成只显 code)。

### D4 — 持久化经 shared_preferences,设置提升为持久化 Notifier 模型

新增 `shared_preferences`(Flutter 标准小依赖),把设置页配置从局部 `setState` 提升成一个
`GenerateSettings` 值对象 + 由 `shared_preferences` 支撑的 Riverpod `Notifier`,改动即写、启动即读。
- 替代:仅会话内(零依赖)—— 否决:选色卡的意义就是记住你的豆,每次启动打回默认把功能架空。
- 替代:等 M10 收藏 DB(sqlite/hive/isar)—— 否决:把本功能耦合到一个更大且未定的决策上,推迟上线;
  KV 持久化用不着关系型/文档 DB。
- 依赖理由:Flutter 无内建跨启动 KV;`shared_preferences` 是官方维护的最小实现,仅 App 侧、不入 crate。
- **首帧同步就绪(防竞态,必须)**:`main()` 中 `await SharedPreferences.getInstance()`,经 provider
  override 注入,使设置 Notifier 首帧即可**同步**读到持久值。否则「就绪前用默认→就绪后覆盖」会引出
  两个竞态:①用户在 prefs 到达前点生成 → 落到**默认**色卡/尺寸(违背「重启恢复改后值」场景);
  ②晚到的持久值**冲掉**用户已改的编辑。同步就绪后,`GeneratePage.initState` 单次用持久宽播种 +
  单次经锁比例推高,不做二次异步改写。

### D5 — 只持久化「宽维度豆数(`width`,非长边)」,高按当次裁剪比例重推

持久化的是**宽维度**(`width`,水平豆数)——**不是「长边」**:竖构图裁剪(9:16 等)下宽是短边,
若误存 `max(w,h)` 会破坏「存宽、推高」模型。宽(网格密度)是稳定偏好值得记;高与裁剪比例耦合,记
绝对值换比例时对不上,故只存宽,进设置页按当次 `cropAspectProvider` 走既有 `lockedGridPair` 推高。
- **越界沿用既有逻辑**:持久宽在新的更瘦长比例下若使配对超 1000,`lockedGridPair` 会整体等比缩小
  (宽不再恰等于持久值)。这是既有越界行为,规范场景须如实表述为「宽恢复为持久值,**越界时整体等比
  缩小**」,不写成绝对的「宽必恢复持久值」(否则与该重算行为自相矛盾)。
- 替代:宽高都记,套 aspect-lock 自动 rebase—— 否决:换比例时 pair 被重算,与用户记忆不符。
- 替代:尺寸都不记—— 否决:丢掉「常用 100 豆」这类真实偏好。
- 替代:存「长边豆数」或总豆数—— 否决:与「存宽、按比例推高」模型不自洽(需先知方向才能反推),徒增歧义。

### D6 — 生成结果钉住「生成时的色卡」

`ResultPage` 现读实时 `paletteProvider`;一旦色卡可变,「生成后改色卡」会用新色卡套旧 `cells`
索引 → 颜色/色号/计数全错。**修法**:把 `_generate` 实际传给 `generate` 的 `paletteJson` 本身
(不重读)与 `GenerateOutput` 一起存进结果(Dart 侧包装 `{output, paletteJson}`),`ResultPage`
读钉住的那份解析。因 `preview.png`/`cells`/`stats` 皆由引擎在生成时用同一份色卡产出,钉住它即
保证预览↔格子↔色号↔面板逐环一致,且 == 用户点生成当刻所选。
- 替代:结果页照读实时 provider,改色卡时作废当前结果—— 否决:体验是「改个色卡结果没了」。
- 替代:把色卡塞进 FFI `GenerateOutput` 类型—— 否决:那是 FFI 生成的类型,不该为 UI 关注改桥;
  Dart 侧包装更合适。
- 不改既有「结果页格子视图」需求:它本就要求从「已解析的 palette(load_palette 保序)」渲染,
  钉住的色卡正是这样一份,二者相容,仅澄清来源。
- **清理孤儿**:`paletteProvider`(`providers.dart:42`)钉住后唯一消费者(`result_page.dart:30`)消失 →
  死代码,须删除;`ResultPage` 改为**同步**解析钉住的 `paletteJson`(去 `.when` loading/error 分支,
  钉住 json 随结果就绪、无异步)。`generateResultProvider` 的 Notifier 类型由 `GenerateOutput?` 改为
  包装体 `{output, paletteJson}?`。

### D7 — 色卡集合用显式注册表,不自动扫描

在 Dart 里维护一个显式注册表(`{id, brand 展示名, asset 路径}`,固定顺序 MARD → Artkal S/A/C/M/R →
Hama → Perler → Nabbi → Yant),换排序/加牌只改这一处。
- 替代:运行时读 `AssetManifest` 自动发现 `assets/palettes/*.json`—— 否决:失去顺序控制,且展示
  标签/顺序仍要另一套配置;14 个静态项显式列更省心。
- **`brand` 展示名入注册表(须同步可得)**:`palette_codec.parsePalette` 不产出 `brand`,而设置页色卡行
  首帧就要**同步**显示当前品牌名——靠注册表直接给,无异步空窗;若靠解析 JSON,则解析前行文案未定义。
- **仅「N 色」惰性解析**各 JSON 得到(14 个小文件),避免注册表硬编码色数漂移;注册表 `brand` 须与
  对应 JSON `brand` 字段一致(测试断言之)。解析失败时「N 色」回落非崩溃占位(如「—」),弹窗不崩溃。

### D8 — 打包 14 份 clean JSON 进 App assets,显式列举、排除 _unlicensed

把顶层 `palettes/` 的 14 个 clean 色卡(已有 `artkal_s` + 新 13 份)**逐字节拷贝**进
`apps/mobile/assets/palettes/`。**`pubspec.yaml` 逐个显式列举这 14 份 asset,不用 `assets/palettes/`
目录通配**——目录通配会把该目录下**任何**文件(含误拷入的 `_unlicensed` AGPL 数据)一并打包,而只做
「14 份 clean 存在且字节一致」的正向校验**抓不到多出来的文件**,AGPL 数据会静默进店包。显式列举使打包集
恰为这 14 份。
- 拷贝而非引用顶层:Flutter asset 只能取 App 包内路径,不能引用仓库上层目录;现有做法即 App 内自带
  一份 `artkal_s` 拷贝。
- **校验必须是可运行的 `flutter test`**(而非「测试或脚本」这类含糊、可能没人调的脚本——未接线的脚本
  什么都不保证),断言:① `apps/mobile/assets/palettes/` 内文件集合**恰等于** 14 个注册表 id(**双向**:
  无缺、无多余、无 `_unlicensed` 路径);② 每份与顶层 `palettes/<name>.json` **逐字节相同**(守 CLI==FFI)。
  纳入 `flutter test`,漂移与 AGPL 多余文件都在 CI 挡住。

## 风险 / 权衡

- **[App 内色卡拷贝与顶层漂移 / AGPL 多余文件]** → 一个纳入 `flutter test` 的校验:`assets/palettes/` 文件集
  **恰等于** 14 注册表 id(双向,挡多余/`_unlicensed`)+ 每份与顶层逐字节相同(见 D8)。新增/改色卡时两处同步。
- **[shared_preferences 首帧异步]** → **已解**:`main()` 预载 `getInstance()` 并 provider override 注入,
  设置 Notifier 首帧同步读持久值;不用「先默认后覆盖」,消除「生成落 gap 用默认」与「晚到值冲掉编辑」两竞态(见 D4)。
- **[宽维度 vs 长边 / 越界重算]** → 持久化的是 `width`(非长边);持久宽在更瘦长比例下越界时 `lockedGridPair`
  整体等比缩小,宽不再恰等持久值——规范场景须如实写「越界时整体等比缩小」,不写绝对「宽必恢复」(见 D5)。
- **[MARD 默认色名冗余]** → 面板显示「A01 A01」;本变更接受(用户已知此权衡下仍选 MARD 默认),后续可让面板在
  `name == code` 时只显 code。**accepted-degraded**:属显式请求的默认色卡行为,非正确性缺陷。
- **[持久 id 失效]** → 若未来移除某色卡,旧持久 id 找不到 → 明确回落默认 MARD,不崩溃(已入规范)。
- **[widget 测试需 mock prefs]** → 选项一旦源自 prefs-backed Notifier,未 `SharedPreferences.setMockInitialValues`
  的 `GeneratePage` widget 测试会抛 `MissingPluginException`;测试任务须注明该 mock-init 前置(见 tasks 6)。
- **[持久化 vs 逐字节闸门]** → 持久化只改「值从哪来」,不改透传语义;三项未设默认路径仍逐字节等价旧输出,
  「CLI==FFI」不回退。

## 迁移计划

纯增量、可回滚,无数据迁移:
1. 拷 13 份 clean JSON 进 `apps/mobile/assets/palettes/`;`pubspec.yaml` **逐个显式列举** 14 份
   `assets/palettes/<name>.json`(**不**声明 `assets/palettes/` 目录,见 D8)+ 加 `shared_preferences`。
2. 引入 `GenerateSettings` 模型 + 持久化 Notifier;`paletteJsonProvider` 改为随选中 id 解析。
3. 设置页加色卡行 + 底部弹窗;三项选项与宽改读写持久化模型。
4. 结果包装携带 `paletteJson`,`ResultPage` 改读钉住色卡。
5. 加 App 内/顶层色卡字节一致性校验。

回滚:还原上述 App 侧改动即可,引擎/CLI/FFI 无涉。

## 待解问题

- 无阻断性问题。色块预览、`name == code` 面板优化、CLI `palette list` 均为本变更**非目标**,列为后续增强。
- **[范围外·仅记录]** 未改文的「目标尺寸由用户在生成页指定」需求括注仍写「当前 40×40 默认」,
  `session_providers.dart:29` 亦然,而代码与本变更默认均为 100——此陈旧值属既有文档瑕疵,**不在本变更
  范围**,留作单独清理;本变更不改该需求文本(避免扩面)。
