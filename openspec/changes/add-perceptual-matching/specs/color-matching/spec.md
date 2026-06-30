# color-matching 规范（增量）

## 新增需求

### 需求:感知匹配器 LabMatcher（CIELAB + ΔE76）
`LabMatcher` 必须实现 `ColorMatcher` trait，用 **CIELAB + ΔE76**（Lab 空间欧氏距离）返回调色板中到 `target` 感知最近的颜色下标。它是配色能力的**算法 Phase 3** 实现（见 INIT.md「Pattern Generation」），与 `RgbMatcher`（Phase 1）并存。

- **sRGB→Lab 转换**：必须把 `[u8; 3]` 按标准管线转 L\*a\*b\*——分量 `/255` → sRGB 反 gamma 线性化 → XYZ（sRGB / D65）→ L\*a\*b\*（含 `6/29` 阈值分段与立方根），用 `f32`。
- **距离 = ΔE76**：ΔE76 是 Lab 空间欧氏距离；因仅取最近、`√` 单调保序，实现**禁止开方**，比较 Lab 平方差之和（`(ΔL)²+(Δa)²+(Δb)²`），与 `RgbMatcher` 的搜索骨架同构。
- **平局取最低下标**：多个调色板色与 `target` 的 Lab 平方距离相等时，必须返回**遍历中最低的下标**（严格 `<` 更新最优、相等不更新），固定且确定。
- **构造守卫**：`LabMatcher::new(&Palette) -> Result<LabMatcher, BeadError>` 必须在构造时把调色板各色**一次性**转 Lab 存入顺序保持的快照（下标 `i` ≡ `palette.colors[i]`，承载最低下标平局规则），并复用与 `RgbMatcher` **相同**的守卫：空 `colors` → `InvalidPalette`（`reason` 含 "no colors"）；`colors.len() > 65536` → `InvalidPalette`（`reason` 含 "more than"，防 `index as u16` 截断）；边界精确（`len == 65536` 合法、`65537` 拒绝）。**禁止**新增 `BeadError` 变体。
- **热路径全函数**：`find_best_match` 不返回 `Result`、不 `panic`；对有界 `u8` 输入，Lab 转换全程为有限值，**不产 NaN**。

#### 场景:精确命中调色板色
- **当** 某像素的 RGB 恰好等于调色板中某色的 RGB
- **那么** `LabMatcher::find_best_match` 返回该色的下标（Lab 平方距离 0）；若多个调色板色共享同一 RGB，按平局规则返回其中**最低下标**

#### 场景:离色取感知最近（可与 RGB 最近不同）
- **当** 某像素不在调色板中
- **那么** `LabMatcher::find_best_match` 返回 Lab 平方差之和最小的调色板色下标；对存在的输入，该结果**可以**与 `RgbMatcher` 在同一像素的结果不同（证明确为感知匹配、非 RGB 距离别名）

#### 场景:Lab 平局取最低下标
- **当** 两个调色板色到某像素的 Lab 平方距离相等
- **那么** `LabMatcher::find_best_match` 返回**下标较小**者，且重复调用结果一致

#### 场景:构造拒绝非法调色板
- **当** 用空 `colors` 的 `Palette` 调 `LabMatcher::new`
- **那么** 返回 `Err(InvalidPalette)`，`reason` 含 "no colors"，不 panic
- **且** `colors.len() == 65537` 时返回 `Err(InvalidPalette)`（`reason` 含 "more than"），而 `len == 65536` 必须**成功**

## 修改需求

### 需求:确定性（含跨架构整数一致）
配色必须确定：同一 `PixelGrid` 与同一 `Palette` 在**同机 / 同平台 + 同依赖版本**下必须产生逐字节相同的 `BeadPattern`。实现**禁止**引入非确定性来源——`rayon` 并行、随机、迭代顺序泄漏；距离度量与平局规则必须固定。跨架构（arm64 / x86_64）一致性**按匹配器分两档**：

- **`RgbMatcher`（算法 Phase 1，纯整数）**：匹配全程为整数运算（平方欧氏、无 `sqrt`、无 `f32`），故跨架构必须逐字节一致——这是数学保证，据此钉一份硬编码的跨架构位精确 golden（不像 `Lanczos3` f32 只能用同进程重算 + Nearest）。**禁止**在 `RgbMatcher` 引入浮点。
- **`LabMatcher`（算法 Phase 3，CIELAB + ΔE76）**：Lab 转换与距离用浮点（`f32`），其 `cbrt`/`powf` 跨 libm 实现**不保证逐位一致**，故配色结果**跨架构不保证位精确**——这与 `Lanczos3` 的 `f32::sin` 同构。其确定性按既有 **canonical 平台模型**保证：canonical = arm64 Linux 承担四样产物的字节 golden（arm64 Linux 是可稳定 bless 的**回归基准、非生产字节保真**——iOS/Android libm 各异，见 `tests/golden/README.md`），其它平台只断言 float-independent 结构不变量；「CLI == FFI」是**同机**逐字节闸门（同机同 libm → 同结果），不受跨架构浮点影响。浮点是 `LabMatcher` **唯一**允许的非确定性面。

#### 场景:重复匹配一致
- **当** 对同一 `PixelGrid` + 同一 `Palette`（同机 / 同平台）多次调用 `match_pattern`（无论用 `RgbMatcher` 或 `LabMatcher`）
- **那么** 每次返回的 `BeadPattern` 完全相等（含 `cells` 顺序）

#### 场景:RgbMatcher 跨架构位精确 golden
- **当** 用 **`RgbMatcher`** 对一个固定小 `PixelGrid`（含精确命中 / 等距平局 / 离色取最近三类格）+ 固定小调色板匹配
- **那么** `cells` 等于硬编码的期望 `Vec<u16>`，且该断言在 arm64 与 x86_64 上都通过

#### 场景:LabMatcher 同机重复一致
- **当** 用 **`LabMatcher`** 对同一 `PixelGrid` + 同一 `Palette` 在**同一机器**多次匹配
- **那么** 每次返回的 `cells` 逐字节相等（跨架构位精确**不**在保证范围内，由 canonical arm64 golden 承担）
