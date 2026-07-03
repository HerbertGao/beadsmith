## 为什么

Post-M9「Mobile UI Refinement」三工作流的最后一项(裁剪器升级、FFI 拓宽已完成)。
当前四屏(Home / Crop / 生成 / Result)只是 `ColorScheme.fromSeed(Colors.deepPurple)`
的默认 Material 皮——用户原话「过于简陋」。同时 `widen-ffi` 已开放的三项引擎选项
(`max_colors` / `despeckle` / `generator`)在 App 里仍是**死的**:`pattern_engine`
硬编码 `maxColors: null, despeckle: null, generator: staged`,用户无从调节
(ROADMAP 原话「Settings 控件在 FFI 落地前是死的」——现已落地)。

本变更把四屏按 *pegboard workshop* tokens 精细化重塑 + 深色模式,并把三项引擎选项
作为控件落到**设置页**(由生成页扩成)透传给引擎。

## 变更内容

- **设计系统**:把 pegboard tokens(accent `#6C4BF4` / secondary `#12A594` / ink
  `#1C1830` / ground `#F4F3F7` / line `#E6E3EF`)定义为 **light + dark** `ThemeData`
  (替换 `deepPurple` seed);圆润珠状控件;豆号/豆数用 mono 字体。设计参考 ROADMAP 的
  pitch mockup(`claude.ai/code/artifact/e80e77a4-…`)。
- **四屏重塑**:Home / Crop(`CropFrame` 全套主题化,取代此前「轻度套主题」)/ 设置 /
  Result 全部套 tokens。
- **设置页 = 扩展的生成页**(同一 `GeneratePage` / `/generate` 屏,非新屏):保留尺寸(比例锁定
  不变)并**新增三控件**——减色 `max_colors`、祛斑 `despeckle`(关 ⇒ `null`,非 `0`)、生成模式
  `generator`(`staged` / `gerstner`)。选项以**设置页本地组件状态**持有(与 `_width`/`_height` 同款,
  不引 session provider),`_generate()` **读出并转发**进 `GeneratePattern.call` → `PatternEngine.generate`
  → `ffi.generate`(**移除 `pattern_engine` 里硬编码的 `null / null / staged`**;沿途 Dart 形参加默认
  `null/null/staged` 的可选参数)。数值输入约束为可表示 `u32`(防 FRB 编码失败),但不做业务校验。
  (注:`generator = gerstner` 在豆数 > 裁剪源时会命中引擎 `target ≤ source` 守卫报错,经既有错误
  展示,不加预警功能。)
- **深色模式**:跟随系统(`themeMode: system`),light/dark 两套 tokens。
- **Result**:preview + stats/legend 仍**逐字取自 `GenerateOutput`**(硬规则,绝不从
  渲染图反推);mono 排豆号/豆数。

### 非目标

- 不改 `bead-core` / `bead-ffi`,**不改 Rust/`bead-ffi` 的 `generate` 签名与边界契约**(选项边界已由
  `widen-ffi` 开放);本变更只在 **Dart 壳侧**给 `PatternEngine.generate` / `GeneratePattern.call` 加
  默认 `null/null/staged` 的可选选项形参并转发(属壳内接线,非 FFI 契约变更)。不加新引擎算法;确定性不受影响。
- 不改裁剪器几何。
- 不做多语言与无障碍的**全面**改造(基本对比度/可点区达标即可,细化另议)。

## 功能 (Capabilities)

### 新增功能
<!-- 无新增 capability:本变更修改既有 mobile-app 契约。 -->

### 修改功能
- `mobile-app`(两条 **ADDED** 需求,既有需求均不改行为):
  - **新增**「生成参数(减色/祛斑/生成模式)可在设置页调节并透传」——三项引擎选项从硬编码
    默认改为用户可调、原样透传给 `generate`(不设即逐字段等价旧默认路径,CLI==FFI 仍成立)。
  - **新增**「四屏套用设计 tokens 且支持深色模式」。
  - 既有「目标尺寸由用户在生成页指定」的**尺寸/比例锁定契约不变**——生成页仅在实现层扩为
    「设置页」(尺寸 + 选项共处),该需求无需改。

## 影响

- **代码(仅 `apps/mobile`)**:
  - 新增 `lib/presentation/theme.dart`(pegboard light/dark `ThemeData` + tokens);
    `main.dart` 套 `theme` / `darkTheme` / `themeMode`。
  - 四屏 widget 重绘;`generate_page` 扩为设置页(尺寸 + `max_colors`/`despeckle`/
    `generator` 控件,选项以**本地组件状态**持有);`generate_page._generate()` 读出并转发选项;
    `pattern_engine.generate` 去硬编码、`GeneratePattern.call` 加可选选项形参并透传;新增壳级
    「设定值抵达桥」测试。
- **`bead-core` / `bead-ffi`**:零改动。
- **确定性**:选项是用户输入;引擎对同一(字节 + 选项)确定;三项未设时逐字段等价旧默认,
  「CLI == FFI 逐字节」对选中选项仍可测(`widen-ffi` 已保证)。
- **里程碑**:Post-M9 —— Four-screen restyle(第三工作流,收尾)。
