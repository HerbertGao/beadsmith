## 为什么

引擎现在把图像配到**整份调色板**（自带 Artkal S = 199 色）。但现实拼豆套件颜色有限——市场标准档位是 **24 / 36 / 48 / 72**（入门到进阶），高阶才到 108 / 216。结果：生成的图案常常需要用户**根本没有**的颜色，无法照着摆。手作者真正要的是「**用我手上这 N 种色**配」——这是 INIT.md「Future Features」的头条 *Inventory Mode* 的算法基础，也是 ROADMAP 点名的 M9 后 **算法 Phase 2（降色）**。

本变更引入 `max_colors`：把图案输出限制到 **≤ N 种**拼豆色。

## 变更内容

- 新增 `bead-core` 的 **`quantizer` 模块**：`Quantizer` trait（object-safe，复刻 `matcher::ColorMatcher` 的缝法）+ 首个实现 `MedianCutQuantizer`（**RGB Median Cut**，纯整数）。
- `pipeline::generate_pattern` 新增**可选量化阶段**，插在 resize 与配色之间：`image_to_grid → 【quantize ≤N，仅当 max_colors=Some】→ match_pattern → stats → render`。`max_colors=None` 时**跳过该阶段（恒等）**，默认路径输出**逐字节不变**。
- `GenerateOptions` 新增 `max_colors: Option<u32>`（默认 `None`）。
- CLI `generate` 新增可选 flag **`--max-colors <N>`**（help 提示常见档位 24/36/48/72）。
- **FFI 边界本次不动**（保持 M8 极简 `width`/`height` 边界）；`max_colors` 引擎 + CLI 先行，App/FFI 暴露留作单独后续变更。

### 算法 Phase 声明

属 **算法 Phase 2（降色）**（INIT.md「Pattern Generation」）。Median Cut 起步；不跨档提前引入 Phase 3（已完成）外的其它档。

### 语义（`max_colors`）

- `None` → 全调色板配色（**现行默认**，向后兼容）。
- `Some(n)`，`n ≥ 1` → 先把网格量化到 **n 个代表色**（Median Cut），各代表色再经**默认 `LabMatcher`** 映射到最近调色板色 → 输出**至多 n 种**拼豆色（两代表撞同一调色板色时更少，是「上限」语义）。
- `Some(0)` → **拒绝**（`MedianCutQuantizer::new` 返 `BeadError::InvalidImage { reason }`，复用零维度同变体、reason 含 "max_colors"、不新增变体）。
- `n ≥ 网格不同色数 d` → 自然 no-op（**保证**：量化器先 short-circuit 比较 `d` 与 `n`，相等或更少则原样返回）。⚠️ `d` 是**网格**的不同 RGB 数，**非调色板色数**——量化跑在配色前的原始像素，照片网格 `d` 常远超调色板 199 色，故 **`n ≥ 199` 不蕴含 no-op**。
- 引擎**不**硬编码品牌档位枚举：`max_colors` 取任意 `u32 ≥ 1`，24/36/48/72 只是 CLI help / 未来 UI 的引导提示。

### 对确定性的影响

- **默认路径（`max_colors=None`）逐字节不变** → 现有 golden master **不需重 bless**。
- `MedianCutQuantizer` 是 **RGB 纯整数 Median Cut**（桶切分、固定分裂规则、u64 均值、无随机、无浮点）→ 量化器**单元**输出（grid→grid）**跨架构位精确**（与 `RgbMatcher` 同性质，强于 `LabMatcher` 的浮点）→ 量化器**单元 golden** 可任平台字节断言。但 max_colors 的**端到端**路径仍夹在 Lanczos3 + 默认 LabMatcher 两浮点段间，**端到端 golden 仍 canonical-only**（同默认 golden）；「跨架构位精确」**只对量化器单元、不对端到端路径**。
- 后续 `match_pattern` 仍用默认 `LabMatcher`（浮点，同 ΔE76 那档的确定性口径）：max_colors 路径整体确定性 = 量化整数 + 匹配浮点，同机/同平台逐字节、跨架构由 canonical-arm64 模型吸收（与现状一致）。

## 非目标（Non Goals）

- 不做 Lab/感知量化、K-Means 或抖动；本次只落 RGB Median Cut 降色。
- 不暴露 FFI/App 的 `max_colors` 边界；引擎 + CLI 先行。
- 不实现 Inventory Mode 的「用户拥有色子集」约束配色；本次只限制代表色数量。
- 不新增并行化或依赖。

## 功能 (Capabilities)

### 新增功能
- `color-reduction`: `Quantizer` trait + `MedianCutQuantizer`（RGB Median Cut）+ `max_colors` 语义（≤N、Some(0)→InvalidImage、`n≥网格不同色数` 经 short-circuit 保证 no-op、空网格安全、纯整数跨架构确定性）。

### 修改功能
- `pipeline`: `generate_pattern` 新增可选量化阶段（`max_colors=None` 时恒等跳过、默认输出不变）；`GenerateOptions` 新增 `max_colors: Option<u32>` 字段；错误透传含 `Some(0)` 拒绝。
- `cli`: `generate` 新增可选 flag `--max-colors <N>`（第 6 个 flag，可选；`cell_size`/`filter` 仍仅默认、不暴露）。

## 影响

- **代码**：
  - 新增 `crates/bead-core/src/quantizer/mod.rs`（`Quantizer` trait + `MedianCutQuantizer` + 单测）。
  - `crates/bead-core/src/pipeline/mod.rs`：`GenerateOptions` 加 `max_colors`；`generate_pattern` 插可选量化阶段 + `Some(0)` 守卫。
  - `crates/bead-core/src/lib.rs`：从 crate 根**重导出** `quantizer` 的 `Quantizer`/`MedianCutQuantizer`；并更新 `InvalidImage` 的文档注释（现仅列图像维度场景，补 `max_colors==0`）。
  - `crates/bead-cli/src/main.rs`：`generate` 加 `--max-colors` 可选参数，传入 `GenerateOptions.max_colors`。
  - `tests/golden/`：默认路径不变；**可选**新增一个**量化器单元** golden（grid→grid、任平台字节稳）；端到端 `--max-colors` golden 若加则 canonical-only。
- **文档**：`ARCHITECTURE.md:138-149` quantizer 段标注 Median Cut 已实现（`MedianCutQuantizer`），与 matcher 段 `LabMatcher`「— implemented as the default」体例一致（仓库约定，见 commit b49201b / matcher 先例）。
- **API**：`generate_pattern` 签名不变（仅 `GenerateOptions` 加可选字段，`..Default::default()` 兼容）；CLI 加一个可选 flag；**FFI 边界不变**。
- **依赖**：**无新增**。Median Cut 是纯整数桶切分，仅用 std（排序/分区）。`rayon` 仍不引入。
- **里程碑**：M9 后的算法 Phase 2 工作；引入 `bead-core` 第二条算法 trait（`Quantizer`），验证「算法挂 trait」架构的第二维度。
