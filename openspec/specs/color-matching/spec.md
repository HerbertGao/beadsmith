# color-matching 规范

## 目的
定义把 `PixelGrid`（原始 RGB）映射为 `BeadPattern`（行优先调色板下标）的配色交接：`match_pattern` 是唯一生产点，
逐格经 `ColorMatcher` 取最近调色板色、平局取最低下标（确定性门）；**默认匹配器为 `LabMatcher`**（CIELAB + ΔE76 感知配色），
`RgbMatcher`（RGB 平方欧氏）保留为整数跨架构基准。`BeadPattern { width, height, cells: Vec<u16> }`
不含 `stats` 字段——它是配色后的管线真理源，下游预览/统计/导出皆从 `cells` 派生、绝不从渲染图反推。确定性分两档：`RgbMatcher`
纯整数距离与下标键、跨架构位精确；默认 `LabMatcher` 用 f32（`cbrt`/`powf`）、跨架构非位精确，按 canonical=arm64-Linux 字节
golden + 其它平台结构不变量 + 同机 CLI==FFI 保证（与 `Lanczos3` 的 `f32::sin` 同构）。纯库行为，无文件系统/UI/平台依赖。
## 需求
### 需求:从 PixelGrid 产出 BeadPattern
`match_pattern` 必须接受一个 `PixelGrid` 与一个 `&dyn ColorMatcher`，把每格的原始 RGB 映射到
调色板下标，产出 `BeadPattern`。映射必须**行优先一一对应**：`cells[i]` 对应 `pixels[i]`（同一
`i = y*width+x`），不做坐标转换。`BeadPattern` 的 `width`、`height` 必须原样等于源 `PixelGrid`。
`match_pattern` 禁止读取文件系统、禁止接受调色板以外的外部状态；它是 `PixelGrid`（原始色）到
`BeadPattern`（下标）的唯一交接点。

`match_pattern` **要求前置条件** `grid.pixels.len() == grid.width as usize * grid.height as usize`
（长度运算一律 `usize`，绝不 `u32` 乘——同 `models/mod.rs` 对 `PixelGrid` 的口径，避免大网格 `u32` 溢出）。
该不变量由 `resize_image` 保证；因 `PixelGrid` 字段 `pub`、可被外部手构破坏，违反此前置条件属**调用方
契约违约**（与 `models/mod.rs` 对 `PixelGrid` 的口径一致）。`match_pattern` 遍历 `grid.pixels` 产出
`cells`，保持全函数（不返 `Result`、不复检）；故**前置条件成立时** `cells.len() == width*height` 成立。
退化网格 `width==0` 或 `height==0`（前置条件下 `pixels` 为空）合法产出 `cells.len()==0`。

#### 场景:逐格映射且形状一致
- **当** 对一个满足 `pixels.len()==w*h` 的 `w×h` `PixelGrid` 调用 `match_pattern(&grid, &matcher)`
- **那么** 返回的 `BeadPattern` 满足 `width==w`、`height==h`、`cells.len()==w*h`，且 `cells[i]`
  是 `pixels[i]` 的最近调色板下标

### 需求:最近色用 RGB 平方欧氏距离
`RgbMatcher::find_best_match` 必须返回调色板中到 `target` 的 **RGB 平方欧氏距离**最小的颜色下标：
`d = (Δr)² + (Δg)² + (Δb)²`。禁止开方（`sqrt` 不改变排序且引入浮点）。分量差必须先 widening 到
`i32`（`a as i32 - b as i32`）再平方，禁止 `u8` 减法下溢；距离累加用 `u32`（最大 `3×255²=195075`）。

#### 场景:精确命中调色板色
- **当** 某像素的 RGB 恰好等于调色板中某色的 RGB
- **那么** `find_best_match` 返回该色的下标（距离 0）；若多个调色板色共享同一 RGB（调色板只保证
  `code` 唯一、**不**保证 RGB 唯一），按平局规则返回其中**最低下标**

#### 场景:离色取最近
- **当** 某像素不在调色板中
- **那么** `find_best_match` 返回平方欧氏距离最小的那个调色板色的下标

### 需求:平局取最低下标
多个调色板色与 `target` 平方距离相等时，`find_best_match` 必须返回**遍历中最低的下标**（即用严格
`<` 更新最优、相等不更新）。该规则必须固定且确定，禁止依赖迭代或比较顺序的偶然性。

#### 场景:等距时返回最低下标
- **当** 调色板中有两个色到某像素的平方距离相等（如调色板含两个等距色，像素在二者正中）
- **那么** `find_best_match` 必须返回其中**下标较小**者，且重复调用结果一致

### 需求:BeadPattern 输出形状
`BeadPattern` 必须含 `width`、`height` 与行优先的 `cells: Vec<u16>`（每个 `u16` 是调色板下标），
且 `cells.len() == width*height`、`cells[y*width+x]` 为第 (x, y) 格。长度与下标运算必须以 `usize`
进行。必须提供 `cell_at(x, y) -> Option<u16>`，越界返回 `None`。`BeadPattern` 不含 `stats` 字段
（统计在 M4 以派生函数 `count_colors`/`total_beads`/`generate_summary` 从 `cells` 现算提供，**永不**作为
`BeadPattern` 字段；见 M4-D1。本括注由 add-statistics 修正——原作"（统计属 M4）"易被读作"stats 字段将在 M4 到来"，
而 M4 经探索定为"派生而非存储"，故澄清；规范性要求「不含 `stats` 字段」不变、由此永久强化）。

#### 场景:cell_at 行优先取格与越界
- **当** 对一个 `w×h` 的 `BeadPattern` 调用 `cell_at(x, y)`
- **那么** `x<w && y<h` 时返回 `Some(cells[y*width+x])`，否则返回 `None`

### 需求:matcher 构造拒绝非法调色板
`RgbMatcher::new` 必须返回 `Result<RgbMatcher, BeadError>`，并在构造时拒绝：调色板 `colors` 为空 →
`BeadError::InvalidPalette`（`reason` 含 "no colors"）；调色板色数 `> 65536` →
`BeadError::InvalidPalette`（`reason` 含 "more than"，防 `color_index as u16` 静默截断）。边界精确：
合法下标 `0..=65535`（`u16::MAX==65535`），故 `len == 65536` **合法接受**，首个溢出的是 `len == 65537`
（下标 65536 wrap）。`reason` 必须确定性。matcher 禁止 panic；匹配热路径 `find_best_match` 保持全函数
（不返回 `Result`），其「调色板非空且下标 ≤ 65535」由 `new` 的构造守卫保证。

#### 场景:拒绝空调色板
- **当** 用 `colors` 为空的 `Palette` 调 `RgbMatcher::new`
- **那么** 返回 `Err(InvalidPalette)`，`reason` 含 "no colors"，不 panic

#### 场景:拒绝超 u16 的调色板
- **当** 用 `colors.len() == 65537` 的 `Palette` 调 `RgbMatcher::new`
- **那么** 返回 `Err(InvalidPalette)`，`reason` 含 "more than"，不 panic / 不静默截断下标
- **且** 用 `colors.len() == 65536` 的 `Palette` 调 `RgbMatcher::new` 必须**成功**（下标 65535 可表示）

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

### 需求:BeadPattern 是配色后的真理源
配色完成后 `BeadPattern` 必须是管线的真理源：下游统计（M4）必须从 `BeadPattern.cells` 的下标计数、
下游渲染（M5）必须从 `cells[i]` 查 `palette.colors[idx].rgb` 上色，**禁止**从 `PixelGrid` 的原始
RGB 或渲染图反推。`PixelGrid` 自 `match_pattern` 起降为配色前中间体，不再作为外部结果返回。

> **M3 可验证范围 vs 前向约束**：M3 能钉住的是交接本身——`match_pattern` 是 `BeadPattern` 的**唯一
> 生产点**、`PixelGrid` 不作为外部结果返回（结构性保证，下游拿到 `BeadPattern` 后物理上只能读 `cells`
> 下标）。下面的「统计/渲染从 `cells` 而非原始像素派生」是对 **M4/M5** 的前向约束，由 M4/M5 的任务与
> 测试落地，**M3 无对应测试**（M3 尚无 stats/渲染代码）。

#### 场景:统计来自 BeadPattern 而非原始像素（前向约束，M4 落地，非 M3 测试）
- **当** 将来需要每色豆数（M4）
- **那么** 计数必须遍历 `BeadPattern.cells`（调色板下标），不得遍历 `PixelGrid.pixels`（原始 RGB）

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

