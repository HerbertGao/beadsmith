## 1. 共享几何/布局（预览与保存图共用，防漂移）

- [x] 1.1 新增**纯几何/布局函数**（如 `apps/mobile/lib/presentation/bead_grid_layout.dart`）:入
  `width W / height H / borderRings k / cellPx`（**尺度参数**;**不入 `cells`/`palette`**——那归 draw 适配器),出
  {刻度 margin(顶/左,用 `cellSize` 的**实数比例**、勿整数截断)、板面逻辑区 `(W+2k)×(H+2k)`、内容格 rect（内容偏移
  = 刻度 margin + k 圈）、细网格线位置、**每 10 格加粗线位置**、**刻度标号位置与值**（每 10 边界;`W<10`/`H<10` 时
  该轴无刻度）、边框区、**`canvasAspect`（含 margin,尺度无关)**}。**无副作用、不索引 `cells`**（边框区为合成留白)。
  **尺度无关产出**（`canvasAspect`/标号值/cell-unit 或归一化位置/`k`)是防漂移断言锚;`margin`/`cellSize` 等**像素绝对
  量随 `cellPx`**,各适配器传自身 `cellPx`(预览 fit / 保存预算)各自换算
- [x] 1.2 `apps/mobile/lib/presentation/bead_grid_view.dart`：`CustomPainter` 改用 1.1 的布局 + `ui.Canvas`
  draw 适配器画内容/细线/**每 10 加粗**/**1-based 刻度(TextPainter)**/**k 圈空留白边框**;`BeadGridView` 接
  `borderRings k`（**默认 0**——现有 test 与 `result_page:299` 调用点无 k）。**关键(B3)**:`gridAspect`(:161)
  改读 **`layout.canvasAspect`（含 margin）**(**非** `(W+2k)/(H+2k)`);命中(:145-146)`gridRect` 语义定为**含
  margin 整画布**,`content=((screen−gridRect.topLeft)−layout.marginTopLeft)/layout.cellSize` 再 `−k`,越界
  **no-op**(不回调 `onCellTap`)——此处 `layout` 用**预览 fit 后 cellPx** 调用(`marginTopLeft`/`cellSize` 与屏上
  `gridRect` 同尺),分母是**板面内 `cellSize`**、不是 `gridRect.width/(W+2k)`、也非跨适配器共享绝对值;`shouldRepaint`(:294)
  须加 `old.borderRings!=borderRings`,高亮环 loop(:281)只环内容格(边框无珠);detail sheet 维持 1-based
  (`k>0` 传内容相对 `(row,col)` 再 +1,勿含边框偏移)

## 2. 边框圈 k 配置（默认持久化 + 结果页实时改）

- [x] 2.1 `apps/mobile/lib/application/generate_settings.dart`：`GenerateSettings` 加 `borderRings`
  （int，**默认 0**，硬上限 **8**）+ 持久化——须改**约 10 处镜像**：**字段声明** `final int borderRings`、
  ctor、defaults(0)、copyWith、`==`、`hashCode`、`setBorderRings(int)` setter、**const key `_kBorderRings`**
  （load-bearing:漏了静默不持久化)、build()、`_persist`;**build() 读时 clamp**:`(p.getInt(_kBorderRings) ?? 0).clamp(0, 8)`
- [x] 2.2 `openspec/specs` 侧「设置页配置跨启动持久化」需求已 MODIFIED 纳入 `borderRings`——实现须让
  `settings_persistence_test.dart` 的 `read == defaults` 断言在加 `borderRings:0` 后仍成立;**并补一条反空
  round-trip 用例**:`setBorderRings(3)` → 杀重开 → 读回 3(照既有 anti-vacuous 纪律)
- [x] 2.3 生成/设置页（`generate_page.dart`）加「边框圈」默认值控件（步进器 0..上限;文案走 gen-l10n）
- [x] 2.4 `apps/mobile/lib/presentation/result_page.dart`：**把 `borderRings` 状态提到 `ResultPage` 级**——
  `ResultPage` 现为无状态 `ConsumerWidget`(:25),`_ResultAppBar`(:51)与 `_ResultBody` 是**兄弟**(statefulify 无法
  跨兄弟共享)——共享同一份 `k` **须经 Riverpod `StateProvider.autoDispose`**;AppBar 保存键、首保浮条(:336)、预览、
  headless 存图**都从同一 provider 读 k**;**provider MUST `autoDispose` + 每次进入结果按本结果 identity 重播种自
  `GenerateSettings.borderRings`**(`ResultPage` 经 `context.push('/result')` 入栈,普通 `StateProvider` 会把上个结果
  改的 `k` 泄漏进下个结果;兄弟 `_ResultAppBar` 读到的须是**已播种**值、非旧结果残留——防 seed 竞态);加步进器,改动后
  **仅重渲染预览**（不调 `_generate`）;初值取 `GenerateSettings.borderRings`,**不回写默认**;**`ResultPage` 独立 `gridAspect`(:265-266)+ whitespace/legend-flush
  改读共享布局的 `canvasAspect`(含 margin,**非** `(W+2k)/(H+2k)`)**

## 3. 「保存到相册」改 CPU 光栅（`image` 包，非 toImage）

- [x] 3.1 新增 headless 存图（复用 1.1 布局 + **`image: 4.9.1` CPU 光栅** draw 适配器）:画整图 →
  `img.encodePng` → `Uint8List`。**禁用** `Picture.toImage`/`RepaintBoundary.toImage`（iOS 模拟器禁令 + 纹理
  上限,同 crop 绕过手段)。**像素预算**:`cellPx = clamp(floor(maxEdgePx / (max(W,H)+2k)), 4, 10)`
  （`maxEdgePx≈4096`);**最终画布最长边 hard-clamp 到 `maxEdgePx`**(不依赖外部长边上界)。`image.drawString` 用
  **固定位图字体**(arial_14/24/48)——按 `cellPx` 分档选字号,使 4..10 跨度下刻度标号不糊/不越格。CPU 光栅大图较慢
  → 放 `compute`/isolate 或加进度
- [x] 3.2 `result_page.dart`：`output.gridPng` 的**两个**保存调用点（AppBar `:91` + 顶部浮条 onSave `:336`）
  **都**改为「用 3.1 按**当前 `k`** 渲染 PNG → `_saveToAlbum`」;渲染为异步,失败走既有 catch 提示（不崩溃)。
  改后 `output.gridPng` 在 App 侧成**死引用**(引擎仍产、App 不再存)——须先确认无既有测试断言「保存的正是 `gridPng`」

## 4. i18n

- [x] 4.1 新增文案键（`app_zh.arb`/`app_en.arb` **两边** + 聚合 getter）:「边框圈」标签、步进器提示等,
  经 gen-l10n;`flutter gen-l10n` 通过(否则 `i18n_coverage_test` 会抓硬编码中文)

## 5. 验证

- [x] 5.1 `flutter analyze` 无新增告警、`flutter test` 全绿
- [x] 5.2 widget test:格子视图 ① 含每 10 格加粗线 + 轴 1-based 刻度（`W,H≥10`）② `W<10` 或 `H<10` 时该轴
  无加粗线/无刻度 ③ 点内容格 detail 行列 1-based ④ **点边框区不弹 detail**
- [x] 5.3 widget test（边框 + 非方形）：`k` 0→2 → 内容外围 2 圈空留白、内容坐标仍 1..W/1..H、
  **配色统计不变**（边框不计）;**非方形 `W≠H` + `k>0`** 下格子仍正方形、命中测试点内容格映射正确
  （先扣刻度 margin 与 k 偏移）
- [x] 5.4 **防漂移(布局层尺度无关,非预览像素)**:断言交互预览与 CPU 存图**读同一 1.1 布局函数**、且**尺度无关产出
  一致**(`canvasAspect`/刻度标号**值**/内容格线归一化位置/`k`);**不**断言 `cellSize`/`marginTopLeft` 等像素绝对量逐
  值相等(预览 fit vs 保存预算尺度天生不同);对 **CPU 光栅 PNG 做 golden**;**不**对预览侧做像素 golden(需被禁的
  `toImage`)。保存 PNG **从 cells 渲染**（非 `gridPng`/`previewPng`/`toImage`）
- [x] 5.5 **像素预算**:大图（如 1000 宽 + k）保存不 OOM——`cellPx` 自适应降,**且最终画布最长边 hard-clamp 到
  `maxEdgePx`**(不依赖 App 长边 ≤1000 外部上界);用一张会触发 hard-clamp 的超大图断言最终边长 ≤ `maxEdgePx`
- [x] 5.6 **引擎无关对账**：`git diff` 确认零改 `crates/`、零改 `tests/golden/`;`cargo test` 仍全绿
  （纯 `apps/mobile` 展示层,`bead-core`/`gridPng`/`BeadPattern`/统计不受影响）
