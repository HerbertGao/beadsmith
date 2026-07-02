# color-reduction 规范（增量）

## REMOVED Requirements

### 需求:Quantizer trait（grid→grid 降色缝）
**Reason**: 减色从配色前的 `PixelGrid` 量化改为配色后的 `BeadPattern` 珠色合并，grid→grid 的 `Quantizer` 缝不再使用。
**Migration**: 改用 `BeadReducer`(pattern→pattern) trait（见本规范 ADDED）。

### 需求:MedianCutQuantizer 构造与 max_colors 校验
**Reason**: `MedianCutQuantizer`（贴板前、调色板无关的 RGB median-cut）被移除。
**Migration**: 改用 `GreedyReducer::new(palette, matcher_kind, max_colors)`；`max_colors==0` 的拒绝语义（`InvalidImage`，reason 含 "max_colors"，不新增变体）在 `GreedyReducer::new` 保留。

### 需求:RGB Median Cut 确定性算法
**Reason**: median-cut 的桶均值代表色会"发明"非调色板中间色（去饱和灰质心），是背景塌灰的根源，整体算法被贪心珠色合并取代。
**Migration**: 见本规范 ADDED「贪心最少用量合并确定性算法」。

### 需求:量化确定性（纯整数跨架构位精确）
**Reason**: 减色现复用匹配器的感知度量，Lab/Oklab 路径引入 f32，全程纯整数的口径不再成立。
**Migration**: 见本规范 ADDED「减色确定性（RGB 整数跨架构 / Lab·Oklab 同机）」。

## ADDED Requirements

### 需求:BeadReducer trait（pattern→pattern 珠色减色缝）
`bead-core` MUST 提供 object-safe 的 **`BeadReducer` trait**：`fn reduce(&self, pattern: &BeadPattern) -> BeadPattern`。它接收一个已贴板的 `BeadPattern`（`cells` 为调色板索引），返回**同形状**（`width`/`height`/`cells.len()` 不变）、不同珠色数受限的新 `BeadPattern`；trait 必须可通过 `&dyn BeadReducer` / `Box<dyn BeadReducer>` 使用。`reduce` **不返回 `Result`**。**前置条件**：`reduce` 的「不 panic」保证要求传入 pattern 满足两点——(1) `cells` 每个值都是**合法调色板下标**（`< 构造时 palette.colors.len()`）；(2) 该 pattern 是对**构造 reducer 所用的同一 `palette`** 配色的产物（`GreedyReducer` 按其构造快照解释 `cells`；用**长度兼容但不同**的 palette 配出的 pattern 会按错误坐标合并、且**无可观测信号**——属调用方契约违反）。管线内 `pattern` 由 `match_pattern` 对**同一** `palette` 产出，两点天然满足——`match_pattern` 保证其**输出**下标合法（这是其**后置条件**，非对 `reduce` 的入参前置；`match_pattern` 本身不吃 cell 下标）。**越界下标**的外部手搓 pattern **不在**不 panic 保证内：与配色后消费下标的 sibling `count_colors`（越界项**跳过**、不 panic）**不同**，`reduce` 需按下标索引色彩快照，越界会 panic——这是 `reduce` 相对该 sibling 的**有意分歧**（越界 cell 无法被有意义地重映射）。**在前置条件内 `reduce` 是全函数、不 panic**。**空 pattern**（`cells.len()==0`）必须原样返回、不 panic。`bead-core` 必须从 crate 根**重导出** `BeadReducer` 与其实现 `GreedyReducer`。

#### 场景:减色产出形状一致的 BeadPattern
- **当** 对一个 `w×h` 的 `BeadPattern` 调用某 `BeadReducer::reduce`
- **那么** 返回的 `BeadPattern` 满足 `width==w`、`height==h`、`cells.len()==w*h`，且其**不同珠色数 ≤ 该 reducer 配置的上限**

#### 场景:空 pattern 原样返回不 panic
- **当** 对 `cells.len()==0` 的 `BeadPattern` 调用 `reduce`
- **那么** 原样返回该空 pattern、**不 panic**

### 需求:GreedyReducer 构造与 max_colors 校验
`GreedyReducer::new(palette: &Palette, matcher: MatcherKind, max_colors: u32) -> Result<GreedyReducer, BeadError>` MUST 在构造时**先校验 `max_colors >= 1`、后校验 palette**（顺序固定，决定错误优先级）：`max_colors == 0` → **立即**返回 `Err(BeadError::InvalidImage { reason })`（**复用零维度同变体**——`max_colors` 是无效生成参数，`reason` 含 "max_colors"，**禁止**新增变体），**先于任何 palette 校验**；`max_colors >= 1` 后再校验 palette——空调色板等既有 `Palette` 非法情形沿用匹配器同款守卫（复用 `InvalidPalette`，`reason` 含 "no colors" / "more than"）；两者都合法 → `Ok`。构造期按 `matcher` 指定的色彩空间**快照**每个调色板色的坐标——**复用匹配器同一份 srgb→Lab/Oklab 转换实现**（见「减色度量复用匹配器色彩空间转换」需求；同 `LabMatcher`/`OklabMatcher` 的一次性转换快照）；构造后配置不可变（值语义）。`new` 禁止 panic。

#### 场景:max_colors 为 0 被拒绝（先于 palette 校验）
- **当** 以 `max_colors == 0` 调 `GreedyReducer::new`（无论 palette 是否合法）
- **那么** 返回 `Err(BeadError::InvalidImage { reason })`（`reason` 含 "max_colors"），**不 panic**、**不**新增错误变体，且该拒绝**先于 palette 校验**——「非法 palette + `max_colors==0`」仍得 `InvalidImage`（而非 `InvalidPalette`）
- **且** `max_colors >= 1` 且 palette 合法时返回 `Ok`；`max_colors >= 1` 但 palette 非法（空 / 超 65536 色）时返回 `Err(InvalidPalette)`

### 需求:减色度量复用匹配器色彩空间转换（不另写转换副本）
`GreedyReducer` 的感知度量 MUST 与对应 `ColorMatcher` **同源**。漂移风险集中在 **srgb→色彩空间转换**（`cbrt` / 矩阵 / gamma，微妙易错），故该转换 MUST **字面复用匹配器同一份实现**、**禁止**在减色实现里另写副本：`bead-core` 内的 `srgb_to_lab`、`srgb_to_oklab`、二者共享的 `linearize`（sRGB 反 gamma 线性化）与调色板长度守卫 `check_palette_len` MUST 以 **`pub(crate)`** 可见性在 crate 内共享、供 `matcher` 与减色实现**复用同一函数**（非各写一份）。**距离**则按 `matcher` 用与对应 `ColorMatcher` **逐字同构的同一平方和公式**（各分量差平方和、**不开方**）：`Rgb` → 纯整数（累加 `u32`、无溢出，与 `RgbMatcher` 同）；`Lab` → ΔE76²、`Oklab` → ΔEok²（f32，**禁** `mul_add`/FMA，与匹配器同约束）。因平方和公式**平凡**（`Σ(Δ)²`、漂移风险可忽略），允许在减色内联、**不强制**抽成共享函数——强制共享的是转换、非平方和。减色的「最近」与配色的「最近」口径一致，由下面的**选择等价**场景守卫（只依赖已暴露的 `find_best_match`，无需暴露 palette-vs-palette 距离）。

#### 场景:减色「最近」与配色「最近」选择等价
- **当** 取一个牺牲色（某调色板色）与一组保留色（调色板色子集），用 `GreedyReducer`（某 `matcher`）在保留色中选出感知最近的目标色；同时以**同一 `matcher`** 对**仅含这组保留色的子调色板** `new`、并对牺牲色的 RGB 调 `find_best_match`
- **那么** 两者选出的目标色**一致**（转换共享、平方和同构；牺牲色作为调色板色，其快照坐标 == 其 RGB 经同一转换的结果）——此断言只依赖已暴露的 `find_best_match`，是「减色最近 == 配色最近」口径一致的**可执行**守卫（一致性测试，防转换实现漂移）

### 需求:贪心最少用量合并确定性算法
`GreedyReducer::reduce` MUST 用**贪心最少用量合并**，全部规则确定、无随机、无 `rayon`、无 `HashMap`/`HashSet` 迭代顺序泄漏：
- 统计 `pattern.cells` 中每个珠色索引的**用量**（出现次数）；"已用珠色"指用量 > 0 的索引，其个数记为 `d`。
- **short-circuit no-op**：若 `d <= max_colors`（含空 pattern `d==0`）→ **原样返回输入 pattern**、逐单元格不改。
- 否则循环，直到已用珠色数 ≤ `max_colors`：
  - **选牺牲色**：已用珠色中**用量最少**者；平局取**调色板下标较大者**（使低下标珠色更易存活，与"最低下标优先"精神一致）。
  - **选目标色**：在**其余仍在用**的珠色中，取与牺牲色**感知距离最近**者（距离度量由构造时 `matcher` 决定，见「减色确定性」需求，比较**平方距离**、不开方）；平局取**调色板下标最小者**（与匹配器一致）。
  - **合并**：把牺牲色的**所有单元格重映射到目标色**（牺牲色用量并入目标色，牺牲色用量归 0、退出"已用"集合）。
- 合并全程只在**真实珠色（调色板索引）**之间流动，**永不发明中间色**，故输出 `cells` 的每个值仍是合法调色板索引。

#### 场景:具体合并示例（RGB、牺牲色平局取下标大）
- **给定** 4 色调色板（下标 0..3）`c0=(0,0,0)`、`c1=(8,0,0)`、`c2=(255,0,0)`、`c3=(247,0,0)`，`matcher=Rgb`，`max_colors=2`
- **且** `cells = [0,0,0, 1, 2,2, 3]`（用量 c0=3、c2=2、c1=1、c3=1；已用珠色数 `d=4`）
- **当** `reduce`
- **那么** 第 1 轮牺牲色在最少用量（c1、c3 各 1）中**取下标较大者 c3**（牺牲平局规则），其目标为其余在用中 RGB 平方距离最近者 c2（`(255-247)²=64` 最小、非平局）→ c3 重映射为 c2 → `cells=[0,0,0,1,2,2,2]`（`d=3`）；第 2 轮牺牲色 c1（用量 1），最近目标 c0（`8²=64` < 到 c2 的 `247²`、非平局）→ c1 重映射为 c0 → **`cells=[0,0,0,0,2,2,2]`**（`d=2==max_colors`，停）；重复调用逐字节一致，每个值仍是合法调色板索引

#### 场景:目标色平局取下标小
- **给定** 3 色调色板 `c0=(0,0,0)`、`c1=(10,0,0)`、`c2=(20,0,0)`，`matcher=Rgb`，`max_colors=2`
- **且** `cells = [0,0, 2,2, 1]`（用量 c0=2、c2=2、c1=1；`d=3`）
- **当** `reduce`
- **那么** 牺牲色 c1（用量最少）到两个保留色**等距**（到 c0：`10²=100`，到 c2：`(10-20)²=100`）→ **目标平局取下标较小者 c0** → c1 重映射为 c0 → **`cells=[0,0,2,2,0]`**（`d=2`，停）；此例专为覆盖**目标色平局规则**（与上例的牺牲平局互补），重复调用一致

#### 场景:max_colors==1 合并到单一珠色
- **当** 已用珠色数 `d>1`、`max_colors==1`
- **那么** 循环合并直到只剩 **1 种**珠色，输出 `cells` 全为同一合法下标（重复调用一致）

#### 场景:合并只在真实珠色间发生
- **当** 减色发生
- **那么** 输出 `cells` 中出现的每个索引，在**输入 pattern 中都曾被使用**（不引入任何输入中未出现的新珠色，也不产生调色板外颜色）

### 需求:减色确定性（RGB 整数跨架构 / Lab·Oklab 同机）
同一 `BeadPattern` 与同一 `(palette, matcher, max_colors)` MUST 产生**逐字节相同**的减色 `BeadPattern`。减色距离度量**复用所选匹配器的度量**：
- `MatcherKind::Rgb` → RGB 平方欧氏（**纯整数**）；此路径减色跨架构（arm64/x86_64）**位精确**，可钉一份跨架构整数 golden。
- `MatcherKind::Lab` → ΔE76²、`MatcherKind::Oklab` → ΔEok²（**f32**）；此二路径为**同机确定性**（与既有 `LabMatcher`/`OklabMatcher`、`Lanczos3`/`Triangle` 缩放同级 canonical-only，非跨架构位精确）。
牺牲色/目标色的平局规则固定（见「贪心最少用量合并」），全程无随机、无并行、无哈希顺序泄漏。

#### 场景:重复减色可复现
- **当** 对同一 `BeadPattern` + 同一 `(palette, matcher, max_colors)` 多次减色
- **那么** 每次输出完全相等

#### 场景:RGB 路径跨架构位精确
- **当** 以 `MatcherKind::Rgb` 对固定小 `BeadPattern` 减色
- **那么** 输出等于硬编码期望 `cells`，该断言在 arm64 与 x86_64 上都通过

## MODIFIED Requirements

### 需求:≤N 上限语义与自然 no-op
减色输出的**不同珠色数 MUST ≤ `max_colors`**（「上限」语义，非「恰好 N」）。**`≤`（而非 `==`）的来源**：当**已用珠色数 `d` ≤ `max_colors`** 时由 short-circuit **原样返回输入 pattern、逐单元格不改**（**保证的 no-op**），此时输出不同珠色数就是 `d`（可 `< max_colors`）；而在**主动合并阶段**（`d > max_colors`）每次合并把牺牲色并入一个**已在用**的目标色、不同珠色数**恰好减 1**，故循环停在**恰好 `max_colors`**（合并进已用目标不会额外掉色——原「目标色重合时更少」措辞不实，已更正）。两情形综合即 `≤ max_colors`。⚠️ 语义变更：no-op 触发条件是**贴板后 pattern 的已用珠色数 `d`**（真实珠色数），**而非**旧版"原始像素网格的不同色数"。因减色现在配色**之后**跑于 `BeadPattern`，`d` 天然 ≤ 调色板色数（照片经匹配后已用珠色通常远少于 199），故 `max_colors` 直接就是"最终拼豆最多 N 种珠色"的上限。`max_colors == 1` 合法：把所有单元格合并到单一珠色。

#### 场景:n 不小于已用珠色数时不改
- **当** 对已用珠色数为 `k` 的 `BeadPattern`，以 `max_colors >= k` 减色
- **那么** 输出 `BeadPattern` 与输入**逐单元格相同**（自然 no-op）

#### 场景:全图单色 no-op
- **当** `cells` 全为同一下标（已用珠色数 `d==1`），以任意 `max_colors >= 1` 减色
- **那么** 由 short-circuit（`d==1 <= max_colors`）**原样返回**、逐单元格不改、不 panic

#### 场景:n 小于已用珠色数时上限成立
- **当** 对已用珠色数为 `k` 的 `BeadPattern`，以 `1 <= max_colors < k` 减色
- **那么** 输出的**不同珠色数 ≤ max_colors**
