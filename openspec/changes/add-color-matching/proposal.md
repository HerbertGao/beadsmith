## 为什么

里程碑 **M3 — Color Matching**。M2 给出了 `PixelGrid`（配色前的原始 RGB 网格，1 像素 = 1 颗豆）；
M3 把每格的原始 RGB 映射到**最近的真实豆色**，产出 `BeadPattern`——`BeadPattern` 自此成为
管线的**真理源**（CLAUDE 规则 3），M4 统计、M5 渲染、M6 导出都从它派生。Phase 1 用
RGB 欧氏距离；Phase 2 的 CIELAB/ΔE 将来在同一 `ColorMatcher` trait 后替换，不改管线。

与 M2 的关键差别：匹配是**纯整数运算**（`[u8;3]` 差的平方和），无 `f32`——跨架构逐字节一致是
**保证**而非「赌它相同」。M2 的 `Lanczos3` 是 `f32`，当时不敢硬编码跨架构 golden（只能用
`Nearest` 绕）；M3 第一次可以钉一份真正的跨架构位精确 golden，为 M7/M8 减负。

## 变更内容

- `bead-core` 新增 `matcher` 模块：
  - `pub trait ColorMatcher { fn find_best_match(&self, target: [u8;3]) -> u16 }`——把任意 RGB
    映射到调色板下标。Phase 2 的 Lab matcher 是同一 trait 的第二实现（ARCHITECTURE 既定接缝）。
  - `pub struct RgbMatcher`：Phase 1 实现。`RgbMatcher::new(&Palette) -> Result<RgbMatcher, BeadError>`
    构造时把 `palette.colors` 的 RGB 拍平成 `Vec<[u8;3]>` 快照（性能接缝；Lab matcher 将来在 `new`
    预转 Lab）。`find_best_match` = 平方欧氏距离、最低下标平局。
  - `pub fn match_pattern(grid: &PixelGrid, matcher: &dyn ColorMatcher) -> BeadPattern`——按声明维度
    产 `width*height` 格：`cells[i] = find_best_match(grid.pixels[i])`，`width/height` 原样搬，行优先
    一一对应。**前置条件**（同 M2 `PixelGrid` 口径）：要求 `grid.pixels.len() == grid.width as usize * grid.height as usize`；该不变量
    由 `resize_image` 保证，外部手构破坏即调用方契约违约——`match_pattern` 保持全函数不返 `Result`（见 D5）。
- 扩 `models` 模块：新增 `BeadPattern { width:u32, height:u32, cells: Vec<u16> }`（行优先
  `cells[y*width+x]` = 调色板下标，`cells.len() == width*height`，derive `Debug+Clone+PartialEq`，
  **不 derive `Eq`**；提供 `cell_at(x,y) -> Option<u16>`）。**M3 不含 `stats` 字段**（推迟 M4）。
- 错误模型**复用** M1 的 `InvalidPalette { reason }`——不新增 `BeadError` 变体（见 design D7）。
- **彻底移除 `BeadCell` 类型**（不只是「取代」——M3 不引入 `BeadCell` struct），并同步校正**所有**仍
  声明该早期草图的真理源，避免文档与代码自相矛盾：
  - `ARCHITECTURE.md`：Data Model Layer 的 `BeadCell { x, y, color_index }` → `BeadPattern.cells:
    Vec<u16>`（行优先，坐标可推，删 `BeadCell` struct）；Rendering Strategy 标注配色后 `BeadPattern`
    是真理源、`PixelGrid` 降为配色前中间体。
  - `ROADMAP.md` M3：把「Introduce `BeadCell`/`BeadPattern`」「`BeadCell.color_index` now points into
    the palette」三处 `BeadCell` 措辞改为 `BeadPattern { cells: Vec<u16> }`（**强制对齐，非「如需」**）。
  - `INIT.md` Data Models 块：删 `BeadCell` struct、`BeadPattern.cells` 改 `Vec<u16>`，并注明 `stats`
    随 M4（INIT 是产品草图，与 ARCHITECTURE 保持同一数据模型故事）。
  - `crates/bead-core/src/models/mod.rs` 模块级 doc-comment（现写「M3 adds `BeadCell` / `BeadPattern`」）：
    随 task 1.1 在同一文件编辑里改为描述 `BeadPattern { cells: Vec<u16> }`，否则该注释一落地即成谎。

> 契约说明：`BeadCell.color_index` 这个早期草图字段被 `Vec<u16>` 取代——延续 M2 `PixelGrid` 的
> 行优先无坐标先例（坐标由 `y*width+x` 推出，省 ~64KB/80×100），保持 models 层内部一致。
> `BeadPattern` 是配色后的真理源；`PixelGrid` 在 `match_pattern` 处完成交接、不再返回外部。
> 归档目录 `openspec/changes/archive/...` 下对 `BeadCell` 的引用是不可变历史，不动。

## 功能 (Capabilities)

### 新增功能
- `color-matching`: 把 `PixelGrid` 逐格映射到调色板最近色，产出 `BeadPattern`（行优先 `Vec<u16>`
  下标）；约定确定性的距离度量、平局规则与跨架构整数一致性。

### 修改功能
<!-- 无：M3 不改 palette / image-grid 已生效规范的需求（只消费它们）。 -->

## 非目标（Non Goals）

按 YAGNI 推迟 / 不做：

- `stats` / `ColorStat` / `BeadPattern.stats` → M4（M3 只产 `cells`）。
- CIELAB / ΔE 感知距离 → Phase 2（在 `ColorMatcher` trait 后替换，不改管线）。
- 颜色量化 / 抖动（dithering）→ Phase 2。
- preview / grid 渲染 → M5（从 `BeadPattern` 派生）。
- `pipeline::generate_pattern` 串联 → M6（M3 的三个 `pub` 是库内/pipeline 复用原语，非 FFI 入口）。
- 离色阈值 / 警告策略 → Phase 2（`find_best_match` 只回 `u16` 下标，不回距离；留接缝不留代码）。
- `rayon` 并行 → Phase 2（ARCHITECTURE 性能策略；M3 单线程）。

## 影响

- **代码**：
  - `crates/bead-core/src/lib.rs`（改：`pub mod matcher;` + 重导出 `ColorMatcher / RgbMatcher /
    match_pattern / BeadPattern`；**不新增 `BeadError` 变体**）
  - `crates/bead-core/src/matcher/mod.rs`（新）、`crates/bead-core/src/models/mod.rs`（扩 `BeadPattern`
    **并改模块级 doc-comment**：现「M3 adds `BeadCell` / `BeadPattern`」→ `BeadPattern { cells: Vec<u16> }`）
- **依赖**：无新增（匹配是纯整数运算 + M1 `Palette` + M2 `PixelGrid`）。
- **确定性**：匹配全程整数（平方欧氏、无 sqrt、无 `f32`、无 `rayon`）、最低下标平局规则固定 →
  **同字节输入 → 逐字节相同 `BeadPattern`，且跨架构（arm64/x86_64）位精确**。这是 M2 的 `Lanczos3`
  做不到、特意推迟的——M3 因此可以钉一份硬编码的真 golden（见 design D8），直接成为 M7 frozen
  检查与 M8「CLI == FFI」在匹配层的强锚点。
- **里程碑 / Phase**：里程碑 M3；算法 Phase 1（RGB 欧氏；Phase 2 的 Lab 在 trait 后替换）。
- **文档**：四处真理源同步校正（见「变更内容」的 `BeadCell` 移除清单）——`ARCHITECTURE.md`
  （Data Model Layer + Rendering Strategy）、`ROADMAP.md` M3（三处 `BeadCell` 措辞）、`INIT.md`
  Data Models 块、`models/mod.rs` 模块级 doc-comment。归档变更里的 `BeadCell` 引用是不可变历史，不动。
