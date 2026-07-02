## 新增需求

### 需求:感知匹配器 OklabMatcher（Oklab + ΔEok²）
`OklabMatcher` 必须实现 `ColorMatcher` trait，用 **Oklab + 欧氏 ΔEok²**（Oklab 空间欧氏距离）返回调色板中到 `target` 感知最近的颜色下标。它是配色能力的**算法 Phase 3** 实现（与 `LabMatcher` 同档，见 INIT.md「Pattern Generation」），与 `RgbMatcher`（Phase 1）、`LabMatcher`（Phase 3）并存，并为引擎**默认**匹配器。

- **sRGB→Oklab 转换**：必须把 `[u8; 3]` 按 Oklab 标准管线转 `[L, a, b]`——分量 `/255` → sRGB 反 gamma 线性化（与 `srgb_to_lab` 第一步**相同**）→ linear sRGB → LMS（M1 矩阵，全正系数）→ 对 LMS 取**立方根**（裸 `cbrt`，**无** `6/29` 阈值分段、**无**白点归一化除法）→ LMS' → Oklab（M2 矩阵），全程用 `f32`。**禁止** `mul_add`/FMA（防 CLI 与 FFI 的 codegen 分叉）。
- **距离 = ΔEok²**：因仅取最近、`√` 单调保序，实现**禁止开方**，比较 Oklab 平方差之和（`(ΔL)²+(Δa)²+(Δb)²`），与 `RgbMatcher`/`LabMatcher` 的搜索骨架同构。
- **平局取最低下标**：多个调色板色与 `target` 的 Oklab 平方距离相等时，必须返回**遍历中最低的下标**（严格 `<` 更新最优、相等不更新），固定且确定。
- **构造守卫**：`OklabMatcher::new(&Palette) -> Result<OklabMatcher, BeadError>` 必须在构造时把调色板各色**一次性**转 Oklab 存入顺序保持的快照（下标 `i` ≡ `palette.colors[i]`），并复用与 `RgbMatcher`/`LabMatcher` **相同**的守卫（`check_palette_len`）：空 `colors` → `InvalidPalette`（`reason` 含 "no colors"）；`colors.len() > 65536` → `InvalidPalette`（`reason` 含 "more than"）；边界精确（`len == 65536` 合法、`65537` 拒绝）。**禁止**新增 `BeadError` 变体。
- **热路径全函数**：`find_best_match` 不返回 `Result`、不 `panic`；对有界 `u8` 输入，M1 全正系数 × 非负线性 rgb ⇒ LMS≥0 ⇒ `cbrt` 有限，转换全程为有限值，**不产 NaN**。

#### 场景:精确命中调色板色
- **当** 某像素的 RGB 恰好等于调色板中某色的 RGB
- **那么** `OklabMatcher::find_best_match` 返回该色的下标（Oklab 平方距离 0）；若多个调色板色共享同一 RGB，按平局规则返回其中**最低下标**

#### 场景:蓝紫区取感知最近且可与 Lab 不同
- **当** 在蓝/紫区取一个不在调色板中的像素，分别用 `OklabMatcher` 与 `LabMatcher` 匹配同一调色板
- **那么** `OklabMatcher::find_best_match` 返回 Oklab 平方差之和最小的调色板色下标；对存在的输入，该结果**可以**与 `LabMatcher` 在同一像素的结果**不同**（证明确为 Oklab 匹配、非 Lab 别名）

#### 场景:Oklab 平局取最低下标
- **当** 两个调色板色到某像素的 Oklab 平方距离相等
- **那么** `OklabMatcher::find_best_match` 返回**下标较小**者，且重复调用结果一致

#### 场景:构造拒绝非法调色板
- **当** 用空 `colors` 的 `Palette` 调 `OklabMatcher::new`
- **那么** 返回 `Err(InvalidPalette)`，`reason` 含 "no colors"，不 panic
- **且** `colors.len() == 65537` 时返回 `Err(InvalidPalette)`（`reason` 含 "more than"），而 `len == 65536` 必须**成功**

### 需求:默认匹配器为 OklabMatcher 且 matcher 可选
配色能力必须支持在 `RgbMatcher`/`LabMatcher`/`OklabMatcher` 三者间选择，选择项由 `MatcherKind { Rgb, Lab, Oklab }` 枚举表达。`MatcherKind` 必须提供 `Default`，且 `Default == Oklab`——即引擎**默认匹配器为 `OklabMatcher`**（取代原 `LabMatcher` 默认）。`LabMatcher` 降为可选备选（`MatcherKind::Lab`），`RgbMatcher` 继续作跨架构整数基准并可经 `MatcherKind::Rgb` 选用。三个 matcher 必须共享同一搜索骨架（最近色、严格 `<` 最低下标平局），仅距离度量与色彩空间不同。

#### 场景:默认选用 Oklab
- **当** 未显式指定 matcher（取 `MatcherKind::default()`）
- **那么** 配色使用 `OklabMatcher`；同 `PixelGrid` + `Palette` 下 `cells` 与用 `LabMatcher` 的结果**可以不同**（默认已从 Lab 翻为 Oklab）

#### 场景:三个 matcher 均可显式选用
- **当** 分别指定 `MatcherKind::Rgb`/`Lab`/`Oklab`
- **那么** 配色分别使用 `RgbMatcher`/`LabMatcher`/`OklabMatcher`，各自满足其最近色与平局规则

## 修改需求

### 需求:确定性（含跨架构整数一致）
配色必须确定：同一 `PixelGrid` 与同一 `Palette` 在**同机 / 同平台 + 同依赖版本**下必须产生逐字节相同的 `BeadPattern`。实现**禁止**引入非确定性来源——`rayon` 并行、随机、迭代顺序泄漏；距离度量与平局规则必须固定。跨架构（arm64 / x86_64）一致性**按匹配器分两档**：

- **`RgbMatcher`（算法 Phase 1，纯整数）**：匹配全程为整数运算（平方欧氏、无 `sqrt`、无 `f32`），故跨架构必须逐字节一致——这是数学保证，据此钉一份硬编码的跨架构位精确 golden（不像 `Lanczos3` f32 只能用同进程重算 + Nearest）。**禁止**在 `RgbMatcher` 引入浮点。
- **感知匹配器 `LabMatcher`（CIELAB + ΔE76）与 `OklabMatcher`（Oklab + ΔEok²）（均算法 Phase 3）**：色彩空间转换与距离用浮点（`f32`），其 `cbrt`/`powf` 跨 libm 实现**不保证逐位一致**，故配色结果**跨架构不保证位精确**——这与 `Lanczos3` 的 `f32::sin` 同构。二者属**同一确定性档**：确定性按既有 **canonical 平台模型**保证：canonical = arm64 Linux 承担四样产物的字节 golden（arm64 Linux 是可稳定 bless 的**回归基准、非生产字节保真**——iOS/Android libm 各异，见 `tests/golden/README.md`），其它平台只断言 float-independent 结构不变量；「CLI == FFI」是**同机**逐字节闸门（同机同 libm → 同结果），不受跨架构浮点影响。浮点是感知匹配器**唯一**允许的非确定性面；两者均**禁止** `mul_add`/FMA。

#### 场景:重复匹配一致
- **当** 对同一 `PixelGrid` + 同一 `Palette`（同机 / 同平台）多次调用 `match_pattern`（无论用 `RgbMatcher`、`LabMatcher` 或 `OklabMatcher`）
- **那么** 每次返回的 `BeadPattern` 完全相等（含 `cells` 顺序）

#### 场景:RgbMatcher 跨架构位精确 golden
- **当** 用 **`RgbMatcher`** 对一个固定小 `PixelGrid`（含精确命中 / 等距平局 / 离色取最近三类格）+ 固定小调色板匹配
- **那么** `cells` 等于硬编码的期望 `Vec<u16>`，且该断言在 arm64 与 x86_64 上都通过

#### 场景:感知匹配器同机重复一致
- **当** 用 **`LabMatcher`** 或 **`OklabMatcher`** 对同一 `PixelGrid` + 同一 `Palette` 在**同一机器**多次匹配
- **那么** 每次返回的 `cells` 逐字节相等（跨架构位精确**不**在保证范围内，由 canonical arm64 golden 承担）
