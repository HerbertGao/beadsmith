## 1. lib.rs：模块声明、重导出、BeadError::ImageEncode

- [x] 1.1 `crates/bead-core/src/lib.rs`：加 `pub mod renderer;`，重导出 `pub use renderer::{render_preview, render_grid,
  RenderOptions, BeadShape};`（紧邻现有 `pub use image::{…}` / `pub use matcher::{…}` 风格）
- [x] 1.2 `crates/bead-core/src/lib.rs` 的 `BeadError` 内**新增一个变体**（`#[non_exhaustive]` 加变体非破坏，D8）：
  ```rust
  /// PNG 编码失败。维度已守卫的合法内存缓冲实际上不会触发；存在仅为不在可达 API 上 panic
  /// （ImageDecode 已占用 #[from] ::image::ImageError，故此处用具名字段手动包裹，不重复 #[from]）。
  #[error("failed to encode image")]
  ImageEncode { source: ::image::ImageError },
  ```
  **不**新增维度类变体——`cell_size==0`/空网格/维度溢出复用既有 `InvalidImage { reason }`（D7/D8）

## 2. renderer 模块：RenderOptions + BeadShape

- [x] 2.1 新建 `crates/bead-core/src/renderer/mod.rs`，模块 doc 注明：从 `BeadPattern + &Palette` 派生 PNG 字节、
  只读 `cells` 下标 + palette RGB（绝不碰 `PixelGrid` 原始 RGB、绝不从渲染图反推、不碰文件系统，规则 3/1）；
  本地模块名与外部 `image` crate 冲突 → 一律 `::image::…`（同 image 模块先例）
- [x] 2.2 定义 `pub struct RenderOptions { pub cell_size: u32, pub shape: BeadShape }`（derive
  `Debug, Clone, Copy, PartialEq, Eq`），`impl Default` → `{ cell_size: 10, shape: BeadShape::Square }`（仿
  `ResizeOptions` 先例，D2）
- [x] 2.3 定义 `#[non_exhaustive] pub enum BeadShape { Square }`（derive `Debug, Clone, Copy, PartialEq, Eq`）；
  加 `// ponytail: 仅 Square；Circle/Ring 留后续 change，#[non_exhaustive] 是 seam，不现在写占位分支`（D2）

## 3. preview 渲染：paint_preview + render_preview + 守卫 + 编码

- [x] 3.1 私有 `fn encode_png(img: &::image::RgbImage) -> Result<Vec<u8>, BeadError>`：用
  `::image::codecs::png::PngEncoder::new_with_quality(&mut buf, PNG_COMPRESSION, PNG_FILTER)` + `img.write_with_encoder(enc)`，
  其中**具体命名常量**（不写 `::Default`，D3/m1）：`const PNG_COMPRESSION: ::image::codecs::png::CompressionType =
  CompressionType::Fast;`、`const PNG_FILTER: ::image::codecs::png::FilterType = FilterType::Adaptive;`；输入 `RgbImage` →
  **8-bit RGB、非交错**（`PngEncoder` 默认非交错、`RgbImage`=RGB8，无需额外设置，但注释声明此前置）；写入内存 `Vec<u8>`；
  编码 `Result::Err` `map_err` 成 `BeadError::ImageEncode { source }`（其 `assert_eq!(len==3*w*h)` panic 路径因喂 RgbImage 自身
  缓冲、长度恒成立而不可达，D8）；`// ponytail: 不靠 write_to 隐式默认（跨版本漂）；钉 concrete 常量只保同版本字节稳，跨版本不保（上游明文）、frozen golden 归 M7（D3）`
- [x] 3.2 私有 `fn cell_rgb(grid: &BeadPattern, palette: &Palette, pos: usize) -> [u8; 3]`：**两层 `.get()`、皆不裸索引**
  `grid.cells.get(pos).and_then(|&idx| palette.colors.get(idx as usize)).map(|c| c.rgb).unwrap_or(MISSING)`，其中
  `const MISSING: [u8; 3] = [255, 0, 255];`（哨兵色，D6）；`// ponytail: 两类可达违约（过短 cells 的 pos 越界 / 越界 palette
  下标）都落品红哨兵、不 panic（同 statistics D4 姿态；裸 cells[pos]/colors[idx] 会 panic，M2）`
- [x] 3.3 私有 `fn paint_preview(grid: &BeadPattern, palette: &Palette, cell: u32) -> ::image::RgbImage`：建
  `RgbImage::new(grid.width*cell, grid.height*cell)`（维度已由 3.4 守卫），对每格 `(x,y)` 算 `pos = y as usize*w as usize+x as usize`
  （**`usize` 算术**）、`cell_rgb(grid, palette, pos)` 取色，填满 `[x*cell,(x+1)*cell) × [y*cell,(y+1)*cell)` 像素
  （`shape==Square` 整格填；M5 无其它分支）
- [x] 3.4 公开 `pub fn render_preview(grid, palette, opts: &RenderOptions) -> Result<Vec<u8>, BeadError>`：**先守卫**
  （D7，命中返 `Err(InvalidImage { reason })`，reason 点名维度）：`grid.width==0` / `grid.height==0` / `opts.cell_size==0`；
  **缓冲守卫（R2-B1/R3-M-ord，全 `u128`、严格定序）**——抽私有 `fn preview_dims_checked(w: u32, h: u32, cell: u32) -> Result<(u32,u32), BeadError>`：
  **全 `u128`**：`out_w = w as u128 * cell as u128`、`out_h = h as u128 * cell as u128`；**① 先**判 `out_w > u32::MAX as u128 || out_h > u32::MAX as u128`
  → `Err`；**② 仅 ① 通过后**算 `bytes = 3 * out_w * out_h`（此时 `out_* <= u32::MAX` → `bytes <= 5.5e19 << u128::MAX`，不溢出），
  `bytes > isize::MAX as u128` → `Err`；返 `(out_w as u32, out_h as u32)`。**全部在 `RgbImage::new` 之前**。通过后 `encode_png(&paint_preview(..))`。
  `// ponytail: 定序是封闭前提——夹 out<=u32::MAX 前算 3*out² 会溢出连 u128 都兜不住（R3-M-ord）；RgbImage::new 内部 3*w*h usize .expect 会 panic（R2-B1）`
- [x] 3.5 `render_preview` doc：无坐标/无网格线/无 margin（成品外观）；前置条件「palette 是产出 grid 的 matcher 用的
  同一份未改动调色板」（越界容错见 3.2，不 panic）

## 4. grid 渲染：位图数字字体 + paint_grid + render_grid

- [x] 4.1 私有 `const DIGITS_3X5: [[u8; 5]; 10]`（每字形 3 宽 × 5 高，每行取低 3 位，MSB=左）：
  ```rust
  const DIGITS_3X5: [[u8; 5]; 10] = [
      [0b111,0b101,0b101,0b101,0b111], // 0
      [0b010,0b110,0b010,0b010,0b111], // 1
      [0b111,0b001,0b111,0b100,0b111], // 2
      [0b111,0b001,0b111,0b001,0b111], // 3
      [0b101,0b101,0b111,0b001,0b001], // 4
      [0b111,0b100,0b111,0b001,0b111], // 5
      [0b111,0b100,0b111,0b101,0b111], // 6
      [0b111,0b001,0b010,0b100,0b100], // 7
      [0b111,0b101,0b111,0b101,0b111], // 8
      [0b111,0b101,0b111,0b001,0b111], // 9
  ];
  ```
  `// ponytail: 手写位图数字，零字体依赖、纯整数 → 跨架构逐位确定（字体渲染 crate 的抗锯齿/hinting 会破坏确定性，D9）`
- [x] 4.1b 私有 `fn decimal_digits(n)` —— 十进制位数（`n==0 → 1`，否则 `floor(log10)+1`，纯整数循环）；`digits(n)` 即 `decimal_digits(n)`
  的别名（CR nit-2，二处调用：`grid_geom_checked` 在 `u128` 域算 `row_digits`、`paint_grid` 在 `u32` 域算各标号 `num_w(digits(n))`，
  两处语义一致）。仅以 `>=STEP(10)` 的值调用 → `d>=2`，`num_w(d)=d*4*scale-scale` 不下溢
- [x] 4.2 固定常量（D5，命名 `const`，`// ponytail: M5 固定不可配；要可配再长 RenderOptions 字段——技术上破坏、靠 Default+..default() 缓解，见 D2`）：
  `BG=[255,255,255]`、`THIN=[200,200,200]`、`BOLD=[120,120,120]`、`TEXT=[0,0,0]`、`BOLD_W: u32 = 2`、`STEP: u32 = 10`
- [x] 4.3 私有数字绘制 `fn draw_number(img, x0, y0, value: u32, scale: u32)`：把 `value` 的每个十进制位用
  `DIGITS_3X5` 按整数缩放 `scale` 画成 `TEXT` 像素，字形宽 `3*scale` + 位间隔 `scale` 顺排（**逐像素边界检查、落图外即跳过、
  不 panic** 作兜底；4.4 的 margin 公式已保证标号正常落图内、不裁切）
- [x] 4.4a 私有 `fn grid_geom_checked(width: u32, height: u32, cell: u32) -> Result<GridGeom, BeadError>`，**全程 `u128` 算几何 +
  严格定序**（R3-B1/R3-M-ord——**禁止任何 margin/scale 在 `u32` 里算**，否则大 `cell_size` 先溢出）：以 `u128` 算
  `scale = max(1, cell/5)`、`pad = scale`、`has_col/has_row`、`row_digits = decimal_digits((height/STEP)*STEP)`、
  `num_w(d) = d*4*scale - scale`、`margin_top = if has_col {7*scale} else 0`、`margin_left = if has_row {num_w(row_digits)+2*pad} else 0`、
  `out_w = margin_left + width*cell`、`out_h = margin_top + height*cell`；**① 先**判 `out_w>u32::MAX || out_h>u32::MAX → Err`（同时夹住
  margins，因 `margin_*<=out_*`）；**② 仅 ① 后**算 `bytes = 3*out_w*out_h`（`<=5.5e19<<u128::MAX`，不溢出）、`>isize::MAX → Err`；
  ③ 全通过后 cast 回 `u32` 装进 `struct GridGeom { cell, scale, pad, margin_left, margin_top, out_w, out_h: u32 }`（均 `<=u32::MAX`，cast
  安全；**必须含 `cell`**——`paint_grid` 画格原点/块/线/标号锚点都需 `cell`，而 `&BeadPattern` 与早稿 `GridGeom` 都不带它，故收进 `GridGeom`，
  review R4-M1/Codex/RC）。`// ponytail: 定序+u128 是封闭前提——margin 在 u32 算或夹前算 bytes 都会溢出（R3-B1/R3-M-ord 三审收敛）`
- [x] 4.4 私有 `fn paint_grid(grid, palette, g: &GridGeom) -> ::image::RgbImage`（**接收已校验的 `GridGeom`（含 `g.cell`），不自行重算
  margin/scale/cell**；绘制全程用 `g.cell`/`g.scale`/`g.pad`/`g.margin_*`/`g.out_*`，**禁裸 `cell`、禁从 `(out_w-margin_left)/width` 反推
  cell**——后者重引 u32 重算且脆，review R4-M1），**按 D5 overlay 几何**绘制：
  - **总尺寸 / cell 原点**：`RgbImage::new(g.out_w, g.out_h)`；cell `(x,y)` 原点 `= (g.margin_left + x*g.cell, g.margin_top + y*g.cell)`，
    块 `g.cell×g.cell`，色 `cell_rgb`（3.2，偏移 margin）。
  - **网格线（overlay，不额外占尺寸，仅画 cell 区内；R2-B2 边界检查写入）**：竖边界 `bx∈0..=width` 于 `x=g.margin_left+bx*g.cell`、
    沿 `y∈[g.margin_top,g.out_h)`；横边界 `by∈0..=height` 于 `y=g.margin_top+by*g.cell`、沿 `x∈[g.margin_left,g.out_w)`（不入 label margin）；
    **`b % STEP == 0` → `BOLD`（宽 `BOLD_W`，向 cell 内延伸）取代该处 `THIN`；否则 `THIN` 1px**（消歧，F6）。**所有线写入用
    `get_pixel_mut_checked` / 显式 `x<g.out_w && y<g.out_h`（禁裸 `put_pixel`，越界即 panic）；末右/下边界 `bx==width`/`by==height` 落
    `g.out_*`（图外）→ 夹取到 `g.out_*-1`，使非 STEP 倍数维度（13/105…）不 panic、外框确定性存在（R2-B2/Codex/RC）`。
  - **坐标（严格 1-indexed，消 off-by-one）**：列号 `for n in (STEP..=width).step_by(STEP)`：**右对齐于边界**
    `draw_number(.., x = g.margin_left + n*g.cell - num_w(digits(n)), y=g.pad, value=n, g.scale)`（右边缘 `g.margin_left+n*g.cell ≤ g.out_w`、不越右图边）；
    行号 `for n in (STEP..=height).step_by(STEP)`：**右对齐**于 `g.margin_left-g.pad`（`x_left = g.margin_left - g.pad - num_w(digits(n))`）、
    `y=g.margin_top+(n-1)*g.cell`、`value=n`。**`value` 恰为 `n`、不是 `n+1`/下标**（M3/Codex）；`num_w/digits` 用 `g.scale`（`u32`，已校验安全）。
- [x] 4.5 公开 `pub fn render_grid(grid, palette, opts: &RenderOptions) -> Result<Vec<u8>, BeadError>`：守卫顺序——
  `grid.width==0`/`grid.height==0`/`opts.cell_size==0` → `Err`；**`opts.cell_size < 5` → `Err(InvalidImage{reason:"cell_size must be >= 5 for grid"})`**
  （R2-M2；preview 无此守卫）；再 **`let g = grid_geom_checked(width, height, cell)?`**（4.4a：全 `u128`、定序、含 margin 的几何+缓冲守卫，
  `>u32::MAX`/`>isize::MAX → Err`，R3-B1/R3-M-ord），全部在 `RgbImage::new` 之前；通过后 `encode_png(&paint_grid(grid, palette, &g))`；
  doc 注 1-indexed/每 10 标号+加粗/内置位图字体/overlay 几何/`cell_size>=5`

## 5. 测试（映射 spec 需求 + 确定性门）

- [x] 5.1 `preview_size_and_per_cell_color`——`w×h` grid + `cell_size=c`，断言解码图尺寸 `(w*c)×(h*c)`，抽样若干格
  + 四角像素 `(x*c+dx,y*c+dy)` 等于 `palette.colors[cells[y*w+x]].rgb`（spec「从 BeadPattern 渲染无坐标 preview」/ D10.1）
- [x] 5.2 `grid_geometry_exact`——宽高**严格 `>10`**（如 13×13，使第 10 边界为内部边界、不与末边界重合被夹取，CR nit-1）、
  `cell_size=10`(scale=2) 的 grid，**按 D5 公式机械断言**：① 解码尺寸 ==
  `(margin_left + w*cell) × (margin_top + h*cell)`；② 第 10 竖/横边界（`margin_*+10*cell`）为 `BOLD`、第 1..9 边界为 `THIN`；
  ③ 列号 "10" 的**确切字形像素**在其右对齐锚点（右边缘 `margin_left+10*cell`，占 `[margin_left+10*cell − num_w(2), margin_left+10*cell)`，
  `y=pad`）为 `TEXT`——即确为 "10"、**非 "11"/错位**、且右边缘 `≤ out_w`（不越右图边）（M3/Codex/CR + 自引列标号溢出修复）；
  ④ 某维度 `<10` 的 grid 仍返 `Ok`、该轴 margin=0、不 panic（spec「带坐标的 grid」三场景 / D10.2）
- [x] 5.2b `grid_multidigit_label_fits_margin`——某维度 `≥100`、`cell_size>=5` 的 grid，断言行号 "100" 的全部字形像素落在
  `margin_left` 内（不裁切、不越界、不 panic）（spec「多位数（≥100）标号完整落在 margin 内」/ D10.2b）
- [x] 5.2c `grid_non_step_multiple_dims_no_panic`——`width`、`height` **均非 10 的倍数**（如 13×17，`cell_size>=5`）→ 末右/下
  THIN 边界落 `out_*`，断言返 `Ok`、解码尺寸正确、**debug 与 `--release` 均不 panic**（线写入边界检查/夹取，非裸 `put_pixel`）
  （spec「非 10 倍数的维度不 panic」/ D10.2c / R2-B2）
- [x] 5.3 `render_options_default`——`RenderOptions::default().cell_size==10 && .shape==BeadShape::Square`（spec
  「RenderOptions 与 BeadShape」/ D2）
- [x] 5.4 `invalid_input_sentinel_not_panic`——(a) 手构 `cells` 含越界下标 + 更小 `Palette`，`render_preview`/`render_grid`
  **不 panic**（debug 与 `--release`），越界格解码为 `[255,0,255]`；(b) **手构过短 `cells`**（`len < w*h`）→ 缺格解码为
  `[255,0,255]`、不 panic（M2）；(c) 空 `Palette`（`colors==[]`）子断言整图哨兵色、不 panic（spec「渲染容错（越界下标与过短
  cells）」两场景 / D6 / D10.3）
- [x] 5.5 `degenerate_and_oversize_returns_err_not_panic`——(a) `width==0`、`height==0`、`cell_size==0` 各断言返
  `Err(BeadError::InvalidImage { .. })`；(b) **grid `cell_size<5`**：对同一 grid 以 `cell_size ∈ {1,2,3,4}` 调 `render_grid` 断言
  `Err(InvalidImage)`，而 `render_preview` 同 `cell_size` 返 `Ok`（R2-M2）；(c) **超大/几何溢出**多例 pub-构造 `cells:vec![]`，均断言返
  `Err(InvalidImage)`、**不触达 RgbImage::new**：(i) `width=height=u32::MAX` + `cell_size:1`（preview，`3*out²` 须 u128+定序才不溢出）；
  (ii) `width=height=250_000_000` + `cell_size:10`；(iii) **R3-B1 margin 角**：`render_grid` 对 `10×10` + `cell_size≈2_386_092_945`
  （使 `9*scale > u32::MAX`，margin 在 `u32` 会先溢出）→ `Err`（证明 margin 在 `u128` 算）；(iv) **R3-M-ord 定序锁**：`width=height=cell=u32::MAX`
  （→ `out_* ≈ u32::MAX² ≈ 1.84e19`，**预夹** `3*out² ≈ 1.0e39 > u128::MAX (3.4e38)`） → `Err`（正序：① `out_*>u32::MAX` 立即 `Err`、永不算 bytes；
  **误把 bytes 提前到 ① 之前算的实现会在 `u128` 乘法处 panic**，本例据此捕获错序。注：用「out 仅略超 u32::MAX」**抓不到**此 bug——那时
  `3*out²≈5.5e19<u128::MAX` 不溢出，R4-M2 修正）；**(a)(b)(c) debug 与 `--release` 均不 panic**（spec「退化/超大输入」三场景 / D7 / R2-B1 / R3-B1 / R3-M-ord / R2-M2 / D10.4+4b）
- [x] 5.6 `render_only_from_cells`——同一 `cells` 配两份**RGB 不同**但等长的 palette，断言渲染像素随 palette 变（证明颜色
  来自 `cells[i]→palette.colors[idx].rgb`、非任何固定/原始像素来源）（spec「渲染只从 BeadPattern 派生」/ D4）
- [x] 5.7 `render_is_deterministic`——(a) 同 `(grid,palette,opts)` 两次 `render_preview`/`render_grid` 的 `Vec<u8>`
  **字节相等**；(b) 解码回像素等于手算期望（含一个越界哨兵格）。`// ponytail: PNG 跨版本可能漂、像素跨架构逐位稳——
  字节测同跑相等、像素测绝对值；硬字节 golden 留 M7（spec「渲染确定性」两场景 / D3 / D10.5）`

## 6. 收尾验证 + 依赖/文档确认

- [x] 6.1 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（debug **与** `--release`）全绿
- [x] 6.2 确认 renderer **无新依赖**（`cargo tree` 不新增 crate；编码用既有 `image` 的 `png` feature）、**无字体渲染 crate**、
  **无 `rayon`/`f32` 绘制坐标/`HashMap`/`HashSet`**；bead-core 仍无 fs/UI/平台依赖（规则 1）
- [x] 6.3 确认**无真理源校正需求**：`ARCHITECTURE.md`（renderer 模块 `render_preview/render_grid` + `Renderer` 在 Future
  Plugin Architecture）、`ROADMAP.md`（M5 done-when）、`INIT.md`（preview 无坐标 / grid 行列号+格线）均已与实现一致、未改动；
  `openspec/specs/color-matching/spec.md`「下游渲染（M5）从 cells 派生」前向约束由本 change 的 renderer 规范 + 5.6 测试落地、
  其规范性文字不变（proposal「修改功能：无」）
