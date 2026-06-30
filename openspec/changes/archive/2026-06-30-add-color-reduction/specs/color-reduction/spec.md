# color-reduction 规范（增量）

## ADDED Requirements

### Requirement: Quantizer trait（grid→grid 降色缝）
`bead-core` MUST 提供 `quantizer` 模块，含 object-safe 的 **`Quantizer` trait**：`fn quantize(&self, grid: &PixelGrid) -> PixelGrid`。它把一个 `PixelGrid` 降为**不同色数受限**的 `PixelGrid`（`width`/`height` 原样保留），是配色**前**的可选阶段。trait 必须保持 object-safe（`&dyn Quantizer` / `Box<dyn Quantizer>`，无泛型方法、无 `Self`-返回、无关联类型）——与 `matcher::ColorMatcher` 同缝法。`quantize` 是**全函数**：不返回 `Result`、不 panic；产出 `PixelGrid` 满足 `pixels.len() == width*height`、与入参形状一致。**空网格**（`pixels.len()==0`，即 `width==0` 或 `height==0`）必须**原样返回、不 panic**（由 `MedianCutQuantizer` 的 step 0 short-circuit 保证——0 不同色 ≤ 任意 `max_colors`，不计算均值、不触发 `sum/count` 除零）。`bead-core` 必须从 crate 根**重导出** `Quantizer` / `MedianCutQuantizer`（同 `matcher` 类型的暴露体例）。

#### Scenario: 量化产出形状一致的 PixelGrid
- **当** 对一个 `w×h` 的 `PixelGrid` 调用某 `Quantizer::quantize`
- **那么** 返回的 `PixelGrid` 满足 `width==w`、`height==h`、`pixels.len()==w*h`，且其**不同色数 ≤ 该量化器配置的上限**

#### Scenario: 空网格原样返回不 panic
- **当** 对 `pixels.len()==0` 的 `PixelGrid`（`width==0` 或 `height==0`）调用 `quantize`
- **那么** 原样返回该空网格、**不 panic**（不计算任何 `sum/count`）

### Requirement: MedianCutQuantizer 构造与 max_colors 校验
`MedianCutQuantizer::new(max_colors: u32) -> Result<MedianCutQuantizer, BeadError>` MUST 在构造时校验 `max_colors >= 1`：`max_colors == 0` → 返回 `Err(BeadError::InvalidImage { reason })`（**复用零维度同变体**——`max_colors` 是无效生成参数，`reason` 含 "max_colors"，**禁止**新增变体）。`max_colors >= 1` → `Ok`。构造后量化配置不可变（值语义，同 `RgbMatcher` 快照）。`new` 禁止 panic。

#### Scenario: max_colors 为 0 被拒绝
- **当** 以 `max_colors == 0` 调 `MedianCutQuantizer::new`
- **那么** 返回 `Err(BeadError::InvalidImage { reason })`（`reason` 含 "max_colors"），**不 panic**、**不**新增错误变体
- **且** `max_colors >= 1` 时返回 `Ok`

### Requirement: RGB Median Cut 确定性算法
`MedianCutQuantizer::quantize` MUST 用 **RGB Median Cut**，规则**全部确定、纯整数**（无随机、无浮点、无依赖排序稳定性的偶然）：
- 量化对象为网格的**像素数组**（含重复，频率参与）。
- **step 0 short-circuit**：先统计网格**不同色数** `d`（确定性 sort+dedup 或 count-only，无随机/无顺序泄漏）。若 `d <= max_colors`（**含空网格 `d==0`**）→ **原样返回输入网格**、**不**进 Median Cut（保证 no-op 精确，且空网格不触发下方 `sum/count` 除零）。
- 否则初始一个桶 = 全部像素，循环：当 `桶数 < max_colors` 且存在可分裂桶（像素数 ≥ 2 且非全同色）时——① 选「最大单通道展布 `max-min`」的桶+通道，**平局取桶下标较小者、再按 R<G<B 通道序**；② 把该桶像素按**严格全序键 `(选定通道值, R, G, B, 原始行优先下标)`** 排序（末键 = 像素在原网格的行优先下标，唯一 → **真·全序、与排序稳定性无关**；同色像素互换不影响桶均值/计数，末键只为硬编码 golden 实现无关）；③ 在**中位下标 `len/2`** 切为下/上两桶，**下半桶原位替换被选桶 `i`、上半桶插入 `i+1`**（其余桶右移），桶序确定 → ① 的「桶下标」平局确定、硬编码 golden 实现无关。
- 每桶**代表色 = 分量均值**：逐通道 `sum: u64 / count` 整数除法截断（**u64 累加器**，防大网格 `255·N > u32::MAX` 溢出）。
- 每像素映射到其所在桶的代表色，产出新 `PixelGrid`。
- 全程 `u8` 色 + 整数计数 + u64 均值，**无 `f32`、无 `sqrt`**。

#### Scenario: 固定输入产出确定的代表色网格
- **当** 对一个固定小 `PixelGrid`（含可触发分裂的多色像素）以固定 `max_colors` 量化
- **那么** 产出的 `pixels` 等于**硬编码的期望网格**，且重复调用结果一致

### Requirement: ≤N 上限语义与自然 no-op
量化输出的**不同色数 MUST ≤ `max_colors`**（「上限」语义，非「恰好 N」——两桶代表色相同时更少）。当**网格不同色数 `d` ≤ `max_colors`** 时，由 step 0 short-circuit **原样返回输入、逐像素不改**（**保证的 no-op**，**不**依赖「桶降单色」涌现行为——后者在偏态分布 + median-index 切分下**不成立**）。⚠️ no-op 触发条件是**网格的不同色数 `d`**、**非调色板色数**：量化在配色**前**跑于原始像素，照片网格 `d` 常远超调色板 199 色，故 **`max_colors ≥ 199` 不蕴含 no-op**（调色板规模只是最终拼豆色数的天花板，不是量化 no-op 触发条件）。`max_colors == 1` 合法：单桶、全图均值色。

#### Scenario: n 不小于不同色数时不改色
- **当** 对一个不同色数为 `k` 的 `PixelGrid`，以 `max_colors >= k` 量化
- **那么** 输出 `PixelGrid` 与输入**逐像素相同**（自然 no-op）

#### Scenario: n 小于不同色数时上限成立
- **当** 对不同色数为 `k` 的 `PixelGrid`，以 `1 <= max_colors < k` 量化
- **那么** 输出的**不同色数 ≤ max_colors**

### Requirement: 量化确定性（纯整数跨架构位精确）
同一 `PixelGrid` 与同一 `max_colors` MUST 产生**逐字节相同**的量化 `PixelGrid`。实现**禁止**引入非确定性来源（随机、`rayon` 并行、`HashMap`/`HashSet` 迭代顺序泄漏、浮点）。因量化全程整数（展布、排序键、中位切、均值截断），跨架构（arm64 / x86_64）**必须逐字节一致**——这是数学保证，据此可钉一份跨架构位精确的量化器**单元** golden（grid→grid，任平台字节断言；区别于经 `Lanczos3`/`LabMatcher` 浮点的**端到端**路径——后者仍 canonical-only）。

#### Scenario: 重复量化与跨架构位精确
- **当** 对同一 `PixelGrid` + 同一 `max_colors` 多次量化
- **那么** 每次输出完全相等；且对固定小输入，输出等于硬编码期望网格，该断言在 arm64 与 x86_64 上都通过
