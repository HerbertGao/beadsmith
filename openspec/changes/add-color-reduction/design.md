## 上下文

引擎把图像配到整份调色板（Artkal S = 199 色），生成的图案常需用户没有的颜色。现实拼豆套件色数有限（市场标准 24/36/48/72，高阶 108/216）。本变更加 `max_colors` 把输出限制到 ≤N 种拼豆色——算法 Phase 2（降色）。

现有引擎只有一条算法 trait `matcher::ColorMatcher`（M3 起，`add-perceptual-matching` 后默认 `LabMatcher`）。`pipeline::generate_pattern`（pipeline/mod.rs:84 一带）固定串 `image_to_grid → match_pattern → stats → render`，`GenerateOptions = {width,height,resize,render}`。OpenSpec 规则已预留 `quantizer` 模块名。

## 目标 / 非目标

**目标：**
- `max_colors: Option<u32>` 把图案输出限制到 ≤N 种拼豆色；`None` 完全向后兼容、默认路径**逐字节不变**。
- 引入第二条算法 trait `Quantizer`（复刻 `ColorMatcher` 缝法），首实现 RGB Median Cut。
- 确定性：默认路径不变（golden 不重 bless）；量化纯整数、跨架构位精确。
- 引擎 + CLI 先行，FFI 边界不动、无新依赖。

**非目标：**
- Lab/感知量化、K-Means（未来同 `Quantizer` trait 后替换）。
- FFI/App 暴露 `max_colors`（单独后续变更）。
- Inventory Mode 按「用户拥有色子集」约束配色（未来，建在本变更之上——本变更是「降到 N 个代表色」，不是「限定到指定子集」）。
- 抖动（Phase 4）；`rayon` 并行。

## 决策

**D1 — `Quantizer` trait：grid→grid 降色，构造期吃 max_colors。**
```
pub trait Quantizer { fn quantize(&self, grid: &PixelGrid) -> PixelGrid; }
pub struct MedianCutQuantizer { /* max_colors 等配置 */ }
impl MedianCutQuantizer { pub fn new(max_colors: u32) -> Result<Self, BeadError>; }
```
- 复刻 `ColorMatcher` 缝法：object-safe（`&dyn`/`Box<dyn>`），配置在 `new` 吃（像 `RgbMatcher::new(palette)`），`new` 校验 `max_colors >= 1`、否则 `Err(BeadError::InvalidImage { reason })`（`reason` 含 "max_colors"；复用零维度同变体——max_colors 是无效生成参数，**不新增变体**；`Some(0)` 的拒绝点）。`quantize` 是全函数（不返回 `Result`、不 panic；空网格经 D2 step 0 的 short-circuit 安全返回，**不**触发 `sum/count` 除零）。`bead-core` 从 crate 根**重导出** `Quantizer` / `MedianCutQuantizer`（同 matcher 类型暴露体例）。
- **grid→grid**（不是「palette 子集选择」）：量化降低**图像**的色数，产出仍是 RGB `PixelGrid`（≤N 个不同色）；随后 `match_pattern` 用默认 `LabMatcher` 把这 ≤N 个代表色映射到调色板。两阶段干净组合：量化管色数、匹配管感知映射。**理由**：palette 子集选择是组合优化（从 199 选 N 最优覆盖，NP），Median-Cut 图像量化是标准、确定、O(像素) 的懒做法；输出色数 ≤N 由「N 代表 → 各 1 调色板下标」保证。

**D2 — RGB Median Cut，确定性规则全钉死（无随机/无浮点）。**
量化对象 = 网格的**像素数组**（含重复，频率参与）。算法：
0. **先 short-circuit**：统计网格**不同色数** `d`（确定性 sort+dedup 或 count-only 集，无随机/顺序泄漏）。若 `d <= max_colors`（**含空网格 `d==0`**），**原样返回输入网格**、不进 Median Cut。这把「`n ≥ 不同色数` no-op」从涌现行为变成**保证**（D4），并使空网格/0 像素**不触发** step 3 的 `sum/count` 除零。
1. 否则初始一个桶 = 全部像素。
2. 当 `桶数 < max_colors` 且存在可分裂桶（像素数 ≥ 2 且非全同色）时循环：
   a. 选「最大单通道展布」的桶+通道：对每桶每通道算 `spread = max - min`；取最大者。平局 → **桶下标小者优先，再 R<G<B 通道序**。
   b. 把该桶像素按**严格全序键 `(选定通道值, R, G, B, 原始行优先下标)`** 排序——末键（像素在原网格的行优先下标）唯一，故是**真·全序、与排序稳定性无关**。（同色像素本可互换、不影响桶均值/计数；加末键只为让硬编码 golden 实现无关、消除「`(通道,R,G,B)` 非严格全序」的歧义。）
   c. 在**中位下标 `len/2`** 切成下/上两桶，**下半桶原位替换被选桶 `i`、上半桶插入 `i+1`**（其余桶右移）——桶序确定，故 a 的「桶下标」平局确定、硬编码 golden 实现无关。
3. 每桶代表色 = **分量均值**：逐通道 `sum: u64 / count` 整数除法截断（**u64 累加器**，防大网格 `255·N > u32::MAX` 溢出）。
4. 建映射：每像素 → 其所在桶的代表色；产出新 `PixelGrid`（`width`/`height` 原样）。
- 全整数（u8 色、计数、u64 均值截断）。`spread`、排序键、代表色均无 `f32`、无 `sqrt` → **跨架构位精确**（与 `RgbMatcher` 同性质）。
- 替代方案：代表色取众数/中位——均可，「均值截断」最简。**no-op 由 step 0 的 short-circuit 保证**，**不**依赖「桶降为单色即停」的涌现行为——后者在偏态分布 + median-index 切分下**不成立**（反例：`A×8,B×1`，`max_colors=4≥k=2`，预算耗在剥 `A` 半区，残桶 `[A,B]` 均值改色）。

**D3 — pipeline 可选阶段，`None` 恒等跳过。**
`generate_pattern`：`max_colors=None` → 完全不构造 quantizer、grid 原样进 `match_pattern`（**默认路径与现状逐字节相同**）；`Some(n)` → `MedianCutQuantizer::new(n)?`（`n==0` 在此 `?` 透传 Err）→ `quantize(&grid)` → 用量化后的 grid 进 `match_pattern`。其余链（stats/render/单一-Palette/错误透传）不变。

**D4 — 语义边界（市场数据驱动）。**
- `n ≥ 网格不同色数 d`（`d` = 量化对象**网格**的不同 RGB 数，**非调色板色数**）→ D2 step 0 的 short-circuit **原样返回输入**、逐像素不改（**保证的 no-op**）。⚠️ 量化跑在配色**前**的原始像素上，照片网格的 `d` 常远超调色板 199 色，故 **`n ≥ 199` 不蕴含 no-op**——no-op 只看网格 `d`，与调色板规模**无关**（调色板规模只是最终拼豆色数的天花板，不是量化 no-op 触发条件）。
- `n == 1` → 一个桶、全图均值色 → 合法（极端但不报错）。
- `n == 0` → `new` 拒绝（`InvalidImage`，`reason` 含 "max_colors"）。
- 引擎不内置 24/36/48/72 枚举——任意 `u32 ≥ 1`；档位仅 CLI help / 未来 UI 提示。

**D5 — 确定性 & golden。**
默认路径零变化 → 现有 golden master 不重 bless。**量化器单元**（grid→grid，纯整数）→ 可加一份**任平台**字节精确的单元 golden（不像 Lanczos3/LabMatcher 只能 canonical）。但 max_colors 的**端到端**路径仍夹在 Lanczos3（量化前 resize）+ 默认 LabMatcher（量化后 match）两个浮点段之间 → 端到端 `--max-colors` golden **与默认 golden 一样 canonical-only**。「跨架构位精确」**只对量化器单元成立、不对端到端路径**（proposal/tasks 据此修正）。

**D6 — FFI 不动，CLI 第 6 个可选 flag。**
`GenerateOptions` 加 `max_colors: Option<u32>`（`derive(Default)` → `None`，`..Default::default()` 兼容）。CLI `--max-colors <N>` 可选（clap `Option<u32>`）。FFI 边界（M8 width/height）**不改**、flutter-ffi 规范不动。

## 风险 / 权衡

- **Median Cut 代表色/分裂规则的确定性** → D2 把「选桶+通道、排序全序键、中位切、均值截断、平局序」全钉死，无随机/无浮点；单测含固定输入的硬编码期望（跨架构位精确）。
- **`n` 大于不同色数的退化** → D2 step 0 的 distinct-count short-circuit 保证 no-op（**不**依赖「桶降单色」涌现行为，那在偏态分布 + median-index 下被反例证伪）；单测覆盖 `n ≥ 不同色数`、`n==k`、空网格均不改色/不 panic。
- **均值截断 vs 众数代表色的观感** → 起步用均值（最简、no-op 性质好）；众数/中位是未来同算法内的可选精修，不影响 trait/边界。
- **量化在 RGB 而非 Lab** → RGB 量化对感知不是最优（蓝区等），但换来跨架构整数确定性 + 最简实现；Lab/感知量化留作未来同 `Quantizer` trait 后替换（与 matcher 的 RGB→Lab 演进同构）。
- **性能** → Median Cut 对 w×h 像素排序/切分，单线程 Phase 1 足够；`rayon` 属后期。

## 迁移计划

1. 落地 `quantizer` 模块（`Quantizer` + `MedianCutQuantizer` + 单测：固定输入硬编码期望、`Some(0)` 拒绝、`n≥不同色数` no-op、`n==1`、跨架构位精确 golden）。
2. `GenerateOptions` 加 `max_colors`；`generate_pattern` 插可选阶段 + `Some(0)` 透传；既有管线测试（默认 None）应全绿不变。
3. `lib.rs` 导出；CLI `--max-colors` flag + 传参；CLI 既有测试不变。
4. 默认 golden 不动；可加一份**量化器单元** golden（grid→grid，任平台字节断言、整数跨架构稳）；端到端 `--max-colors` golden 若加则 **canonical-only**（经 Lanczos3 + LabMatcher 浮点）。

**回滚**：`max_colors=None` 是默认，移除 quantizer 调用即回到现状；新模块可留不被调用。
