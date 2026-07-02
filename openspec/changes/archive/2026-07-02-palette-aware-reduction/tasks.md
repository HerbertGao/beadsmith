# 任务：贴板后调色板感知减色 + 低瓣缩放

## 1. 缩放默认滤镜（②）

- [x] 1.1 将 `image/mod.rs` 中 `ResizeOptions::default().filter` 由 `Lanczos3` 改为 `Triangle`；确认 `filter` 仍可配、可显式指定 `Lanczos3`
- [x] 1.2 更新 image-grid 相关单元测试与断言（默认滤镜、可覆盖）；确保零维守卫、放大、精确尺寸场景仍通过
- [x] 1.3 用 UncleGao 目视对比 `Triangle` vs `Lanczos3`（`CatmullRom` 作参考）的背景杂点数，**确认 `Triangle` 达标**（显著抑噪/去振铃）。默认已钉 `Triangle`（agreed scope）；本步是验收确认、**不改默认**——若发现 `CatmullRom` 明显更优，记入后续提案

## 2. BeadReducer 缝与 GreedyReducer（①，核心）

- [x] 2.1 在 `quantizer` 模块定义 object-safe `BeadReducer { fn reduce(&self, pattern: &BeadPattern) -> BeadPattern }`（不返回 `Result`；**前置条件**：不 panic 仅对「`cells` 均为合法调色板下标、且对构造 reducer 所用同一 palette 配色」的 pattern 成立——越界下标可 panic、不在保证内，见 color-reduction spec；空 pattern 原样返回不 panic）
- [x] 2.2 实现 `GreedyReducer::new(palette: &Palette, matcher: MatcherKind, max_colors: u32) -> Result<_, BeadError>`：**先校验 `max_colors>=1`**（`==0` → `InvalidImage{reason 含 "max_colors"}`，复用变体、不新增）、**后校验 palette**（空/超大沿用匹配器同款 `InvalidPalette` 守卫）——顺序固定以保错误优先级；按 `matcher` 空间快照每个调色板色坐标，**复用 `matcher` 模块 `pub(crate)` 化的 `srgb_to_lab`/`srgb_to_oklab`/`linearize`/`check_palette_len`（不另写副本）**
- [x] 2.5 把 `matcher` 模块的 `srgb_to_lab`/`srgb_to_oklab`/`linearize`/`check_palette_len` 由私有提为 `pub(crate)`，供 `quantizer` 字面复用；`quantizer` **禁止**另写**转换**副本；**距离**用与匹配器**逐字同构的同一平方和公式**（`Σ(Δ)²`、不开方，平凡故允许内联、不强制抽共享 fn），f32 路径**禁** `mul_add`（防 CLI/FFI codegen 分叉）
- [x] 2.3 实现 `GreedyReducer::reduce` 贪心合并：统计用量→若 `d<=max_colors` short-circuit no-op→否则循环「选最少用量牺牲色(平局取下标大)→选其余在用中感知最近目标色(平局取下标小、比较平方距离不开方)→重映射牺牲色单元格」；RGB 度量走整数、Lab/Oklab 走 f32（复用匹配器度量口径）
- [x] 2.4 从 crate 根重导出 `BeadReducer`/`GreedyReducer`

## 3. 移除旧 grid→grid 量化

- [x] 3.1 删除 `Quantizer`(grid→grid) trait 与 `MedianCutQuantizer` 实现及其单元测试
- [x] 3.2 移除 crate 根对 `Quantizer`/`MedianCutQuantizer` 的重导出，修复所有引用点（编译通过）

## 4. 管线顺序切换

- [x] 4.1 改 `pipeline/mod.rs::generate_pattern` 为 **fail-fast 顺序**：`image_to_grid` →（当 `Some(n)`：**先**构造 `let reducer = GreedyReducer::new(palette, opts.matcher, n)?`，在配色**之前**）→ 按 `opts.matcher` 选定 matcher `new(palette)` → `match_pattern` →（当 `Some(n)`：**再** `reducer.reduce(&pattern)`）→ `count_colors/generate_summary → render_*`；`max_colors=None` 时不构造 reducer、pattern 原样。**顺序关键**：reducer 构造在配色前，保证 `max_colors==0` 的 `InvalidImage` 先于 matcher 的 `InvalidPalette`（见 pipeline spec 错误优先级）
- [x] 4.2 确认统计/摘要/两张 PNG 全部基于**减色后**的 `BeadPattern`
- [x] 4.3 更新 pipeline 单元测试：忠实串联（新顺序）、`max_colors=None` 时**减色恒等跳过**（对照「同 `opts` 移除减色阶段」的输出、**非**旧默认字节；**不得**据此保留 `Lanczos3`）、`Some(n)` 时 stats 不同珠色数 ≤ n 且统计/渲染来自减色后 pattern、`Some(0)` 透传 `InvalidImage`（先于配色）、**有效图 + 非法 palette + `Some(0)` → `InvalidImage`（非 `InvalidPalette`）**

## 5. 减色单元测试（确定性/语义）

- [x] 5.1 形状一致、空 pattern no-op 不 panic
- [x] 5.2 `d<=max_colors` 自然 no-op（逐单元格不变）；`1<=max_colors<d` 上限成立（不同珠色数 ≤ max_colors）；`max_colors==1` 合并到单一珠色
- [x] 5.3 合并只在真实珠色间发生（输出索引都在输入中出现过、无调色板外色）
- [x] 5.4 确定性：重复减色逐字节相等；牺牲/目标平局规则可复现
- [x] 5.5 RGB 路径跨架构位精确的硬编码 golden（整数）；Lab/Oklab 路径同机确定性
- [x] 5.6 一致性测试（**选择等价**，只用已暴露的 `find_best_match`）：`GreedyReducer`（某 matcher）为某牺牲色在一组保留色中选出的目标色，== 用同一 matcher 对**仅含这组保留色的子调色板** `new` 后、对牺牲色 RGB 调 `find_best_match` 的结果（守卫「减色最近 == 配色最近」、防转换实现漂移）。子调色板须**保留保留色的相对顺序**，使平局取小下标在 reducer 与 `find_best_match` 间一致

## 6. Golden 与端到端

- [ ] 6.1 重生成受影响的 golden 基准（默认滤镜变更 + 减色路径变更），更新 golden-tests 夹具
- [x] 6.2 端到端跑 UncleGao（oklab + max-colors=24/48）确认：背景无 S07 Gray 灰条、开阔蓝背景无灰珠、`max_colors` 上限生效；**并检查高光/点睛色（如反光点）未被 least-used 合并过度吞掉**（若明显，记为已知取舍、非 blocker，见 design Risks）
- [x] 6.3 `cargo build`、`cargo test`、`cargo clippy` 全绿

## 7. 文档

- [x] 7.1 同步 `ARCHITECTURE.md`：管线顺序与减色描述（缩放→贴板→减色；调色板感知合并）、**改写 `quantizer` 模块段与 `Quantizer` trait 代码块（≈L139–148 / L397）、Median Cut 描述（L144）为 `BeadReducer`/`GreedyReducer`**、默认滤镜 `Lanczos3`（L76）→ `Triangle`
- [x] 7.2 同步 `CLAUDE.md`（L56 `Quantizer` trait 列举 → `BeadReducer`）与 `openspec/config.yaml`（design 规则 L44 `Quantizer / ColorMatcher / Renderer` → `BeadReducer` 取代 `Quantizer`）
- [x] 7.3 更新 `crates/bead-ffi` 默认路径注释（filter `Lanczos3`→`Triangle`）、`crates/bead-core/benches/bench.rs` 若注释提 Lanczos/量化
- [x] 7.4 复核 `crates/bead-cli/tests/cli.rs::cli_max_colors_ok_and_zero_rejected`（L267）：语义不变（`≤N`、`--max-colors 0` 退出 1 且 stderr 含 "max_colors"），但错误现源自 `GreedyReducer::new`，随 golden 重跑复核通过
- [x] 7.5 归档时更新三个主规范的 `## 目的` 段（delta 不携带 `目的`、归档原样保留、否则与被改需求自相矛盾）：`openspec/specs/color-reduction/spec.md`（"`Quantizer`/`MedianCutQuantizer` 在配色前处理 `PixelGrid`" → 配色后 pattern 级珠色合并）、`openspec/specs/pipeline/spec.md`（"两个浮点源 `Lanczos3` + `OklabMatcher`" → 默认 `Triangle` + `OklabMatcher` + 可选减色）、`openspec/specs/golden-tests/spec.md`（"`Lanczos3` 经 `f32::sin`" → 默认 `Triangle`（f32）+ `OklabMatcher` `cbrt`）
