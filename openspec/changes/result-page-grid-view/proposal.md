## 为什么

M9 的 `ResultPage` 只给用户三样东西：位图预览（`previewPng`）、配色计数列表（`stats`）、以及一段与计数信息高度重叠的「汇总」文字块（`summary`）。用户真正"照着拼"时最需要的那一层——**第 N 行第 M 格该用哪颗豆**——完全缺失；位图预览又不可交互、无法点查。同时生成结果只活在内存里，用户切其它 App 被系统杀后台即丢失，缺最低限度的持久化出口。「汇总」文字块与上方配色列表信息冗余 90%，是噪声而非信号。

本次变更落在 ROADMAP「Post-M9 — Mobile UI Refinement」工作流：纯前端，不碰引擎、不碰 FFI 边界、不影响确定性。

## 变更内容

- **新增**：可交互格子视图 `BeadGridView`——`CustomPainter` 按 `pattern.cells`（u16 调色板索引）+ 解析后的 palette 渲染每格纯色方块，格间留 gap 透出 line 色形成可见网格线（gap 随 cell 大小缩放，clamp）；`InteractiveViewer` 包裹支持双指缩放（1×–20×）与拖动平移，大尺寸 grid（如 100×100）可放大到点单格。**缩放为相册式手感**：`AspectRatio` 置于 `InteractiveViewer` 内部并 `Center`，视口即整个区域、grid 居中（上下留白在视口内），放大时内容整体 transform，留白随 grid 放大自然消失。点格命中走 `InteractiveViewer` 自带的 `onInteraction*` 回调（单指无位移=tap，用 `globalToLocal` 逆映射回格子坐标），**不引入独立 `GestureDetector`**，从而双指缩放永不被 tap 手势竞争抢走。
- **新增**：点格 → 底部 detail sheet（色块 + code + name + count + 行列位置 + "高亮同色"按钮）；"高亮同色"在 grid 上对所有同 `paletteIndex` 的格子描边。
- **新增**：保存到相册——`AlbumService` 封装 `gal` 包，存 `GenerateOutput.gridPng`（引擎渲染的格子图，带网格线/行列号，即"照着拼"的参考图；非平滑 `previewPng`）；首次进入 `ResultPage` 弹一次性**顶部浮动提示条**（文案「建议保存到相册」，7s 自动消失 + × 手动关 + 保存快捷键；session 级 flag，不跨启动 nag，不占底部）。
- **新增**：Dart 端 palette 解析 `parsePalette`——把已内置的 `artkal_s.json` 解析成 `List<PaletteColor>`，**列表下标与 `BeadPattern.cells[i]` 的索引同序**（对齐 `bead_core::palette::load_palette` 保序），供 grid 渲染色块。
- **重构**：`ResultPage` 布局从 `ListView` 改为 grid 主角——grid 区（`Expanded`，居中 letterbox 保留上下留白）+ AppBar（preview 缩略图点击放大 + 保存 + 复制）+ 底部**可展开配色面板**（`_LegendSheet`：收起=薄栏「配色·N 色」+ 迷你色块；点击 header 切换 `_legendExpanded`，父层 `AnimatedContainer` 在收起/展开高度间动画；展开时 grid 区收缩到刚好 grid 高度，grid 上移贴配色顶边、填掉顶部留白，配色顶边不与 grid 冲突）。
- **新增**：默认目标尺寸从 50×50 提到 100×100（生成页初值），更贴合实际拼豆图精度。
- **新增**：底部安全区处理——不套外层 `SafeArea`，配色面板表面色铺满至物理底边（延伸到 home indicator 下），列表内容底部 padding 叠加 `MediaQuery.paddingOf().bottom`，色面连续且末行清出 home indicator，无死白。
- **移除**：「汇总」文字块（`SelectableText(summary)`）。复制 summary 保留在 AppBar 右上角图标按钮（M9 既有）。
- **BREAKING**：无（App 未上架，且仅改 `ResultPage`/`GeneratePage` 呈现，不改导航流、不改 FFI 契约、不改引擎输出）。

## 功能 (Capabilities)

### 新增功能

（无）

### 修改功能

- `mobile-app`: `ResultPage` 的呈现需求变更——新增"逐格可交互格子视图"作为主视图、新增"保存到相册"持久化出口、移除冗余的"汇总"文字块；preview 从主图降为 AppBar 缩略、配色 legend 从平铺列表降为底部可展开栏。规范级的行为变更（不只是实现细节）：结果页的"看整体"与"逐格辨认"两个任务分离，且结果可离物化到相册。

## 影响

- **代码**（全部在 `apps/mobile`，引擎三 crate 零改动）：
  - `lib/presentation/result_page.dart`（重写：grid 主角 + 可展开配色 sheet + 顶部保存提示 + 安全区处理）
  - `lib/presentation/bead_grid_view.dart`（新：CustomPainter grid + 相册式缩放 + 网格线）
  - `lib/presentation/generate_page.dart`（默认尺寸 50→100）
  - `lib/infrastructure/palette_codec.dart`（新）
  - `lib/infrastructure/album_service.dart`（新）
  - `lib/application/save_to_album.dart`（新）
  - `lib/application/providers.dart`（+`paletteProvider` / `albumServiceProvider` / `saveToAlbumProvider`）
  - `ios/Runner/Info.plist`（+`NSPhotoLibraryAddUsageDescription`）
  - `test/palette_codec_test.dart`、`test/bead_grid_view_test.dart`（新）
- **依赖**：新增 `gal: 2.3.2`（精确 pin，与 pubspec 既有惯例一致）。stdlib 无相册写入能力；已有 `image` 包只做编解码不做相册持久化；自写平台通道成本高于引入一个轻量社区包。`gal` 仅出现在 `apps/mobile`，不污染任何 crate。
- **bead-core 模块**：无（`image` / `palette` / `quantizer` / `matcher` / `renderer` / `statistics` / `pipeline` / `models` / `errors` 均不动）。
- **里程碑**：Post-M9（Mobile UI Refinement）。
- **确定性**：无影响。纯 UI 改动，不碰 `pipeline::generate_pattern`、不碰 FFI 边界、不碰 golden。格子视图读取的是已有结构化数据（`BeadPattern.cells` + palette JSON），不从 `previewPng`/`gridPng` 位图反推（守 CLAUDE 硬规则 3）；保存到相册直接透传引擎已生成的 `gridPng`，不在壳内重绘。同 `image+palette+dimensions+options` 仍产生逐字节相同的引擎输出。默认尺寸改动只影响新会话初值，不改引擎逻辑。
- **算法 Phase**：无新算法（纯呈现层）。
- **跨平台**：Android（Pixel_10 / API 37 模拟器）与 iOS（iPhone 17 Pro / iOS 26.5 模拟器）均实测：编译、安装、四屏流程、点格、相册式缩放、配色展开、保存到相册、安全区均通。`NSPhotoLibraryAddUsageDescription` 权限已加。

## 非目标

- **不引入 in-app 方案持久化**（`SaveProject` / 项目列表 / 一键调起）。这属 domain 层，ARCHITECTURE.md 标注"持久化落地时再建"，本次只预留"保存到相册"这个离物化出口；in-app save 留待后续单独变更。
- **不改 FFI 边界**——`generate` 的参数与返回类型不变；`palette_codec` 是 Dart 侧解析已内置的 palette JSON，不改 `bead-ffi`，也不把 palette RGB 加进 `ColorStat` DTO（保持 M8 边界）。
- **不改引擎**——`bead-core` / `bead-cli` / `bead-ffi` 零改动；golden 测试不受影响。
- **不改其它三屏**（Home / Crop / Generate）——本次只动 `ResultPage`。
- **不做格子视图的坐标/行列标注叠加**（如行列号刻度）——留给后续打磨，本次只解决"点格查豆"核心。
- **不做 grid 截图另存**——本次保存到相册只存引擎已渲染的 `gridPng`（带网格线/行列号的格子图）；App 端的交互 grid 视图本身不另做截图另存。
