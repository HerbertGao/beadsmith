# renderer 规范

## 目的
定义从 `BeadPattern` + `&Palette` 渲染 PNG 字节的两个总函数：`render_preview` 产**无坐标**的成品珠外观（每格 `cell_size ×
cell_size` 实心方块、行优先），`render_grid` 产**带行号/列号 + 网格线**的拼装图（1-indexed、每 10 格加粗分隔并标坐标数字，用
内置位图数字字体、无字体依赖）。二者**只从 `BeadPattern.cells`（调色板下标）+ palette 派生**颜色（每格取 `palette.colors[idx].rgb`），
**禁止**读 `PixelGrid` 原始 RGB、**禁止**从任何渲染图反推、**禁止**碰文件系统（返 `Result<Vec<u8>, BeadError>` 的内存 PNG 字节）。
PNG 编码参数钉死 → 同输入同依赖版本逐字节稳定（golden 与「CLI == FFI」前提）；越界下标、空网格（0 维度）、过大输出缓冲、
`cell_size` 非法均确定性返 `Err` 或可观测信号、**不 panic**。`BeadShape` 当前仅 `Square`，留 seam 供后续扩展。

## 需求
### 需求:从 BeadPattern 渲染无坐标 preview
`render_preview` 必须接受一个 `BeadPattern`、一个 `&Palette` 与一个 `&RenderOptions`，返回 `Result<Vec<u8>, BeadError>`
（成功时为内存 PNG 字节）。它**必须只从 `BeadPattern.cells`（调色板下标）派生**：每格 `idx = cells[y*width+x]` 取
`palette.colors[idx].rgb` 作为颜色（越界见「容错」需求），**禁止**读取 `PixelGrid` 原始 RGB、**禁止**从任何渲染图反推、
**禁止**读取文件系统。每颗珠子必须渲染成 `cell_size × cell_size` 的实心方块（`shape == Square`），行优先摆放，输出像素
尺寸为 `(width*cell_size) × (height*cell_size)`。preview **禁止**含坐标数字、网格线或 margin（它代表成品外观）。

#### 场景:逐格上色且尺寸为 width×height 放大 cell_size 倍
- **当** 对一个 `w×h` 的 `BeadPattern` 与 `cell_size = c` 调用 `render_preview`
- **那么** 返回 `Ok(bytes)`，其解码后的图像尺寸为 `(w*c) × (h*c)`，且位置 `(x*c+dx, y*c+dy)`（`0<=dx,dy<c`）的像素
  等于 `palette.colors[cells[y*w+x]].rgb`

### 需求:从 BeadPattern 渲染带坐标的 grid
`render_grid` 必须接受一个 `BeadPattern`、一个 `&Palette` 与一个 `&RenderOptions`，返回 `Result<Vec<u8>, BeadError>`
（成功时为内存 PNG 字节）。它**必须只从 `cells` 下标 + palette 派生**（同 preview 的禁止项）。除按 `cell_size` 渲染珠子格外，
还**必须**：① 在每个格边界画细网格线；② 每第 10 个边界（`10, 20, 30, …`）画加粗分隔线；③ 标注**1-indexed** 行号与列号——
在不超过对应维度的每 10 处（`10, 20, …`）显示十进制坐标。坐标数字**必须用内置位图数字字体绘制**，**禁止**引入字体渲染依赖。
线色、字色、背景色、加粗线宽与标号间隔（10）必须是固定常量（M5 不可配置）。

`render_grid` **必须要求 `cell_size >= 5`**：当 `opts.cell_size < 5` 时**必须**返回 `Err(BeadError::InvalidImage { reason })`（坐标
标号需此最小格宽，否则行标号竖向裁切、列标号横向重叠）。`render_preview` **不**受此限（无标号，`cell_size >= 1` 即可）。

布局几何**必须是完全确定性的整数几何**（design D5 给出全部公式），使输出可机械计算、可写确定性测试、可被 M7 冻结：
- margin、总输出尺寸、cell 像素原点、网格线位置、粗/细线在 `×10` 边界的优先级、标号锚点**必须**由 `cell_size` 与字体缩放
  （`scale = max(1, cell_size/5)`）按固定公式算出；网格线**叠加（overlay）在格边缘像素上、不额外占尺寸**，故总尺寸
  `= margin + 维度 × cell_size`、cell `(x,y)` 原点 `= (margin_left + x*cell_size, margin_top + y*cell_size)`。
- `×10` 边界**必须**画加粗线**取代**（而非叠加于）该处的细线（消除粗细叠加歧义）。
- **所有线像素与字形像素的写入必须经边界检查**（落在 `[0,out_w)×[0,out_h)` 外即跳过/夹取，**禁止**用越界即 panic 的裸像素写）；
  特别是末右/下边界线（落在 `out_w`/`out_h`，即图外）**必须夹取到最后合法像素**，使任意 `width`/`height`（含**非 10 倍数**，如
  13、105）都不 panic、且外框确定性存在。
- 坐标**必须严格 1-indexed**：对每个 `n ∈ {10, 20, …}` 且 `n ≤ 该维度`，绘制的十进制文本**必须恰为 `n`**（对应第 `n` 列/行、
  0-based 下标 `n-1`）；**禁止**把间隔值当下标而画成 `n+1`（如把列 10 标成 "11"）。列标号**右对齐于其列右边界** `margin_left + n*cell`
  （`n ≤ width` 保证不越右图边）；行标号**右对齐于左 margin** 内。
- 左/顶 margin **必须**按**最大标号的位数**撑够宽/高，使任何被绘制的标号（含 ≥100 的多位数；**无尺寸上限**——`width`/`height`
  为 `u32`，标号位数由 `decimal_digits` 决定，可达 4 位及以上）**完整落在 margin 内、不被裁切**。

#### 场景:1-indexed 坐标在确定性几何位置、每 10 加粗线取代细线
- **当** 对一个宽高均 **严格 `> 10`**（如 13×13，使第 10 边界为**内部**边界、不与末边界重合被夹取）、`cell_size = c`、`scale = max(1,c/5)` 的 `BeadPattern` 调用 `render_grid`
- **那么** 返回 `Ok(bytes)`，其解码图：尺寸等于按 D5 公式手算的 `(margin_left + width*c) × (margin_top + height*c)`；
  第 10 个竖/横边界（像素 `margin_* + 10*c`）处为加粗线色、第 1..9 边界为细线色；列号 "10" 的位图字形像素出现在其确定性锚点
  （**右对齐于边界**：右边缘 `x = margin_left + 10*c`、即占 `[margin_left+10*c − num_w(2), margin_left+10*c)`，`y = pad`）且为字色
  ——即确为 "10"（非 "11" 或错位）、且右边缘不越图右边

#### 场景:多位数（≥100）标号完整落在 margin 内
- **当** 对一个某维度 `≥ 100` 的 `BeadPattern` 调用 `render_grid`
- **那么** 该轴标号 "100" 的全部字形像素落在对应 margin 范围内（不裁切、不越界、不 panic）

#### 场景:维度小于标号间隔仍出图不 panic
- **当** 对一个某维度 `< 10` 的 `BeadPattern`（该维度无任何 10 的倍数）调用 `render_grid`
- **那么** 返回 `Ok(bytes)`（该维度不绘制坐标数字、该轴 margin 为 0），且**不 panic**

#### 场景:非 10 倍数的维度（末边界线落图边）不 panic
- **当** 对一个 `width` 与 `height` 均非 10 的倍数（如 13×17）、`cell_size >= 5` 的 `BeadPattern` 调用 `render_grid`
- **那么** 返回 `Ok(bytes)`、解码尺寸等于 D5 公式值，且**不 panic**（末右/下边界线落在 `out_w`/`out_h` 时夹取到最后合法像素，不越界写）

### 需求:RenderOptions 与 BeadShape
`RenderOptions` 必须含 `cell_size: u32` 与 `shape: BeadShape`，并提供 `Default`（`cell_size == 10`、`shape ==
BeadShape::Square`）。`BeadShape` 必须是 `#[non_exhaustive]` 枚举且当前**仅**含 `Square` 一个变体（为后续扩展其它珠形
保留 seam，但 M5 **禁止**实现任何非 `Square` 的渲染分支）。`cell_size` 控制每颗珠子的像素边长。

#### 场景:默认选项为 10px 方块
- **当** 构造 `RenderOptions::default()`
- **那么** 其 `cell_size` 等于 `10`、`shape` 等于 `BeadShape::Square`

### 需求:渲染对非法输入容错（越界调色板下标与过短 cells 均不 panic）
`render_preview` 与 `render_grid` **禁止**在越界调色板下标**或过短的 `cells`** 上 panic。取色必须经**两层边界检查、皆不裸
索引**：先按位置取格 `cells.get(y*width + x)`，再按格取色 `palette.colors.get(idx as usize)`（`idx: u16` → `usize` 加宽，索引 `Vec` 需 `usize`）；任一为 `None`（缺格或越界下标）时，
该格**必须**填一个固定的哨兵色 `[255, 0, 255]`（品红，作为「调色板配错 / pattern 损坏」的可观测信号），而非 panic 或静默贴
一个貌似合理的真实色。两类可达违约都必须容错：
- **越界调色板下标**（传入比配色时更小或不同的 `Palette`，`idx >= colors.len()`）→ 哨兵色；空调色板（`colors.len() == 0`）
  是其退化特例：每格皆越界 → 整图哨兵色、不 panic。
- **过短 `cells`**（`cells.len() < width*height`；`BeadPattern` 字段 `pub`、调用方可违反 `cells.len()==width*height` 不变量）
  → 渲染按 `pos = y*width+x` 空间索引，缺失位置 → 哨兵色、不 panic（裸 `cells[pos]` 会越界 panic，故必须用 `get`）。

文档化前置条件：`palette` 应是产出该 `BeadPattern` 的匹配器所用的同一份未改动调色板、且 `cells.len()==width*height`
（与 statistics/matcher/models 同口径）；违反时按上面哨兵规则确定性容错。

#### 场景:越界下标渲染为哨兵色而非 panic
- **当** 用一个比产出 `cells` 时更小的 `Palette`（存在 `cells[i]` 越界）调用 `render_preview` 或 `render_grid`
- **那么** 不 panic，越界格在解码图中为 `[255, 0, 255]`（品红哨兵色）

#### 场景:过短 cells 的缺失格渲染为哨兵色而非 panic
- **当** 用一个 `cells.len() < width*height` 的手构 `BeadPattern` 调用 `render_preview` 或 `render_grid`
- **那么** 不 panic，缺失位置在解码图中为 `[255, 0, 255]`（品红哨兵色）

### 需求:退化/超大输入返回确定性错误而非 panic
当无可渲染面积、维度非法或输出缓冲过大时，`render_preview` 与 `render_grid` **必须**返回确定性的
`Err(BeadError::InvalidImage { reason })`、**禁止** panic（在 debug 与 release 构建下均不 panic）。触发条件至少包括：
`width == 0`、`height == 0`、`cell_size == 0`；`render_grid` 额外的 `cell_size < 5`（见 grid 需求）；以及**输出缓冲尺寸守卫**。
**输出缓冲尺寸守卫——守卫算术本身不得溢出，且全部几何量须在 `u128` 中算、严格定序**：实现**必须**在调用任何分配
（`RgbImage::new`）之前、且**在把任何几何量物化为 `u32` 之前**，**以 `u128` 计算全部几何**——`scale`、`margin_left`、`margin_top`、
总输出宽 `out_w`、总输出高 `out_h`、总缓冲字节数 `3 × out_w × out_h`。**禁止先在 `u32` 里计算 `scale`/margin 等中间量**（大
`cell_size` 下 `7*scale`、`num_w(d)=d*4*scale-scale` 等 `u32` 乘法会在守卫之前溢出）。守卫**必须按此序**：① 算 `out_w`、`out_h`
（u128），**先**判 `out_w > u32::MAX || out_h > u32::MAX` → `Err`（这一判**同时**夹住 margins，因 `margin_* <= out_*`）；
② **仅在 ① 通过后**（此时 `out_w,out_h <= u32::MAX`）算 `bytes = 3 × out_w × out_h`、判 `bytes > isize::MAX` → `Err`。**此定序是封闭性
前提**：若在夹 `out_* <= u32::MAX` 之前算 `3 × out_w × out_h`，其上界 `≈ 3 × (u32²)² ≈ 1.0e39` 会**溢出 `u128`**（`u128::MAX ≈ 3.4e38`）；
夹后 `3 × out_w × out_h <= 3 × u32::MAX² ≈ 5.5e19 << u128::MAX`，不溢出。其依据是：底层 `RgbImage::new`（`ImageBuffer::new`）内部以
`usize` 计算 `3*w*h` 后 `.expect()`（溢出 `usize` 即 **panic**），而 `> isize::MAX` 的尺寸是 Rust 必失败的分配。`isize::MAX` 守卫挡
这两类（usize-溢出 `.expect` + `> isize::MAX` 分配）；但**字节数 `<= isize::MAX` 却超本机内存**的 OOM-abort **不**在此守卫范围内，
是一项已接受的已知限制（仅经蓄意违反不变量的荒谬 pub 构造可达、管线不可达、与 M2 resize 同样无尺寸上限；详见 design D7）。仅守逐维度、
用 `u64`、或在 `u32` 算 margin、或乱序算 `bytes`，都不足以阻止该可达 panic（可经 `pub` 构造的超大 `BeadPattern` 触发）。`reason` 必须确定性地点名违例维度。PNG 编码若失败必须返回 `Err(BeadError::ImageEncode { .. })`（不得 panic、不得复用解码
错误变体）。

#### 场景:空网格或零 cell_size 返回 InvalidImage
- **当** 对 `width == 0`（或 `height == 0`，或 `cell_size == 0`）的输入调用 `render_preview` / `render_grid`
- **那么** 返回 `Err(BeadError::InvalidImage { reason })`，且不 panic（debug 与 release 均然）

#### 场景:render_grid 在 cell_size 小于 5 时返回 InvalidImage 而 render_preview 不
- **当** 对同一 `BeadPattern` 以 `cell_size ∈ {1,2,3,4}` 分别调用 `render_grid` 与 `render_preview`
- **那么** `render_grid` 返回 `Err(BeadError::InvalidImage { reason })`，而 `render_preview` 返回 `Ok(bytes)`；均不 panic

#### 场景:超大输出缓冲返回 InvalidImage 而非 panic
- **当** 用一个使 `3 × (width*cell_size) × (height*cell_size)` 溢出 / 超过 `isize::MAX`（或某维度超 `u32`）的 `pub`-构造
  超大 `BeadPattern` 调用 `render_preview` / `render_grid`
- **那么** 返回 `Err(BeadError::InvalidImage { reason })`，且**不 panic**（debug 与 release 均不触达 `RgbImage::new` 的 `.expect`）

### 需求:渲染只从 BeadPattern 派生，绝不从渲染图反推
渲染**必须**把 `BeadPattern` 当作真理源：颜色只来自 `cells[i] → palette.colors[idx].rgb`（CLAUDE 规则 3 / color-matching
「BeadPattern 是配色后的真理源」前向约束的落地）。**禁止**回看 `PixelGrid` 的原始 RGB，**禁止**从已渲染图像反推任何
数据，**禁止**把渲染结果存回 `BeadPattern`（`BeadPattern` 保持 `{ width, height, cells }` 不变）。

#### 场景:颜色来自 cells 下标查 palette 而非原始像素
- **当** 渲染任一格的颜色
- **那么** 该颜色取自 `palette.colors[cells[i]].rgb`（按下标查 palette），不得取自 `PixelGrid.pixels` 或任何渲染图像

### 需求:渲染确定性（PNG 字节同跑相等且像素跨架构一致）
同一 `(BeadPattern, Palette, RenderOptions)` **必须**产生确定性输出。实现**禁止**引入非确定性来源（`rayon` 并行、随机、
`f32` 绘制坐标、`HashMap`/`HashSet` 迭代顺序泄漏）；像素绘制必须为整数运算，因此像素缓冲跨架构（arm64 / x86_64）必须
逐位一致。PNG 编码参数（压缩等级、filter）必须**显式固定为具体命名常量**（不依赖可能随依赖版本变动的隐式默认），且输出
**必须为 8-bit RGB、非交错（non-interlaced）**，使同一二进制对同一输入多次调用产生**逐字节相同**的 PNG 字节，并使「解码回
像素比较」的无损 round-trip 良基。注：逐字节 PNG 输出**仅在锁定的依赖版本下**稳定（上游不保证跨版本字节一致）；跨版本的
frozen 字节 golden 属 M7。

#### 场景:同输入多次渲染字节相等
- **当** 对同一 `(BeadPattern, Palette, RenderOptions)` 多次调用 `render_preview`（或 `render_grid`）
- **那么** 每次返回的 `Vec<u8>` 完全相等

#### 场景:解码后像素等于按下标查 palette 的期望值
- **当** 对一个固定小 `BeadPattern` + 固定小 `Palette` 渲染并把 PNG 解码回像素
- **那么** 每格像素等于 `palette.colors[cells[i]].rgb`（越界格为哨兵色）的手算期望，且断言在 arm64 与 x86_64 上都通过

