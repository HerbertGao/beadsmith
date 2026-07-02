# gerstner-superpixel 规范

## 目的
定义 `Gerstner` 可选生成模式的前段契约：确定性 SLIC 变体超像素（实数 per-axis 步长、原始网格锚定候选、明确 round-0 质心、快照式更新、固定累加序、上采样守卫）将裁剪源图联合降采样并分配到 `w×h` 输出 cell，再以 Oklab-argmin 贴到固定珠板产全板 `BeadPattern`；`max_colors` 复用 `GreedyReducer`。同机 canonical 确定性（f32、非跨架构位精确），永不发明中间色。生成模式选择 `GeneratorKind` 落在 `pipeline`。
## 需求
### 需求:Gerstner 超像素确定性算法
`Gerstner` 前段 MUST 用**确定性 SLIC 变体**把裁剪后的源图降采样并分配到 `w×h` 个输出 cell，产出全板 `BeadPattern`。全部规则确定、**无随机、无 `rayon`、无 `HashMap`/`HashSet` 迭代顺序泄漏、无 `mul_add`/FMA**。下列每一项都是确定性所必需、MUST 逐条落地：

- **裁剪与步长（实数、per-axis）**：先 `crop_center` 到目标宽高比得源区 `W×H`（**与 Staged 用同一 `crop_center`、同参**）。步长 `S_x = W/w`、`S_y = H/h`，均为 **`f32` 实数、不取整**。**要求 `W >= w` 且 `H >= h`**（即 `S_x,S_y >= 1`）；否则（目标大于源 / 上采样）Gerstner MUST 返回 `Err(BeadError::InvalidImage { reason })`（reason 点名「Gerstner 需 target ≤ source」），**不**进入退化路径（见「Gerstner 上采样守卫」需求）。
- **种子 ↔ cell 一一对应，初始质心明确**：cell `(i,j)`（`0≤i<w, 0≤j<h`，种子下标 `k = j*w + i`，行优先）对应一个种子；**初始位置** = 源坐标 `((i+0.5)·S_x, (j+0.5)·S_y)`（`f32`）；**初始（round-0）质心** = 该中心**最近整数像素** `(round(x), round(y))`（`round` **MUST 固定为 Rust `f32::round` 语义（ties away from zero）**；再 clamp 到 `[0,W)×[0,H)`）的 Oklab 坐标（复用 `pub(crate) srgb_to_oklab`/`linearize`）+ 其**位置**。**位置表示约定 MUST 统一**：质心存**原始源像素坐标**（非归一化）；距离计算（见下）时才对 `Δ位置` 按 `S_x`/`S_y` 归一——避免二次归一。共 `w·h` 个种子，恒定不增删。
- **固定迭代次数 `T`**：迭代**恰 `T` 轮**后停，**禁止**任何依赖阈值 / 残差 / 收敛判据的提前停。
- **每轮 = 快照式「先全量分配、后全量更新质心」**：本轮分配**只读上一轮质心快照**、写入独立分配缓冲；**分配途中禁止改动任何质心**；分配全部完成后再由本轮分配**重算**所有质心。
- **候选集按固定原始网格锚定（覆盖不随种子漂移失效）**：像素 `p` 的**原始网格 cell** = `(floor(p.x/S_x), floor(p.y/S_y))`（clamp 到 `[0,w)×[0,h)`）；`p` 的**候选种子集** = 其原始网格 cell **及 8-邻网格 cell** 对应的种子（至多 9 个，**按 cell 下标、与漂移后的种子位置无关**）。该集合由 `p` 的原始网格归属**唯一决定**，故 `p` **恒有 ≥1 候选**（自身 cell 的种子）→ **必被分配到恰好一个超像素、无漏**（即便 `T>1` 种子漂移后依然成立）。
- **分配**：在候选集中取**距离最近**的种子；距离 `= ΔOklab² + m²·((Δx/S_x)² + (Δy/S_y)²)`（**per-axis 归一位置**、`m`=compactness 常量、不开方）；**平局取最小种子下标 `k`**。
- **质心更新（f32 累加顺序 MUST 固定）**：对**整源图行优先单遍**，按每像素的 assignment 把其 Oklab 坐标与位置**累加**到以种子下标索引的 `Vec` 累加器，再除以簇大小。**遍历序（行优先）与归并结构（`Vec` 索引）固定** → 每个簇的 f32 累加顺序确定（f32 加法非结合，顺序不定即破坏同机确定性）；**禁** `HashMap` 聚合。
- **空簇**：某种子本轮分配到 **0 像素** → **保留其上一轮质心**（不移动、不重播、不删除）；round-1 即空的种子保留其**已明确的 round-0 初始质心**。
- **收敛后**每个 cell 代表色 = 其超像素簇的第 `T` 轮 **Oklab 质心**。

`m`、`T` 为**固定编译期常量**（数值实现期目视调优、一经确定即固定、非运行时输入、不进 CLI）。`S_x=W/w`、`S_y=H/h` 是**运行期由裁剪源 `W×H` 与目标网格 `w×h` 导出的确定值**（**非编译期常量**，但对同输入唯一确定）——其**计算公式固定**（`W/w`、`H/h`、`f32` 实数），是确定性的一部分。

#### 场景:重复生成逐字节相等
- **当** 对同一 `(image_bytes, palette, opts)` 以 `Gerstner` 在**同机**多次生成
- **那么** 每次 `BeadPattern` 完全相等（实数步长 + 明确 round-0 质心 + 原始网格锚定候选 + 快照式更新 + 固定累加序 + 固定平局 + 恰 T 轮 → 无非确定性来源）

#### 场景:每个源像素必被分配（原始网格锚定，漂移后无漏）
- **当** 任一轮（含 `T>1` 种子已漂移）分配完成
- **那么** 每个源像素**恰属于一个**超像素——其候选集由**原始网格归属**决定、含自身 cell 的种子，故恒 ≥1 候选、无漏、无歧义（最近 + 最小下标）

#### 场景:空簇保留质心、种子数恒定
- **当** 某轮某种子分配到 **0 像素**（含 round-1 即空）
- **那么** 该种子保留上一轮质心（round-1 空则保留明确的 round-0 初始质心）、不重播 / 不删除；种子总数恒为 `w·h`、`cells.len()==w*h` 不变

#### 场景:分配平局取最小种子下标
- **当** 某像素到候选集中两个种子的 5D 距离相等
- **那么** 分配到**下标较小**（`k = j*w+i` 行优先序）的种子，重复一致

#### 场景:恰 T 轮、无提前停
- **当** 生成运行
- **那么** 迭代**恰 `T` 轮**（无残差/阈值提前停）——固定轮数是确定性的一部分，任何早停都禁止

### 需求:Gerstner 上采样守卫（target ≤ source）
`Gerstner` MUST 在裁剪后校验 `W >= w && H >= h`（源区不小于目标网格、`S_x,S_y >= 1`）；**不满足**（目标大于源）时返回 `Err(BeadError::InvalidImage { reason })`（reason 点名 Gerstner 上采样约束），**不 panic**、**不**新增变体、**不**进入 `S<1` 退化路径（`S<1` 会令多种子塌到同像素、窗口退化、大量空簇，视觉与语义均无意义）。`Staged` 路径**不受**此约束（其 `image_to_grid` 仍允许放大）。

#### 场景:Gerstner 目标大于源被拒绝
- **当** 以 `GeneratorKind::Gerstner`、目标 `w×h` 大于裁剪后源区（任一维 `S<1`）生成
- **那么** 返回 `Err(BeadError::InvalidImage { reason })`（reason 含 Gerstner target/source 约束），**不 panic**、**不**新增变体

### 需求:palette-constrained 贴板（Oklab argmin，永不发明中间色）
`Gerstner` 每个 cell 的 Oklab 簇质心 MUST 贴到**固定珠板**中**感知最近**的珠色，得全板 `BeadPattern`。贴板 = **对珠板的 Oklab 快照做 ΔEok² argmin**：`(ΔL)²+(Δa)²+(Δb)²`、不开方、严格 `<` 更新、平局取**最低下标**——**与 `OklabMatcher` 同规则、同 `pub(crate) srgb_to_oklab` 快照**，但**入参是 Oklab 坐标（簇质心）而非 RGB**，故是一个**独立的 Oklab-坐标 argmin**（**不是** `find_best_match`，后者只吃 `[u8;3]` RGB）。迭代中 **MUST NOT** 优化或发明珠板外颜色——输出 `cells` 每个值都是**输入珠板的合法下标**（延续 palette-aware-reduction 的「永不发明中间色」）。

#### 场景:输出色都在珠板内
- **当** `Gerstner` 生成完成
- **那么** `cells` 中每个下标都 `< palette.colors.len()`，无任何珠板外颜色

### 需求:Gerstner 的 max_colors 复用珠色减色
`Gerstner` 前段产出**全板** `BeadPattern`（不同珠色数可能 > `n`）。`max_colors == Some(n)` 时 MUST **复用 `GreedyReducer`**（见 color-reduction 规范）降到 ≤`n`——与 `Staged` **同一减色语义与 ≤N 上限口径**；`Gerstner` **不**内置另一套色数缩减。`None` 时不减色。注：`GreedyReducer` 的最少用量合并可能**重新打散** Gerstner 刚建立的空间连贯（可接受、由目视验收，见 design Risks）。

#### 场景:Gerstner + max_colors 上限成立
- **当** 以 `GeneratorKind::Gerstner` + `max_colors == Some(n)`（`n` < Gerstner 全板不同珠色数）生成
- **那么** 最终不同珠色数 ≤ `n`，由 `GreedyReducer` 在 Gerstner 前段之后施加（同 Staged 减色阶段）

### 需求:Gerstner 确定性口径（同机 canonical，非跨架构位精确）
`Gerstner` 全程 f32（Oklab、位置、质心、距离），`cbrt` 经 libm → **跨架构不保证位精确**，属**同机确定性**档（与 `OklabMatcher`/`Triangle` 缩放同级 canonical-only）。**MUST NOT** 引入 `mul_add`/FMA、`rayon`、`HashMap`/`HashSet` 迭代顺序泄漏或随机。committed golden 走**合成小夹具**（见 golden-tests / tasks）：canonical arm64 字节，或跨机结构不变量（cell 数、色都在板内、若干已知 cell）——不绑二进制照片。

#### 场景:同机重复一致、跨架构不保证
- **当** 同机对同输入以 `Gerstner` 多次生成
- **那么** 逐字节相等；跨架构位精确**不**在保证范围内（由 canonical arm64 golden 承担，见 golden-tests 规范）

