# 提案：贴板后调色板感知减色 + 低瓣缩放

## 为什么

当前量化流程 `Lanczos3 缩放 → median-cut 减到 N 色（贴板前、调色板无关）→ 最近邻贴板` 在真实照片上会崩：减色发生在贴板**之前**，且 median-cut 的代表色是**桶内各通道算术均值**——跨色相取均值必然去饱和、向灰质心塌缩。带噪声的近均匀背景桶一旦混入描边的暖暗像素，代表色就变成一坨浊灰（S07 Gray），再被整片贴成灰珠（UncleGao 背景第 5 行一整条灰）。同时 Lanczos3 在 sRGB gamma 空间的负瓣振铃在锐边过冲出发灰/发白像素（顶部虚线带、光晕杂点），进一步污染背景桶。根因是流程架构：**在贴板前用"发明任意代表色"的通用量化器，且不知道珠子调色板的存在**。

## 变更内容

> **依赖（归档顺序，硬性）**：本变更 STACK 在 `add-oklab-matching` 之上——其 pipeline / golden-tests / cli / flutter-ffi delta 假设 `add-oklab` 已归档的基线（默认 `MatcherKind::Oklab`、`--matcher` flag）。**必须先归档 `add-oklab-matching`、再归档本变更**；反序则本变更 MODIFIED 需求对照的基线内容尚不存在。

- **BREAKING** 调整量化流程顺序：从 `缩放 → 减色 → 贴板` 改为 `缩放 → 贴板 → 珠色减色`。减色不再作用于原始像素网格，而是作用于已贴板的 `BeadPattern`。
- **BREAKING** 重定义 `color-reduction` 功能：从"grid→grid 的调色板无关 median-cut"改为"**pattern 级、调色板感知的珠色合并**"——统计已用珠色，反复把**用量最少的珠色合并到感知空间（Lab/Oklab）最近的保留珠色**并重映射其单元格，直到不同珠色数 ≤ `max_colors`。减色全程只在**真实珠色**之间流动，**永不发明中间色**，从根上消除灰质心。
- 减色的确定性口径**按 matcher 分两档（不整体降级）**：`Rgb` 路径仍是**纯整数、跨架构位精确**（可钉跨架构整数 golden）；`Lab`/`Oklab` 路径为**感知距离 f32、同机确定性**（与既有 `LabMatcher`/`OklabMatcher` 同档）。两档均确定：合并平局取保留珠色的较小下标、扫描/合并顺序固定、无随机 / 并行 / 哈希顺序泄漏。
- 缩放默认重采样滤镜从 `Lanczos3` 改为**低瓣/面积平均类**（如 `Triangle`），在降采样阶段抹平源图 ±10 噪声并消除负瓣振铃；`ResizeOptions.filter` 仍可配、可显式指定 `Lanczos3`。
- 不含 Floyd-Steinberg 抖动、gamma-correct 缩放、Gerstner 2012（均为独立后续提案）。

## 功能 (Capabilities)

### 新增功能
（无）

### 修改功能
- `color-reduction`: 从"贴板前 grid→grid 的 RGB median-cut（纯整数）"改为"贴板后 pattern 级、调色板感知的珠色合并（感知距离、同机确定性）"；`max_colors` 语义仍为"最终拼豆最多 N 种珠色（上限）"。
- `pipeline`: `generate_pattern` 的固定阶段顺序由 `缩放 →（可选）减色 → 贴板 → 统计/渲染` 改为 `缩放 → 贴板 →（可选）珠色减色 → 统计/渲染`；统计与渲染基于减色后的 `BeadPattern`。
- `image-grid`: `ResizeOptions` 的**默认**重采样滤镜由 `Lanczos3` 改为低瓣/面积平均类（可配项与显式 `Lanczos3` 保留）。

## 影响

- 代码：`crates/bead-core/src/quantizer`（**移除** grid→grid `Quantizer` trait 与 `MedianCutQuantizer`，新增 pattern→pattern `BeadReducer` + `GreedyReducer`）、`matcher/mod.rs`（`srgb_to_lab`/`srgb_to_oklab`/`linearize`/`check_palette_len` 提为 `pub(crate)` 供 quantizer 复用）、`pipeline/mod.rs`（阶段顺序 + 减色器 fail-fast 构造）、`image/mod.rs`（默认滤镜）、`lib.rs`（重导出增减）。
- 契约/门：所有涉及减色或默认缩放的 **golden 输出会变化**，需重生成 golden 基准；减色**单元** golden **按 matcher 分两档**——`Rgb` 路径仍跨架构位精确（整数），`Lab`/`Oklab` 路径 canonical 同机（f32，同 Lab/Oklab 匹配路径）。CLI `--max-colors` / `--max-colors 0` 面向用户语义不变（`≤N` 上限、`0` 退出 1 且 stderr 含 "max_colors"）。
- 文档 / 迁移面（随 trait 移除必须同步，防漂移）：`ARCHITECTURE.md`（quantizer 模块段 + `Quantizer` trait 代码块〔≈L139–148 / L397〕、Median Cut 描述〔L144〕、管线顺序、确定性段对 `Lanczos3` 重采样的提及〔如 L76〕→ `Triangle`；注：无独立「默认=Lanczos3」行）、`CLAUDE.md`（L56 `Quantizer` trait 列举）、`openspec/config.yaml`（design 规则 L44 `Quantizer / ColorMatcher / Renderer` → 以 `BeadReducer` 取代 `Quantizer`）、`crates/bead-ffi` 默认路径注释（filter `Lanczos3`→`Triangle`）、`crates/bead-core/benches/bench.rs`（若注释提 Lanczos/量化）；`crates/bead-cli/tests/cli.rs::cli_max_colors_ok_and_zero_rejected`（L267）语义不变但需随新错误源（`GreedyReducer::new`）与 golden 重跑复核。**并含三个主规范的 `## 目的` 段**（`color-reduction`/`pipeline`/`golden-tests`——delta 不携带 `目的`、归档需手动更新，见 tasks 7.5）。
- 约束保持：`bead-core` 无 UI/FS、Phase1 单线程、确定性（同输入同输出 + CLI==FFI 同机位一致）、算法走既有 trait 缝。
