# 设计：贴板后调色板感知减色 + 低瓣缩放

## Context

现状管线（`pipeline::generate_pattern`）：`image_to_grid(Lanczos3 缩放) →（可选）MedianCutQuantizer.quantize 减色 → match_pattern 贴板 → 统计/渲染`。减色是 grid→grid、调色板无关的 RGB median-cut，代表色 = 桶内分量均值。实测在带噪照片上会把近均匀背景桶塌成灰质心（S07 Gray），贴板后整片变灰；Lanczos3 在 gamma 空间的负瓣振铃又在锐边制造发灰污染像素。约束：`bead-core` 无 UI/FS、Phase1 单线程、确定性（同输入同输出 + CLI==FFI 同机位一致）、算法走 trait 缝。

## Goals / Non-Goals

**Goals**
- 根治"oklab + 减色 → 背景塌成灰珠"的灰条，以及"减色反而让背景更脏"。
- 让 `max_colors` 语义落到"最终拼豆最多 N 种珠色"，且减色只在真实珠色间发生。
- 降采样阶段抑制源噪声与振铃。

**Non-Goals**
- Floyd-Steinberg 抖动（③，改肤色/渐变）、gamma-correct 缩放（④）、Gerstner 2012（联合降采样+调色板优化）——均单独后续提案。

## Decisions

**D1：减色移到贴板之后，作用于 `BeadPattern`（珠色索引）而非 `PixelGrid`。**
管线顺序改为 `缩放 → 贴板 → 珠色减色 → 统计/渲染`。减色输入是已贴板的 `BeadPattern` + `Palette`，输出不同珠色数 ≤ `max_colors` 的 `BeadPattern`。理由：贴板前的通用量化会"发明"非调色板中间色，是灰质心的根源；贴板后合并只在**已存在的真实珠色**间流动，不可能产生调色板外颜色。
- *备选*：保留贴板前量化但改成调色板感知（把每个桶代表色贴到调色板）。否决：仍是两段量化、且桶均值仍会先去饱和，不如直接在结果珠色上合并干净。

**D2：贪心"最少用量合并"算法（确定性）。**
统计各珠色用量；当不同珠色数 > `max_colors`：选**用量最少**的珠色为牺牲色（平局取**调色板下标较大者**，使低下标珠色更易存活，与"最低下标优先"精神一致）→ 合并到**保留珠色中感知距离最近者**（平局取**调色板下标最小者**，与匹配器一致）→ 把牺牲色的所有单元格重映射到目标 → 重复。
- *备选*：k-means / median-cut on 珠色。否决：贪心合并对"上限语义 + 确定性 + 复用匹配器距离"最简单直接，Phase1 单线程足够。

**D3：减色距离复用当前匹配器的度量。**
RGB 匹配 → RGB 平方欧氏（整数）；Lab → ΔE76²；Oklab → ΔEok²（f32）。Reducer 在构造期按所选空间快照每个调色板色的坐标（同 `LabMatcher`/`OklabMatcher` 的快照做法）。理由：Lab 结果在 Lab 里合并才自洽；且 RGB 路径的整数度量得以**保持跨架构位精确**（见 D5）。

**D4：减色走新 trait 缝，取代旧的 grid→grid 量化。**
新增 object-safe `BeadReducer { fn reduce(&self, pattern: &BeadPattern) -> BeadPattern }` 与实现 `GreedyReducer::new(palette, matcher_kind, max_colors) -> Result<_, BeadError>`。**移除**现 `Quantizer`(grid→grid) trait 与 `MedianCutQuantizer`（其唯一用途——`max_colors` 路径——被本变更取代；YAGNI，Gerstner 另案再引）。`max_colors==0` 仍在 `GreedyReducer::new` 复用 `InvalidImage{reason 含 "max_colors"}` 拒绝、不新增变体；`new` **内部先校验 `max_colors>=1`、后校验 palette**，故与旧版（配色前 `MedianCutQuantizer::new`）错误优先级一致——「非法 palette + `max_colors==0`」仍先命中 `max_colors` 的 `InvalidImage`。管线**先于配色 fail-fast 构造** `reducer`、配色后才 `reduce`（见 pipeline 规范），使该优先级在编排层落地。

**D5：确定性口径。**
`reduce` 是全函数（不返回 `Result`；`max_colors ≥ 已用珠色数` → 原样返回 no-op；空 pattern 原样返回）。**前置条件**：不 panic 保证要求 (1) `cells` 均为合法调色板下标、(2) pattern 是对**构造 reducer 所用同一 palette** 配色的产物（长度兼容但不同的 palette 会按错误坐标静默合并——调用方契约违反）。管线内 `pattern` 由 `match_pattern` 对同一 palette 产出、两点天然满足（下标合法是 `match_pattern` 的**输出后置条件**，非其入参前置——`match_pattern` 不吃 cell 下标）。越界下标的外部手搓 pattern **可 panic、不在保证内**：与配色后消费下标的 `count_colors`（越界项跳过、不 panic）不同，`reduce` 需按下标索引色彩快照 → 越界 panic，属有意分歧（越界 cell 无法有意义重映射）。RGB 度量整数 → 跨架构位精确，可钉整数 golden；Lab/Oklab 度量 f32 → 同机确定性（与既有 Lab/Oklab 匹配、`Triangle`/`Lanczos3` 缩放路径同级 canonical-only）。牺牲/目标平局规则固定 → 无随机、无 `HashMap` 顺序泄漏、无 `rayon`。

**D6：缩放默认滤镜 `Lanczos3 → Triangle`。**
仅改 `ResizeOptions::default().filter`；`filter` 仍可配、可显式回到 `Lanczos3`。理由：`image` crate 对降采样按缩放比放大滤波支持域，`Triangle` 无负瓣、天然面积平均，抹平 ±10 噪声与振铃。Triangle 亦为 f32，跨架构口径不变（端到端仍 canonical-only）。

## Risks / Trade-offs

- [破坏性：移除 `Quantizer`/`MedianCutQuantizer`，减色语义与顺序全变] → 走 OpenSpec 修改三个功能规范；重生成所有相关 golden；CLI `--max-colors` 用户语义保持不变。
- [Triangle 对大比例降采样可能比 Lanczos3 更软/损失锐度] → `filter` 保持可配；design 阶段用 UncleGao 目视对比 Triangle vs CatmullRom vs Lanczos3 后再定默认。
- [Lab/Oklab 减色 f32 → 非跨架构位精确] → 与既有 Lab/Oklab 匹配同级，golden 走 canonical-only；RGB 路径保持整数跨架构 golden。
- [贪心合并非全局最优] → 可接受；上限语义只要求 ≤N，且合并按感知最近，视觉足够；未来可另案上 k-means 精炼。
- [least-used 先吃「用量最少」珠色 → 小而显著的高光 / 点睛色被先合并] → 眼神光、单颗亮点等**面积占比极小但视觉权重高**的珠色，在低 `max_colors` 下会先于大面积背景色被并掉，视觉损失比其面积占比更大。可接受（贪心 + 纯用量排序的固有取舍，非本变更能根治；面积×显著性联合权重或 Gerstner 2012 联合优化留后续提案）；验收（tasks §6.2）用 UncleGao 显式检查高光/点睛色是否被过度合并、记录为已知取舍。

## Migration Plan

1. 加 `BeadReducer`/`GreedyReducer`，pipeline 顺序切到 `缩放 → 贴板 → 减色`。
2. 移除 `Quantizer`/`MedianCutQuantizer` 及其重导出与测试。
3. 改 `ResizeOptions::default().filter`。
4. 重生成 golden 基准，更新 golden-tests 夹具。
5. 回滚：还原上述提交即可（无持久化状态、无数据迁移）。

## Open Questions

- ~~默认滤镜取 `Triangle` 还是 `CatmullRom`~~ **已定：`Triangle`**（agreed scope；无负瓣、天然面积平均、抹平 ±10 噪声与振铃）。`filter` 仍可配，tasks §1.3 的目视对比是**确认 `Triangle` 达标**的验收步骤；若日后目视发现 `CatmullRom` 明显更优，另案调整默认、**不在本变更范围**。
- 减色是否需要暴露"距离空间独立于匹配器"的开关——暂不做（YAGNI），默认随匹配器。
- **[augment/SA 建议，本轮不做]** `quantizer` 模块在 `Quantizer` trait 移除后仅剩 `BeadReducer`/`GreedyReducer`，名实不符，可重命名为 `reducer`。本变更**不改**——属 M0 既定结构、三家 gating reviewer 未列 blocker；记为低成本可选后续（采纳需同步 ARCHITECTURE 模块列表与 tasks 中「quantizer 模块」措辞）。
