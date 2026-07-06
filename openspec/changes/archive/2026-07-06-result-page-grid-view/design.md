## 上下文

M9 的 `ResultPage`（`apps/mobile/lib/presentation/result_page.dart`）以 `ListView` 平铺三块：位图预览 `previewPng`、配色计数 `stats`、`summary` 文字块。问题：①"照着拼"最需要的"第 N 行第 M 格用什么豆"完全缺失；②`previewPng` 是位图、不可点查；③`summary` 文字块与 `stats` 信息重叠 90%，是噪声；④结果只活在内存，切后台被杀即丢失。

本次变更属 ROADMAP「Post-M9 — Mobile UI Refinement」，纯前端，引擎三 crate 零改动，确定性不受影响。设计方向由 qiaomu-design skill 三阶段工作流裁定为 B 方案（grid 全屏主角 + preview 缩略 + legend 底栏），用户明确选定。

约束（ARCHITECTURE.md 五条硬规则）：
- `BeadPattern` 是唯一真相源；统计/预览/导出都从它派生，**绝不从渲染图反推**。
- `bead-core` 不感知 UI/平台；本次只动 `apps/mobile`，不碰任何 crate。
- FFI 边界（`generate` 参数与返回类型）不变。
- 确定性：同输入同输出；本次纯呈现层，不影响引擎输出。

## 目标 / 非目标

**目标：**
- 在 `ResultPage` 提供"逐格可交互格子视图"作为主视图：双指缩放、拖动平移、点格查豆。
- 提供"保存到相册"持久化出口，防切后台丢失。
- 砍掉冗余的"汇总"文字块；复制 summary 保留在 AppBar。
- 守住"不从 `previewPng` 位图反推信息"硬规则——格子视图必须读结构化 `pattern.cells`。

**非目标：**
- 不引入 in-app 方案持久化（`SaveProject` / 项目列表）——属 domain 层，后续单独变更。
- 不改 FFI 边界（不给 `ColorStat` DTO 加 RGB 字段）。
- 不改引擎 / golden / 其它三屏。
- 不做行列号刻度叠加、不做 grid 截图另存。

## 决策

### D1. 格子渲染：`CustomPainter` 而非 `GridView.builder`

每格一个纯色方块。大尺寸 grid（100×100 = 10000 格，150×150 = 22500 格）下，`GridView.builder` 会实例化 N 个 `Container` widget，内存与布局开销大；`CustomPainter` 单次 `paint` 遍历画 N 个 `drawRect`，无 widget 树负担，`shouldRepaint` 只在 `cells`/`highlightedIndex` 变化时重绘。

- **替代方案**：`GridView.builder` + `ColoredBox`。**否决理由**：10000+ widget 实例化在低端机上帧时间不可接受；且 hit-test 交给 Flutter 命中测试在缩放后精度下降。`CustomPainter` + 手算命中更可控。
- **替代方案**：把 grid 渲染成 `Image` 再显示。**否决理由**：违反"不从渲染图反推"——一旦转成位图，点查就要回映像素→cell，精度差且脆。

### D2. 缩放/平移/点击：自管 `GestureDetector.onScale*` + `Transform`，而非 `InteractiveViewer`

grid 用 `GestureDetector`（`onScaleStart`/`onScaleUpdate`/`onTapUp`）自己维护 `scale`/`offset`，套一个 `Transform`（translate + scale）应用到含 letterbox 留白的整个视口内容。缩放围绕手势 focal point（手指下的内容保持不动），并 clamp offset 防止 grid 被拖出屏幕。点击命中在 `onTapUp` 里对当前 transform 求逆（`scene = (point - offset) / scale`）再减去 grid 1× 的 top-left，落到格子坐标。

**相册式手感**：`Transform` 作用于「居中的 grid + 其上下留白」这一整块，1× 时 grid 居中留白在两侧，放大时整块一起放大，grid 长大、留白被推出视口——与相册放大方形图一致。

- **替代方案（初版实现）**：`InteractiveViewer` 内建缩放 + `onInteraction*` 回调检测 tap。**否决理由（实测事故）**：在 Android 模拟器的 Ctrl-drag 捏合下 `InteractiveViewer` **完全不响应缩放**（同机、同手势，裁剪页的 `GestureDetector.onScale*` 却正常）。改用与裁剪页（`CropFrame`）同一套自管 `onScale*` 机制后 Android/iOS 均正常。`test/bead_grid_view_test.dart` 新增「双指 pinch → Transform scale > 1×」测试锁住这个行为，防回退。
- **替代方案**：`InteractiveViewer` 外套 `GestureDetector`。**否决理由**：既然 `InteractiveViewer` 的缩放本身不响应，包在外面也无意义；自管 Transform 一处搞定缩放+平移+命中。

### D3. 每格 RGB 来源：Dart 端解析 palette JSON，而非给 `ColorStat` DTO 加 RGB

`ColorStat` DTO 只有 `code/name/count`（M8 边界）。grid 需要 index→RGB 映射。新增 `parsePalette(String json)` 在 Dart 侧解析已内置的 `artkal_s.json`（`paletteJsonProvider` 已有），`BeadPattern.cells[i]` 的 u16 索引直接下标这个 `List<PaletteColor>`——顺序对齐 `bead_core::palette::load_palette` 保序。

- **替代方案**：给 `ColorStat` 加 `rgb` 字段（改 FFI DTO）。**否决理由**：扩大 FFI 边界，`bead-ffi` 要改 + FRB 重新生成 + iOS/Android 重编；而 palette JSON 本就在 App 里、本来就要传给 `generate`，Dart 侧解析是零边界成本的镜像读取。
- **替代方案**：从 `previewPng` 像素采样取色。**否决理由**：违反硬规则 3（不从渲染图反推）。

### D4. 保存到相册：`gal` 包，而非自写平台通道

`gal` 2.3.2 封装 iOS `UIImageWriteToSavedPhotosAlbum` + Android MediaStore，一个 `putImageBytes` 调用搞定。`AlbumService` 包一层权限请求（`hasAccess`/`requestAccess`），`SaveToAlbum` use case 调它。

- **替代方案**：自写 `MethodChannel` + 原生代码。**否决理由**：要写 Swift + Kotlin 两份平台代码 + 权限处理，成本远高于引入一个 2KB 的社区包。
- **替代方案**：`photo_manager`。**否决理由**：功能过重（完整相册管理 API），本场景只需"存一张图"。
- **代价**：新增一个 `apps/mobile` 依赖。`gal` 不进任何 crate，不影响引擎依赖树。精确 pin `2.3.2` 与 pubspec 既有惯例一致。

### D5. grid 为主、preview 缩略、配色可展开面板（B 方案 + 用户迭代）

`ResultPage` 用 `Stack`：底层 `Column`（grid 区 `Expanded` + 底部配色面板 `_LegendSheet`），叠加顶部保存提示。`AppBar` 左侧放 `previewPng` 缩略图（点击 `Dialog` + `InteractiveViewer` 放大），右侧保存 + 复制图标。点格子弹底部 detail sheet（色块 + code + name + count + 行列位置 + "高亮同色"）。

**配色面板的展开行为（用户迭代确定）**：`_LegendSheet` 收起态是薄栏（drag handle + "配色 · N 色" + 迷你色块）；点击 header 切换 `_legendExpanded`，父层 `AnimatedContainer` 在 `collapsedH` 与 `expandedH` 间做高度动画。`expandedH` 由 `LayoutBuilder` 动态算 = grid 的 letterbox 留白高度（`(1 - gridHF) * bodyH`，clamp 到 `≤0.6*body`）——展开时 grid 区 `Expanded` 相应收缩，grid 上移贴合面板顶边，**顶部留白被吃掉、面板顶边不遮 grid**。grid 本身保持居中 letterbox（1× 时上下留白保留，符合用户「格子图可以有上下空白」的明确要求）。

- **替代方案（初版）**：配色是固定薄栏，点击弹独立 `showModalBottomSheet`。**否决理由**：用户要「配色拉起时 grid 上移填掉空白、面板顶边贴 grid 底边」，独立 modal 盖在 grid 上不满足；改为 Column 内高度联动的 `_LegendSheet`。
- **替代方案（DraggableScrollableSheet）**：用可拖拽 sheet。**否决理由**：拖拽 sheet 覆盖在 grid 上（overlay），grid 不会随之收缩上移；且 max 高度要精确等于留白才不遮 grid，Column 内高度联动更直接。
- **替代方案 A（渐进优化）**：保留 `ListView`。**否决理由**：大尺寸 grid 在列表里空间受限。
- **替代方案 C（理想工作台）**：preview 叠层 + 浮动 legend。**否决理由**：工作量过大、浮动面板易遮挡；待 in-app save 落地后再升级。

### D6. 高亮同色：`CustomPainter` 描边，而非背景叠色

`highlightedIndex` 设置时，painter 对所有 `cells[i] == highlightedIndex` 的格子画 `PaintingStyle.stroke` 描边（accent 色，线宽随 cell 尺寸缩放）。状态由 `ResultPage` 持有（`_highlightedIndex`），grid 只读。

- **替代方案**：给同色格子叠半透明背景色。**否决理由**：遮住豆色本身，破坏"豆色是主角"的设计契约。描边只加轮廓不遮色。

### D7. 保存提示：顶部浮动条，7s 自动消失，session 级 flag

首次进入 `ResultPage` 弹一次提示（文案仅「建议保存到相册」+ 保存快捷键 + × 关闭），`static bool _saveHintShown` 保证 session 内只弹一次。提示是 `Stack` 顶部 `Positioned` 的浮动 pill（`AnimatedOpacity`），`Future.delayed(7s)` 后自动淡出。

- **替代方案（初版）**：底部 `SnackBar`。**否决理由（用户反馈）**：用户要「不常显示、类似提示、5–10s 自动消失、不在底部、文案只提示保存」。底部持久 SnackBar 与配色面板位置冲突且不够轻。改为顶部浮动 pill + 7s 自动淡出。
- **替代方案**：`SharedPreferences` 持久化 flag。**否决理由**：跨启动不弹会让新用户错过；session 级既不 nag 又能每次冷启动温和提醒一次。

### D8. 保存内容：`gridPng`（引擎格子图），而非 `previewPng`

保存到相册存 `GenerateOutput.gridPng`（引擎 M5 `render_grid` 渲染的格子图，带网格线 + 行列号，正是"照着拼"的参考图），不是平滑的 `previewPng`。`gridPng` 由 FFI 层从 `GenerateResult.grid_png` 直接 move 出（有 `assert !is_empty`）。AppBar 缩略图与点击放大仍用 `previewPng`（"看整体"用途）。

- **替代方案**：存 `previewPng`。**否决理由（用户反馈）**：用户明确「保存的应该是格子图」——平滑预览不能照着拼，带网格线/行列号的 gridPng 才是参考图。
- **替代方案**：截 `BeadGridView` 的 `RepaintBoundary`。**否决理由**：引擎已渲染 gridPng（含行列号刻度，比 App 端 grid 视图信息更全），直接透传零成本且守硬规则（不在壳内重绘）。

### D9. 底部安全区：面板铺到物理底边 + 内容 inset，而非外层 `SafeArea`

不套外层 `SafeArea`。配色面板表面色铺满至物理底边（延伸到 home indicator 下），列表内容底部 padding 叠加 `MediaQuery.paddingOf().bottom`；`collapsedH`/`expandedH` 也加上该 inset。这样色面连续、末行清出 home indicator、面板下方无死白。

- **替代方案（初版）**：整个 body 套 `SafeArea(top: false)`。**否决理由（用户反馈：安全区不自然）**：`SafeArea` 把面板顶到 home indicator 上方，面板下方留一条与面板不连续的死白，末行还被挤。iOS 自然做法是表面铺到底、内容加 inset。

### D10. 默认目标尺寸 50→100

`GeneratePage` 的默认宽/高从 50×50 提到 100×100（初值 + 种子 height 计算），更贴合实际拼豆图精度（用户要求）。只改初值，不改引擎逻辑，`_clampSide` 上限 1000 内、预设 `[50,80,100]` 已含 100。

## 风险 / 权衡

- **[大尺寸 grid 性能]** 150×150 = 22500 格，单次 `paint` 画 22500 rect。默认已提到 100×100（10000 格）。→ 缓解：`shouldRepaint` 严格比对，仅 `cells`/`highlightedIndex`/`accent`/`lineColor` 变化才重绘；缩放/平移由 `Transform` 的矩阵处理（`setState` 只改 scale/offset，painter 不重画）。实测 iOS/Android 模拟器上 100×100 交互流畅；超大 grid 待真机压测，必要时改 `Picture.toImage` 缓存。
- **[跨平台缩放机制]** 缩放走自管 `GestureDetector.onScale*`（非 `InteractiveViewer`，后者在本机 Android 模拟器捏合下不响应）。→ 缓解：与已验证的 `CropFrame` 同一套机制；`test/bead_grid_view_test.dart` 有「pinch → Transform scale > 1×」回归测试；Android(Pixel_10/API 37) 与 iOS(iPhone 17 Pro/26.5) 模拟器均已实测缩放/平移/点格。
- **[点击 vs 缩放手势竞争]** 单指点格与双指缩放需分流。→ 缓解：`onTapUp`（抬起无缩放）与 `onScaleUpdate`（有位移/缩放）由 `GestureDetector` 手势竞技场天然分流；`HitTestBehavior.opaque` 已加；缩放后点格经逆变换命中，测试覆盖。
- **[palette 解析顺序漂移]** `parsePalette` 依赖 JSON `colors` 数组顺序与 engine `load_palette` 保序一致。→ 缓解：`test/palette_codec_test.dart` 锁住"index 0 == 第一个 color"；若未来 engine 改排序，测试会先红。
- **[保存权限被拒]** 用户拒绝相册权限。→ 缓解：`AlbumAccessDenied` 异常被 `_saveToAlbum` 捕获，弹"相册权限被拒绝，请在系统设置中允许"SnackBar，不崩。iOS `NSPhotoLibraryAddUsageDescription` 已加。
