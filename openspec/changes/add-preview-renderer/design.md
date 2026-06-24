## 上下文

里程碑 M5。前序已建：`palette`（M1，`Palette { brand, colors: Vec<PaletteColor> }`、`PaletteColor { code, name,
rgb: [u8;3] }`、有序 `Vec`、无 `HashMap`）；`image`（M2，解码/裁剪/缩放 → `PixelGrid`，模块名 `image` 与外部
`image` crate 冲突，内部一律 `::image::…`；`ResizeOptions` 是「选项结构体」先例，并刻意把 `FilterType`/`RgbImage`
等外部类型 leak 进公开签名换取原语复用，见 M2-D3）；`matcher`（M3，`match_pattern: PixelGrid → BeadPattern`，
唯一生产点；下标 `i ≡ palette.colors[i]`）；`statistics`（M4，纯整数派生、越界 palette「跳过+可观测信号、不 panic」
的 D4 姿态）。

M5 交付 `renderer` 模块：两个公开函数从 `BeadPattern + &Palette` 渲染**内存 PNG 字节**——`render_preview`（无坐标
成品外观）与 `render_grid`（行/列号 + 网格线）。约束不变：纯库、不碰文件系统（规则 1，返回 `Vec<u8>` 由 CLI 写盘）、
**确定性是门**、单线程、`thiserror`/`Result<T, BeadError>`。color-matching 规范已写下渲染的前向约束（`下游渲染从
cells[i] 查 palette.colors[idx].rgb 上色，禁止从 PixelGrid 原始 RGB 或渲染图反推`，line 94–106），M5 落地它。

珠形：用户明确「先做实心方块，后续要扩展多种形式」——故 M5 留 `BeadShape` seam（单 `Square` 变体）但只实现方块。
放大倍率：用户定 `cell_size` 默认 10、可调。网格坐标：1-indexed、每 10 格标号 + 加粗、数字用内置位图字体（无字体依赖）。

## 目标 / 非目标

**目标：** `renderer` 模块（`render_preview` + `render_grid`）；`RenderOptions { cell_size, shape }` + `BeadShape`
seam；`cell_size × cell_size` 实心方块珠（颜色取 `palette.colors[idx].rgb`）；grid 的每格细线 + 每 10 格粗线 +
1-indexed 行/列号（内置 3×5 位图数字字体）；越界 palette 下标哨兵色容错（不 panic）；空网格/`cell_size==0`/维度
溢出确定性 `Err`（不 panic）；像素缓冲跨架构逐位相同 + PNG 字节同跑两次相同。

**非目标：** 非 `Square` 珠形（seam only）；`Renderer` trait（future，见 D1）；写盘 / pipeline 串联（M6）；可配置
线宽/线色/字色/标号间隔（M5 固定常量，D5）；抗锯齿/透明/TrueType 字体/图内色卡；硬编码 PNG 字节 golden（M7）；
`rayon`（Phase 2）。

## 决策

**D1 — 模块与签名：新 `renderer` 模块，两个公开自由函数 `render_*(grid, palette, opts) -> Result<Vec<u8>, BeadError>`；M5 不引 `Renderer` trait。**

```rust
pub fn render_preview(grid: &BeadPattern, palette: &Palette, opts: &RenderOptions) -> Result<Vec<u8>, BeadError>;
pub fn render_grid(grid: &BeadPattern, palette: &Palette, opts: &RenderOptions) -> Result<Vec<u8>, BeadError>;
```

返回**内存 PNG 字节** `Vec<u8>`（规则 1：core 不碰文件系统；CLI 写 `preview.png`/`grid.png`、FFI 透传字节）。
内部分两层：私有 `paint_preview/paint_grid(...) -> ::image::RgbImage`（纯像素绘制，跨架构逐位相同）+ 私有
`encode_png(img) -> Result<Vec<u8>, BeadError>`（PNG 序列化，见 D3/D8）。

- **理由**：① 自由函数 + 模块对齐 ARCHITECTURE 的 renderer 模块清单（`render_preview(...)`/`render_grid(...)`）与
  M4 statistics 的自由函数风格（真理源模型的叶子消费者）。② `BeadPattern + &Palette + &RenderOptions` 三参足够——
  渲染是 `(pattern, palette, options)` 的纯函数。③ 像素层/编码层分离让**确定性测试钉在像素缓冲**（整数、跨架构稳），
  PNG 字节只测「同跑两次相等」，把脆弱的字节硬编码留给 M7（D3）。
- **替代方案 (A)：引 `Renderer` trait（M5 即一个 `impl`）。** 否决：这是 ponytail「单实现接口」反模式，且用户明确选普通
  函数。ARCHITECTURE 把 `Renderer` 列在「**Future** Plugin Architecture」；M3 给 `ColorMatcher` 引 trait 是因为第二个
  实现（CIELAB+ΔE）是 INIT/ROADMAP **已排期的 Phase-3 算法档位**——trait 买的是具体计划中的换算法。渲染**没有**已排期的
  第二渲染算法；产品近期真正要的变化轴是**珠形**，而珠形是 `RenderOptions.shape` 这一**数据字段**、不是另一个渲染器
  （D2）。真出现第二种渲染**策略**时再非破坏地引 trait（`#[non_exhaustive]` 心智、不改 pipeline 主流程，符合设计规则
  「新增实现不改 `generate_pattern`」）。设计规则「可替换算法走 trait」的**意图**是别为变体 fork pipeline——M5 用一个函数
  + shape 字段恰恰满足该意图，零分叉。
- **替代方案 (B)：公开 `paint_* -> RgbImage`（不编码）让调用方自行编码。** 否决：契约产物是 PNG 字节（CLI 直接写
  `preview.png`、M7 golden 是 `preview.png`）；公开 `RgbImage` 把编码责任与「外部类型 leak」推给每个调用方。M5 只暴露
  两个产 PNG 字节的函数；`RgbImage` 留私有（像素层仅供模块内测试/复用）。YAGNI：真有「要原始像素」的消费方再加。
- **前向 seam 备忘（review m5，findings-only，不在 M5 落地）**：① **输出格式**变体（SVG / 原始像素 / 非 PNG）与**珠形**是
  **正交**两轴——`BeadShape` 只覆盖格内字形几何，**不**吸收输出格式；真要第二种输出格式时，需引 `Renderer` trait **或**加新函数
  （届时非破坏、不改 `generate_pattern`），D1 的「trait 可后引、非破坏」**仅**指「引 trait 本身非破坏」，不代表当前的 `shape` 字段
  已替未来格式变体兜底。② **M8 FFI** 很可能想要**原始像素缓冲**（避免 Dart/skia 为显示而 PNG 解码往返）——但 PNG `Vec<u8>` 是
  **M5 既定契约（用户已定 core 返 PNG 字节）**，且「加一个返像素的入口」是**可加的、非破坏**的（M8 再做）；本 change**不**现在建
  `RenderedImage` 像素结构（= 越出已定范围、YAGNI，SA 亦认「YAGNI holds」）。此条记为**已知前向约束、accepted（scope 内保持
  PNG 字节）**，非 M5 缺陷。

**D2 — `RenderOptions { cell_size: u32, shape: BeadShape }` + `BeadShape` seam（仅 `Square`，`#[non_exhaustive]`）。**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOptions { pub cell_size: u32, pub shape: BeadShape }

impl Default for RenderOptions {
    fn default() -> Self { RenderOptions { cell_size: 10, shape: BeadShape::Square } }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeadShape { Square }   // ponytail: 仅此一形；Circle/Ring 后续 change 加，#[non_exhaustive] 留 seam、不现在写
```

- **理由**：① 仿 `ResizeOptions` 先例（选项结构体 + `Default`），调用方写 `RenderOptions::default()` 或调 `cell_size`。
  ② 用户明确「后续扩展多种珠形」→ `shape` 字段 + `#[non_exhaustive]` 枚举是**用户要求的** seam（ponytail 例外：显式要求
  的扩展点不算投机抽象）；但 M5 **只**实现 `Square`，绝不写 Circle/Ring 占位分支（那才是投机）。`#[non_exhaustive]` 让后续
  加变体非破坏。③ `cell_size: u32` 与 `width/height: u32` 同宽，避免类型转换噪声；`cell_size==0` 由 D7 守卫。
- **替代方案：`cell_size` 用 `NonZeroU32`。** 否决：把「非零」搬进类型虽优雅，但 `Default` 要 `unwrap`/`new` 噪声、且与
  `ResizeOptions` 全 plain 字段的先例不一致；M5 用运行时守卫 + `InvalidImage { reason }`（D7），与 image 模块零维守卫同口径。
- **替代方案：M5 不留 `shape`、只 `cell_size`。** 否决：用户明确要珠形扩展 seam；只留 `cell_size` 会迫使后续加珠形时
  改 `RenderOptions` 形状（破坏）。留 `shape` 现在零成本、未来扩展靠 `BeadShape`（`#[non_exhaustive]` 枚举加变体非破坏）。
- **`RenderOptions` 故意 *不* 标 `#[non_exhaustive]`（与 `BeadShape` 相反）+ 「加字段非破坏」是误述、已更正（review m2）**：
  `RenderOptions` 字段 `pub` 且无 `#[non_exhaustive]` → 调用方可字面构造 `RenderOptions { cell_size, shape }`（同 `ResizeOptions`
  先例，便利）。**代价**：日后给 `RenderOptions` **加 `pub` 字段是技术上的破坏变更**（已有字面构造失配）——早稿说这类后续加字段
  「非破坏」是**错的**，现更正：① 该破坏由 `Default` + `..Default::default()` 结构更新语法**缓解**（调用方写
  `RenderOptions { cell_size: 5, ..Default::default() }` 则加字段不失配）；② M5 阶段**无任何外部字面构造者**（CLI/FFI 未落地），
  破坏面为零；③ 这是**与 `ResizeOptions` 一致的自觉取舍**（它同样非 `#[non_exhaustive]`、同样以便利换「加字段破坏」）。
  与之相对，`BeadShape` **标** `#[non_exhaustive]`，因加**枚举变体**（Circle/Ring）是近期已知扩展、必须非破坏。
  替代（未采纳）：现在就给 `RenderOptions` 标 `#[non_exhaustive]` 保「加字段非破坏」——否决：会夺走 `ResizeOptions` 享有的字面
  构造便利、且与该先例不一致；珠形扩展走 `BeadShape` 已够。

**D3 — 输出格式 = PNG 字节；确定性双层（像素跨架构逐位 + PNG 字节同跑相等）；编码参数显式钉死；硬编码字节 golden 留 M7。**

- **像素层**（`paint_*`）：全整数绘制——按 `cell_size` 把每格 `idx` 填成 `palette.colors[idx].rgb`（越界见 D6）、画线、
  贴位图数字（D5）。**无 `f32`、无随机、无 `HashMap`/`HashSet`、无 `rayon`** → `RgbImage` 缓冲**跨架构（arm64/x86_64）
  逐位相同**。这是渲染确定性的真锚点。
- **编码层**（`encode_png`）：用 `::image::codecs::png::PngEncoder::new_with_quality(w, compression, filter)` **显式传
  具体（concrete）压缩等级与 filter 常量**——**钉死为命名常量** `const PNG_COMPRESSION: CompressionType =
  CompressionType::Fast;` + `const PNG_FILTER: FilterType = FilterType::Adaptive;`（**不写 `::Default`**——`CompressionType::Default`
  映射到 `png::Compression::Balanced` 多一层可被版本重映射的间接；`Fast` 直映 `png::Compression::Fast`）。输入是 `RgbImage`
  → 编码为 **8-bit RGB、非交错（non-interlaced）**（`PngEncoder` 默认即非交错、`RgbImage` 即 RGB8；显式声明此前置让 D3-golden
  的「解码回比较像素」round-trip 良基）。**不**靠 `RgbImage::write_to` 的隐式默认（默认值可能随版本变）。写入内存 `Vec<u8>`（无 I/O）。
- **golden 策略**：M5 的确定性测试 **(a)** 同跑两次 `render_*` 字节 `Vec<u8>` 相等（run-determinism，满足 ROADMAP
  done-when「跨跑字节一致」）；**(b)** **解码回 `RgbImage` 比较像素**等于手算期望（跨架构稳健、对 PNG 编码器版本漂移
  免疫；依赖 (a) 钉死的 8-bit-RGB-非交错路径做无损 round-trip）。**不**在 M5 单测里硬编码 PNG 字节串——逐字节 PNG golden
  与「依赖升级须重生成 golden」的取舍属 M7（M7 让 PNG golden「响亮失败」正是为抓故意算法/依赖变更）。
- **理由 + 确定性边界的诚实声明**：像素层是真确定性来源、可硬断言；PNG 字节**仅在锁定 `Cargo.lock` 的同一版本下逐字节稳定**
  ——**显式钉编码参数买的是「同版本同字节」+ 意图清晰，并不买跨版本字节稳定**：`image 0.25` 对 `new_with_quality` 明文声明
  「exact output is expressly **not** part of the SemVer stability guarantee」（已查证 `image-0.25.10` 源码 doc），即便钉死
  参数，跨 `image`/`png` 版本 deflate 实现仍可能改字节。故把 M5 单测钉在像素上既满足 done-when 又不脆。① ROADMAP done-when
  原文「produced and **byte-identical across runs** for the same input」——「across runs」（同二进制重复跑）由同版本编码确定性
  满足，(a) 直接覆盖。② INIT golden 清单含 `preview.png`，那是 M7 的 frozen 字节 golden（升级依赖须重生成），本 change 不抢跑。
- **替代方案：M5 即硬编码 PNG 字节 golden。** 否决：脆（依赖 bump 即炸且与算法无关）、与 M7 职责重叠；像素 golden 表达
  同一意图却跨版本稳。

**D4 — preview 渲染：`cell_size × cell_size` 实心方块，颜色取 `palette.colors[cells[i]].rgb`，行优先放置，无坐标。**

输出尺寸 `width*cell_size × height*cell_size`。格 `(x,y)`（`idx = cells[y*width+x]`）填 `palette.colors[idx].rgb`
（越界 idx → D6 哨兵色），占像素矩形 `[x*cell_size, (x+1)*cell_size) × [y*cell_size, (y+1)*cell_size)`。**无网格线、
无坐标、无 margin**（preview 是成品外观）。`shape==Square` → 整格填满；其它 shape 不在 M5。

- **理由**：直接落地 color-matching 前向约束（`从 cells[i] 查 palette.colors[idx].rgb 上色`），且渲染**只读 `cells`
  下标 + palette RGB**，绝不回看 `PixelGrid` 原始 RGB（M3-D6 冻结、规则 3）。行优先索引 `y*width+x` 复用 `BeadPattern`
  既有布局（与 `cell_at` 一致）。
- 长度/索引算术全程 `usize`（`y as usize * width as usize + x as usize`），同 `models` 对溢出的告诫。

**D5 — grid 渲染：完整钉死的整数几何（margin / 总尺寸 / 线位置 / 粗细优先级 / 标号锚点全部有公式）；珠子格 + 每格细线 + 每 10 格粗线 + 1-indexed 行/列号（内置 3×5 位图数字字体）；线色/字色/间隔为固定常量。**

> **为何整段公式化**（review M1）：早稿只说 margin「确定性算出」、线「额外占位」，但没给公式——导致 grid 总尺寸、cell 像素
> 原点、线坐标、粗细在 ×10 边界的叠加规则全不可计算，spec 的「第 10 边界有粗线像素」场景无法机械写测、两份合规实现会
> 像素级分叉、M7 golden 无从冻结。故此处把几何**逐条钉成整数公式**；并改「线额外占位」为**线叠加在格边缘像素上（overlay，
> 不额外占位）**——cell 原点遂为 `margin + i*cell`，坐标可算（代价：线吃掉边缘 1–2px 珠色，对拼装网格图可接受；preview.png
> 无线、保留纯色）。

**固定常量**（M5 不可配，D5 非目标；命名 `const`、确定性）：`BG=[255,255,255]`、`THIN=[200,200,200]`、`BOLD=[120,120,120]`、
`TEXT=[0,0,0]`、`BOLD_W: u32 = 2`、`STEP: u32 = 10`。`// ponytail: 固定常量；要可配再长 RenderOptions 字段（技术上破坏，靠 Default + ..default() 缓解，见 D2），不现在加`。
低对比特例（THIN 与浅灰珠同值、BG 白与白珠在 margin 邻接 → 线/框不可见）是已知、可接受的（固定常量取舍，配色化是 non-goal），见风险段。

**`render_grid` 要求 `cell_size >= 5`（review R2-M2，blocker 级两 major 的统一根治）**：grid 的坐标标号字形最小 5px 高、3 位标号
`num_w(3,scale=1)=11px` 宽。当 `cell < 5` 时 `scale=max(1,cell/5)=1`、行标号间距 `STEP*cell < 50` 但字形仍 5px 高、列标号间距
`STEP*cell < 50` 却挡不住 `num_w` —— 会发生**行标号竖向裁切**（末行标号越下图边，Codex）与**相邻列标号横向重叠**（`cell=1`:
`num_w(3)=11 > 间距 10`，Codex/CR/RC）。统一根治：`render_grid` **守卫 `opts.cell_size < 5 → Err(InvalidImage{reason})`**（与
`width==0` 等同列入 D7 守卫）。`cell >= 5` 下：列标号间距 `STEP*cell >= 50 >> num_w`（永不重叠）、行标号字形高 `5*scale <= cell`
（末行标号恰好落 `out_h` 内、不裁切）——所有「标号完整落 margin/不裁切/不重叠」保证**在允许域内无条件成立**。`render_preview`
**不**受此限（无标号/无线，`cell >= 1` 即可，含 1px/珠 缩略图）。

**位图字体**：手写 `const DIGITS_3X5: [[u8; 5]; 10]`（每字形 3 宽 × 5 高，每行低 3 位为像素）。整数缩放
`scale = max(1, cell / 5)`（grid 已守 `cell >= 5` → `scale >= 1`；`cell=10 → scale=2`，字形 6×10px）。一个 `d` 位十进制数
的像素宽 `num_w(d) = d*3*scale + (d-1)*scale = d*4*scale - scale`，高 `5*scale`。任意位数 `d`（含 ≥1000 的 4 位，可达——
**无尺寸上限**，下 R2-M3）由 `decimal_digits` 与 `margin_left` 公式吸收。**无字体渲染 crate**（D9）。

**几何公式（全整数、确定性；`cell = opts.cell_size`，`pad = scale`）。** **所有几何量（`scale`/`num_w`/`margin_*`/`out_*`/`bytes`）
必须由 `grid_geom_checked` 统一在 `u128` 算出、按 D7 定序校验（`out_* <= u32::MAX` 后才算 `bytes`）、通过后才 cast 回 `u32` 收进
`GridGeom { cell, scale, pad, margin_left, margin_top, out_w, out_h }`（**含 `cell`**——`paint_grid` 画格原点/块/线/标号都需它，review R4-M1），交
`paint_grid`——`paint_grid` 全程用 `g.cell`/`g.scale`/`g.margin_*`/`g.out_*`，**不**自行在 `u32` 里重算 margin/scale/cell（否则大 `cell_size`
下 `u32` 中间量先溢出，R3-B1/R3-M-ord）：**
- `has_col_labels = width >= STEP`；`has_row_labels = height >= STEP`。
- `max_row_label = 若 has_row_labels 则 (height/STEP)*STEP 否则 0`；`row_digits = decimal_digits(max_row_label)`（≥1 时取实际位数）。
- `margin_top = 若 has_col_labels 则 5*scale + 2*pad（= 7*scale）否则 0`（列号横排，只吃竖向条高）。
- `margin_left = 若 has_row_labels 则 num_w(row_digits) + 2*pad 否则 0`（行号需容下**最宽**行标号 → 多位数字必落在 margin 内，
  消除「标号比格宽、静默裁切」隐患，review CR/RC）。
- **cell `(x,y)` 左上像素 = `(margin_left + x*cell, margin_top + y*cell)`**，块 `cell×cell`，色同 D4（越界/缺格 → D6 哨兵）。
- **总尺寸**：`out_w = margin_left + width*cell`；`out_h = margin_top + height*cell`（线 overlay、不加尺寸）。
- **网格线（overlay，仅画在 cell 区域内、不入 label margin）**：竖边界 `bx ∈ 0..=width` 在 `x = margin_left + bx*cell`、
  **沿 `y ∈ [margin_top, out_h)`** 画线；横边界 `by ∈ 0..=height` 在 `y = margin_top + by*cell`、**沿 `x ∈ [margin_left, out_w)`** 画线
  （线不进顶/左 margin，故不污染坐标数字区）。**边界 `b` 是粗线 iff `b % STEP == 0`**（含 0 与末边界）：粗线 `BOLD`、宽 `BOLD_W`；
  否则 `THIN`、宽 1px。**`×STEP` 边界画 `BOLD` 取代（非叠加）`THIN`**（消歧，review F6）。
  - **末边界 / 越界像素守卫（review R2-B2，blocker——`put_pixel` 越界会 panic）**：末右/下边界 `bx==width`/`by==height` 落在
    `x=out_w`/`y=out_h`（= 图外第一像素，合法范围 `[0,out_w)`/`[0,out_h)`）；且粗线宽 `BOLD_W` 向内延伸也可能触界。**所有线像素写入
    必须经边界检查**（`get_pixel_mut_checked` / 显式 `x<out_w && y<out_h`，**禁裸 `put_pixel`**——已查证 `image-0.25.10`
    `put_pixel`/`get_pixel_mut` 越界即 `panic!`）：① 末右/下边界线**夹取到最后合法像素** `out_w-1`/`out_h-1`（保证外框始终存在、
    确定性，而非随 `width % STEP` 时有时无）；② 任何落在 `[0,out_*)` 外的线像素跳过。**否则非 STEP 倍数的 width/height（如 15/23/105，
    极常见）会让末 THIN 边界写到 `x=out_w` → panic**（与 D7「绝不 panic」矛盾，仅 STEP 倍数维度才偶然安全、掩盖此 bug）。
- **列号（1-indexed，每 STEP）**：对 `n ∈ {STEP, 2*STEP, …, ≤ width}`（即 `(STEP..=width).step_by(STEP)`），在顶 margin 画
  `n` 的十进制文本（**显示值恰为 `n`**，对应第 `n` 列、0-based 下标 `n-1`；消除「把 STEP 当下标→画成 11」的 off-by-one，review M3），
  **右对齐于其边界**：右边缘 `x = margin_left + n*cell`（= 第 `n` 列右边界 / 粗线处）、左边缘 `= margin_left + n*cell - num_w(digits(n))`、
  顶边缘 `y = pad`。**因 `n ≤ width` → 右边缘 ≤ `out_w`，绝不溢出右图边**（修早稿左对齐 `(n-1)*cell` 在末列 2 位标号会越右边裁切的自引 bug）；
  **因 grid 守 `cell >= 5` → 相邻列号间距 `STEP*cell >= 50 >> num_w`，恒不相撞**（R2-M2；早稿无条件「不相撞」在 `cell<5` 假，已由 cell≥5 守卫根治）。
- **行号（1-indexed，每 STEP）**：对 `n ∈ (STEP..=height).step_by(STEP)`，在左 margin 画 `n`，**右对齐**于右边缘
  `x = margin_left - pad`（左边缘 = 右边缘 − `num_w(digits(n))` ≥ `pad` ≥ 0，落在 margin 内）、顶边缘 `y = margin_top + (n-1)*cell`
  （**因 cell≥5 → 字形高 `5*scale <= cell`、末行标号 `(n-1)*cell + 5*scale <= n*cell <= out_h` 恰落图内、不竖向裁切**，R2-M2/Codex）
  （高 `5*scale ≤ cell`（`cell≥5`）→ 不越下图边；`cell<5` 时由 `draw_number` 边界裁剪兜底、不 panic）。
- **`draw_number` 仍做逐像素边界检查**（任何像素落在图外即跳过、不 panic）作为兜底——但上面 margin 公式已保证标号必然落在图内，
  故正常输入下不发生裁切。

- **理由**：每格细线 + 每 STEP 加粗 + 隔 STEP 标号是拼豆/十字绣**通用对位法**，用户已选此口径。位图字体让坐标数字**零字体
  依赖**且**确定性逐位**（TrueType 渲染含 hinting/抗锯齿、跨版本不稳、且要新 crate）。overlay 线模型让几何可算、可机械写测、可被
  M7 冻结。常量而非配置：确定性 + 简单（ponytail ladder：值从不变 → 不做 config），字段可后加。
- **替代方案：线额外占位（早稿）。** 否决：cell 原点不再是 `margin+i*cell`、几何与测试坐标依赖未定义的占位累加，两实现分叉；
  overlay 模型几何闭合、可测，仅以边缘 1–2px 珠色为代价（grid 是拼装辅助、非成品图）。
- **替代方案：引 `ab_glyph`/`fontdue` 渲染数字。** 否决：新依赖 + 抗锯齿/hinting 破坏跨架构逐位确定性 + 体积；3×5 位图
  常量几十字节、纯整数、够清晰。
- **替代方案：每格都标号 / 0-indexed。** 否决：每格标号噪声且图巨大（用户选「每 STEP」）；0-indexed 反人类摆豆习惯（用户选
  1-indexed）。代码内 `cells` 仍 0-based，仅显示 `n`（= 第 n 列/行）。

**D6 — 越界 palette 下标 + 过短 `cells`：哨兵色容错，不 panic（statistics D4 的渲染化身；覆盖两类 pub-可构造的不变量违约）。**

取色用**两层 `.get()`、皆不裸索引**——`paint_*` 按位置 `pos = y*width + x` 取格、再按格取色：
```rust
let rgb = grid.cells.get(pos)                               // ① 过短 cells（pos 越界）→ None
    .and_then(|&idx| palette.colors.get(idx as usize))     // ② 越界 palette 下标 → None
    .map(|c| c.rgb)
    .unwrap_or(MISSING);                                   // 任一缺失 → 哨兵
```
`const MISSING = [255, 0, 255]`（品红，传统「缺纹理」色）。**两类可达违约都不 panic、都落同一哨兵**：
- **① 过短 `cells`**（`cells.len() < width*height`）：`BeadPattern` 字段 `pub`、`models/mod.rs` 明文「调用方负责
  `cells.len()==width*height`」，故手构一个短 `cells` 是可达输入；渲染按 `pos=y*width+x` **空间索引**（不同于 statistics 直接
  `for &idx in &cells` 迭代、天然安全），裸 `cells[pos]` 会 panic（review M2/Codex）。用 `cells.get(pos)` → 缺格画哨兵。
- **② 越界 palette 下标**（传比配色时更小/不同的 palette，`idx >= colors.len()`）：同 M4-D4，裸 `colors[idx]` 会 panic → `get` 跳过。

- **文档化前置条件**：`palette` 应是产出该 `BeadPattern` 的 matcher 用的同一份未改动 palette，且 `cells.len()==width*height`
  （同 `models`/`match_pattern` 口径）。**违约不 panic、产出确定性「缺格哨兵」可观测信号**（图上品红块一眼可见「palette 配错/
  pattern 损坏」，同 M4「Σ count < total」）——比崩溃或静默错色诚实。
- **理由**：① core 不在可达输入上 panic（M2-D5′/M3-D7/M4-D4）。② 哨兵 `const` → 确定性。③ 一处 `.get().and_then(.get())`
  同时收口两类违约，无额外分支。
- **替代方案：违约返 `Err` / 入口校验 `cells.len()`。** 否决：与「渲染是全绘制、只在维度/编码失败时 fallible」框架冲突，且单格
  缺失不该让整图失败；哨兵让损坏**可见**而非阻断（M4「跳过+信号、不 `Result`」同姿态）。空 palette（`colors.len()==0`）是 ② 的
  退化特例：每格皆越界 → 整图哨兵（仍不 panic）。

**D7 — 边界/退化：空网格、`cell_size==0`、grid 的 `cell_size<5`、总缓冲过大 → 确定性 `Err(InvalidImage { reason })`，绝不 panic。**

`render_*` 入口按序守卫，命中即返 `Err(BeadError::InvalidImage { reason })`（`reason` 确定性、点名维度）：
- **空网格**：`grid.width == 0 || grid.height == 0`（无可渲染面积）→ `Err`。0×0 PNG 无意义；返确定性 `Err` 而非 panic，
  满足 ROADMAP「不 panic」（注：返 `Err` 不是 panic）。
- **`cell_size == 0`**：无面积 → `Err`（preview 与 grid 同）。
- **`render_grid` 专属：`cell_size < 5` → `Err`（review R2-M2）**：grid 坐标标号需 `cell >= 5`（详上 D5：否则行标号竖向裁切、
  列标号横向重叠）。`render_preview` **不**设此限（无标号，`cell >= 1` 即可，含 1px/珠 缩略图）。
- **几何+总缓冲尺寸守卫：全程 `u128`、含 margin、严格定序（review R2-B1 + R3-B1 + R3-M-ord，blocker）**：守卫必须在**单一函数**
  `grid_geom_checked(width, height, cell) -> Result<GridGeom, BeadError>` 里**用 `u128` 算出全部几何**（`scale, pad, row_digits,
  num_w, margin_left, margin_top, out_w, out_h`），**禁止先在 `u32` 里算任何 margin/scale 中间量**——否则大 `cell_size`（无上限、pub
  可构造）下 `margin_top=7*scale`、`margin_left=num_w(row_digits)+2pad` 等 `u32` 乘法**在守卫之前就溢出**（如 10×10 grid、`cell≈2.39e9`
  → `9*scale > u32::MAX`，debug panic / release wrap，R3-B1，三审一致）。**定序是封闭性的前提（R3-M-ord）**——`u128` 对几何**并非**
  "对任何输入无条件封闭"：在 `out_w/out_h` 被夹到 `<= u32::MAX` **之前**算 `3*out_w*out_h`，其上界 `≈ 3 * (u32²)² ≈ 1.0e39 > u128::MAX
  (3.4e38)`，**连 `u128` 都会溢出**。故守卫**必须按此序**：
  ① 用 `u128` 算 `out_w = margin_left + width*cell`、`out_h = margin_top + height*cell`；**先**判 `out_w > u32::MAX || out_h > u32::MAX
  → Err`（图像维度须 `u32`；此判**也**夹住 margins，因 `margin_* <= out_*`）。
  ② **仅在 ① 通过后**（此时 `out_w,out_h <= u32::MAX`）算 `bytes = 3 * out_w * out_h`（`u128`，上界 `3 * u32::MAX² ≈ 5.5e19 << u128::MAX`，
  **不溢出**），`bytes > isize::MAX as u128 → Err`。
  ③ 全部通过后才把 `scale/margins/out_w/out_h` cast 回 `u32`（均 `<= u32::MAX`，cast 安全）、交 `paint_grid` 绘制。全在 `RgbImage::new`
  之前。preview 用 `preview_dims_checked`（`margin_*=0` 的同序同 `u128` 公式）。**精确归因（review R2-m1）**：`RgbImage::new`
  (=`ImageBuffer::new`) 内部 `image_buffer_len = 3*w*h`（`usize` `checked_mul`）返 `None` 时 `.expect()` **panic**（`3*w*h` 溢出 `usize`）；
  而**虽容于 `usize` 但巨大**（如 ~12GiB，Codex 举）则到 `vec![…; size]` **OOM-abort**（非 `.expect`）。`isize::MAX` 守卫挡的是
  **① 字节数溢出 `usize`（否则 `image_buffer_len` 的 `.expect` panic）+ ② 字节数 `> isize::MAX`（Rust 分配硬上限、必失败）**；它**不**挡
  **③ 字节数 `<= isize::MAX` 却超本机内存** 的 OOM-abort（= 已接受的 R4-B1 限制，见下）（已查证 `image-0.25.10` `images/buffer.rs`）。
  **注（review nit）**：`isize::MAX` 随目标位宽变（64-bit≈9.2e18、32-bit≈2.1e9）；
  确定性门的目标是 arm64/x86_64（64-bit），守卫按 64-bit `isize::MAX` 算；32-bit（M8 `armeabi-v7a`）下 Ok/Err 边界在无可达豆板触及的尺寸上
  略移，对既定目标无正确性影响。
  > **已知边界（review R4-B1，用户已裁定：接受、不加上限）**：`isize::MAX` 守卫挡住「Rust 必失败」的分配，但**容于 `isize::MAX` 却仍超
  > 机器内存**的巨尺寸（如 pub-构造 `width=height=1.5e9, cells:vec![]`、`cell=1` → `bytes≈6.75e18 < isize::MAX` → 守卫放行 → `RgbImage::new`
  > 的 `vec!` 触 **OOM-abort**）M5 **不**拦截。判定（用户 2026-06-24 明确选「记为已知限制、不加上限」）：① 仅经**蓄意违反
  > `cells.len()==width*height` 不变量 + 荒谬维度**的 pub 构造可达，**管线不可达**（M6 维度来自 M2 resize 目标，CLI 校验为正常值）；②
  > **与既有引擎一致**——M2 `resize_image` 同样无尺寸上限、对同类荒谬目标也会 OOM（已查证 core 无任何 byte/dim cap）；③ 确定性的修法是
  > **任意的 `MAX_RENDER_BYTES` 策略上限**（为一致应施于全引擎、非 M5-only），属新增配置/策略、越出本次议定范围；`try_reserve` 修法机器
  > 相关、违确定性门。故 M5 记为**已接受的已知限制**，不加尺寸上限（若将来要，应作引擎级跨切面策略另开 change）。
- 复用 M2 既有 `InvalidImage { reason }`（image 模块零维/越界裁剪已用同变体），**不**为这些维度类失败新增变体。

- **理由**：① 维度类失败与 image 模块的零维/`u64`-加宽守卫同性质，复用同变体 + 同加宽手法保持错误面与风格一致。② 空网格在
  statistics（M4）产 `[]`/`Total Beads: 0` 是合理的「空清单」，但**空图**无字节可言——确定性 `Err` 比伪造 1×1 占位图诚实、且可测。
  ③ 所有守卫确定性 → 同输入同 `Err`，可断言（debug 与 `--release` 均不 panic）。
- **空网格的 M6 协调（review m4，已查证非矛盾）**：`render_*` 对空网格返 `Err`、而 M4 statistics 对同一空网格返 `Ok([])`/
  `Total Beads: 0`——**二者只在「管线不可达」的输入上分歧**：M6 `pipeline::generate_pattern` 在配色前必过 M2 `resize_image`，而
  `resize_image` 对 `width==0 || height==0` 的目标**已先返 `InvalidImage`**（已查证 `image/mod.rs:179-188`），故管线**永不产出空
  `BeadPattern`**。空网格分歧仅发生在「手构空 `BeadPattern` 直喂库原语」的误用上：此时 render 给确定性 `Err`、stats 给 `Ok([])`，
  各自是其原语在不可达输入上的诚实边界、**无需在 M5 强行统一**（强行让 render 也对空网格返 `Ok` 需伪造占位图，更差）。写进 Open
  Questions 作 M6 前向约束备忘（M6 若真要直收手构 pattern，再决定统一口径）。
- **替代方案：空网格产 1×1 透明 PNG。** 否决：任意/有损、且引入透明通道（非目标）；`Err` 表达「无可渲染内容」更准。

**D8 — 错误模型：维度类复用 `InvalidImage { reason }`；新增 `ImageEncode` 变体（PNG 编码失败）；`#[non_exhaustive]` 加变体非破坏。**

```rust
// lib.rs，BeadError 内新增（ImageDecode 已占用 #[from] ::image::ImageError，故新变体用具名字段、不重复 #[from]）：
#[error("failed to encode image")]
ImageEncode { source: ::image::ImageError },
```

- **理由**：① `render_*` 的真可达失败有两类：**维度**（D7，复用 `InvalidImage`）与 **PNG 编码**。② PNG 编码对「已守卫维度的
  合法内存缓冲」实际上不会失败（无 I/O），但 `image`/`png` 的编码 API 仍返 `Result<(), ImageError>`——core **不在可达 API 上
  `.expect()`/panic**（同房屋风格），故须有 `BeadError` 路径承接其 `Err` 臂。**注（review m3）**：`PngEncoder::write_image` 另有
  一条 `#[track_caller]` 的 `assert_eq!`（缓冲长度 ≠ `3*w*h` 即 panic，已查证 `image-0.25.10` `codecs/png.rs`）——这是与 `Result`
  **并列的 panic 路径，`ImageEncode` 不接它**；但 `encode_png` 喂的是 `paint_*` 产出的 `RgbImage` **自身缓冲**（长度按构造恒
  `== 3*out_w*out_h`、不变量成立），故该 `assert_eq!` **不可达**，`ImageEncode` 只需接 `ImageResult::Err` 臂——「不在可达 API 上
  panic」的声明据此成立、不被这条 assert 推翻。③ 不能复用 `ImageDecode`（语义是「解码」且 `#[from]` 已绑 `ImageError`，同类型不能
  二次 `#[from]`）；新变体用 `{ source }` 具名字段手动包裹。④ `BeadError` 已 `#[non_exhaustive]`，加变体**非破坏**（同 M4 留门心智）。
- **替代方案：编码失败 panic（`.expect`）。** 否决：违「core 在可达输入不 panic」；虽守卫后近乎不可达，仍给确定性 `Err` 路径。
- **替代方案：编码失败塞进 `InvalidImage { reason }`。** 否决：混淆「维度非法」与「编码失败」两类、丢失底层 `ImageError`
  来源；`ImageEncode { source }` 与 M2 `ImageDecode` 对称、可诊断。

**D9 — 依赖与字体：复用 `image`（0.25，png feature 已开），零新 crate；数字字体手写 `const` 位图。**

- **零新依赖**：解码（M2）已用 `image` 的 `png` feature；编码用**同 crate 同 feature**的 `PngEncoder`。绘制是直接写
  `RgbImage` 像素缓冲（**边界检查写入**：`get_pixel_mut_checked` 或显式 `x<out_w && y<out_h` 守卫——**禁裸 `put_pixel`/`get_pixel_mut`**，
  它们越界即 `panic!`，见 R2-B2），无需绘图 crate。
- **字体**：`const DIGITS_3X5: [[u8;5];10]` 手写位图（D5），**不**引 `ab_glyph`/`rusttype`/`fontdue`（字体渲染 crate 会带
  抗锯齿/hinting/查表 → 破坏跨架构逐位确定性 + 增依赖/体积，违 D3）。
- `renderer` 依赖 `models`（`BeadPattern`）+ `palette`（`Palette`）+ `::image`（编码/像素缓冲），**不**依赖 `matcher`/
  `statistics`（依赖方向干净，渲染是真理源模型的另一叶子消费者，与 statistics 平级）。
- **理由**：image crate 已在树内且解码/编码同源；手写位图字体是几十字节整数常量，比任何字体 crate 都更确定、更小。

**D10 — M5 须钉死的边界（确定性门 / golden 清单）。**

1. **preview 尺寸与逐格上色**：`w×h` grid + `cell_size=c` → preview 像素尺寸 `(w*c)×(h*c)`；解码回 `RgbImage`，断言每
   像素 `(x*c+dx, y*c+dy)` 等于 `palette.colors[cells[y*w+x]].rgb`（抽样若干格 + 边角）。
2. **grid 几何精确（按 D5 公式可机械计算；用 `cell>=5`）**：构造**严格 `>10`** 宽高（如 13×13，使第 10 边界为内部、不被末边界夹取，CR nit-1）、
   `cell=10`(scale=2) 的 grid，断言（解码像素）：
   ① 解码尺寸 `== (margin_left + w*c) × (margin_top + h*c)`（按 D5 公式手算）；② 第 10 竖/横边界 `x/y = margin_* + 10*c` 处为
   `BOLD` 像素、第 1..9 边界为 `THIN`；③ 列号 "10" 的**确切位图像素**落在其**右对齐锚点**——占 `[margin_left+10*c − num_w(2),
   margin_left+10*c)`、`y=pad`——为 `TEXT`（断言字形像素，证明确为 "10"、非 "11"/错位/越右边，review M3/Codex/CR + R2-M1 修正
   早稿左对齐 `margin_left+9*c` 的 stale 锚点）；④ 维度 `<10` 的轴不画该轴标号、margin 为 0，但仍出图、不 panic。
2b. **多位/≥100 标号落在 margin 内**（review CR/RC F-RC5）：构造一维 ≥100、`cell>=5` 的 grid，断言行号 "100" 的字形像素**全部**落在
   `margin_left` 内（不裁切、不越界、不 panic），证明 margin 按 `row_digits` 撑够。
2c. **非 STEP 倍数维度不 panic（review R2-B2，blocker——末边界线 OOB）**：构造 `width`、`height` **均非 10 的倍数**（如 13×17，
   `cell>=5`）→ 末右/下 THIN 边界落 `out_*` 外，断言返 `Ok`、解码尺寸正确、**debug 与 `--release` 均不 panic**（线写入经边界检查/
   夹取，非裸 `put_pixel`）。
3. **越界 palette 下标 + 过短 cells → 哨兵色，不 panic**（D6）：(a) 手构 `cells` 含越界下标 + 更小 palette → 越界格解码为
   `MISSING=[255,0,255]`、不 panic；(b) **手构过短 `cells`**（`len < w*h`）→ 缺格解码为 `MISSING`、不 panic（review M2）；
   (c) 空 palette 子断言：整图哨兵色、不 panic。
4. **退化 → 确定性 `Err`，不 panic**（D7）：`width==0`、`height==0`、`cell_size==0` 各断言返 `Err(InvalidImage{..})`；
   **`render_grid` 专属**：`cell_size ∈ {1,2,3,4}` 断言返 `Err(InvalidImage{..})`（R2-M2），而**同样 `cell_size` 下 `render_preview`
   返 `Ok`**（preview 不设 cell≥5）（均 debug 与 `--release` 不 panic）。
4b. **总缓冲溢出/过大 → `Err`，不 panic（review R2-B1/R3-B1/R3-M-ord，blocker——守卫全程 `u128`、含 margin、定序）**：pub-构造
   超大 `BeadPattern{cells:vec![]}` 多例，均断言返 `Err(InvalidImage{..})`、**debug 与 `--release` 均不 panic**（不触达 `RgbImage::new`）：
   (i) **preview** `width=height=u32::MAX` + `cell_size=1`（`out_w=out_h=u32::MAX`，`3*out²` 在 `u64`/`u128`-若先乘 都溢出，定序下
   `out<=u32::MAX` 通过、`bytes=3*u32²≈5.5e19>isize::MAX` 命中）；(ii) `width=height=250_000_000` + `cell_size=10`；
   (iii) **R3-B1 角（grid margin u32 溢出）**：`10×10` grid + `cell_size≈2.39e9`（使 `9*scale > u32::MAX`，即 `margin_left` 在 `u32`
   下会溢出）→ 必须返 `Err`、不 panic（证明 margin 在 `u128` 算、不在 `u32` 先溢出）；(iv) **R3-M-ord 定序锁**：`width=height=cell=u32::MAX`
   （`out_* ≈ 1.84e19`，预夹 `3*out² ≈ 1.0e39 > u128::MAX 3.4e38`）→ 返 `Err`（正序下 ① `out_*>u32::MAX` 立即 `Err`、永不算 bytes；**误把
   `bytes` 提前算的实现会在 `u128` 乘法处 panic**，本测据此捕获错序。注：「out 仅略超 u32::MAX」抓不到——那时 `3*out²≈5.5e19<u128::MAX`
   不溢出，R4-M2 修正）。
5. **确定性双层**（D3）：(a) 同 `(grid,palette,opts)` 两次 `render_preview`/`render_grid` 的 `Vec<u8>` **字节相等**；
   (b) 解码回的像素缓冲等于手算期望（跨架构整数稳）。`// ponytail: PNG 跨版本可能漂、像素跨架构逐位稳，故字节测同跑相等、
   像素测绝对值；硬字节 golden 留 M7`。

## 风险 / 权衡

- [PNG 字节跨依赖版本可能漂移（D3）] → M5 单测钉**像素**（跨架构稳）+ **同跑字节相等**（run-determinism），不硬编码 PNG
  字节；逐字节 frozen golden + 「dep bump 须重生成」归 M7（M7 让其响亮失败正是为此）。
- [越界 palette 下标静默错色而非报错（D6）] → 是对契约违约输入的诚实退化（图上品红块即可观测信号，同 M4「Σ<total」）；
  彻底诊断（等长但内容不同的 palette）需 M6 持单一 `Palette` 同喂 matcher/stats/render（M4-D4 已记此管线不变量），非渲染层
  能担。哨兵色 `const` 钉死、测试断言其值防回归。
- [`Renderer` trait 推迟（D1）vs 设计规则「可替换算法走 trait」] → 规则意图是别为变体 fork pipeline；M5 一个函数 + `shape`
  字段零分叉即满足。珠形扩展走 `BeadShape`（数据轴）；真出现第二渲染**策略**再非破坏引 trait（不改 `generate_pattern`）。
- [小 `cell_size` 下 grid 标号裁切/重叠（review R2-M2，两 major，已修）] → `render_grid` 守 `cell_size >= 5`（`<5 → Err`）：cell≥5 下
  列标号间距 `STEP*cell>=50 >> num_w`（不重叠）、行标号字形高 `5*scale<=cell`（不竖向裁切），所有「落 margin/不裁切/不重叠」保证无条件
  成立。`render_preview` 不设此限（无标号，cell≥1，含 1px 缩略图）。D10.4 钉死 grid `cell<5→Err` 而 preview 同 cell `Ok`。
- [**总缓冲/几何溢出 → 可达 panic（review R2-B1→R3-B1→R3-M-ord，blocker，三轮收敛已修）**] → 真 panic 面是 `RgbImage::new` 的
  `image_buffer_len=3*out_w*out_h`（usize）`.expect`。三轮收敛：R2 用 `u64` 算 `3*out_w*out_h` **守卫自身溢出**（`>u64::MAX`）；R3-B1
  发现 **margin/scale 若在 `u32` 算会先于守卫溢出**（大 `cell_size`，如 10×10、`cell≈2.39e9` → `9*scale>u32::MAX`）；R3-M-ord 发现 **即便
  `u128`，若在夹 `out_*<=u32::MAX` 前算 `3*out²` 也会溢出 `u128`**（`≈1.0e39>u128::MAX`）。终态：单一 `grid_geom_checked` **全程 `u128`、
  含 margin/scale、严格定序**——① 算 `out_w/out_h`（u128），**先**判 `>u32::MAX → Err`（同时夹住 margins）；② **仅在 ① 后**算
  `bytes=3*out_w*out_h`（此时 `<=5.5e19<<u128::MAX`），`>isize::MAX → Err`；③ 通过后才 cast `u32` 交 `paint_grid`，全在 `RgbImage::new`
  前（`isize::MAX` 挡 usize-溢出 `.expect` 与 `>isize::MAX` 的必失败分配；`<=isize::MAX` 却超内存的 OOM-abort 是已接受的 R4-B1 限制、不挡）。D10.4b 钉死四例（含 R3-B1 margin 角 + R3-M-ord 定序锁）→ Err 不 panic
  （debug+release）。**无尺寸上限**（review R2-M3：INIT 无 300×300 cap，那只是 Criterion 基准最大档；守卫按全 `u32` 域设），守卫只为挡
  pub-构造病态输入；4 位标号（height≥1000，可达）由 `decimal_digits`+`margin_left` 公式自然吸收。
- [过短 `cells` / 越界 palette 下标 → 可达 panic（review M2，已修）] → 渲染按 `pos=y*w+x` 空间索引，裸 `cells[pos]`/`colors[idx]` 会 panic；
  D6 改用两层 `cells.get(pos).and_then(colors.get)` → 缺格/越界哨兵。D10.3 钉死。
- [线写入越界 panic（review R2-B2，blocker，已修）] → `image` `put_pixel`/`get_pixel_mut` 越界即 `panic!`（已查证）；末右/下边界落
  `out_*`（图外）、非 STEP 倍数维度（如 13/17/105，常见）的末 THIN 边界会触发。D5 改为**所有线写入经边界检查/夹取到 `out_*-1`**、禁裸
  `put_pixel`。D10.2c 钉死「非 10 倍数维度 → Ok 不 panic」。
- [grid 几何早稿未定义（review M1，已修）] → D5 现给全套整数公式（margin/总尺寸/线位/粗细优先/标号锚点）+ overlay 线模型，
  使几何可机械计算、spec 场景可写确定性测试、M7 可冻结；列标号**右对齐于边界**（修自引越右边裁切 bug）、D10.2 按公式断言确切像素。
- [empty-grid M5(Err)/M4(Ok) 分歧（review m4，非矛盾）] → 已查证 M2 `resize_image` 拒 0 目标 → 管线永不产空 pattern；分歧仅在
  手构空 pattern 直喂原语的误用上，各原语在不可达输入上的诚实边界，无需在 M5 统一（见 D7 + Open Questions）。
- [低对比固定线色（review n2）] → THIN 与浅灰珠同值、BG 白与白珠在 margin 邻接时线/框不可见；固定常量取舍下可接受、配色化是
  non-goal；记此已知特例（D5 常量段），真要可配再非破坏长 `RenderOptions`。

## Migration Plan

无运行时迁移：纯新增能力 `renderer` + 新增 `RenderOptions`/`BeadShape` + 一个非破坏 `BeadError::ImageEncode` 变体
（`#[non_exhaustive]`）。**不改**任何已生效规范需求（color-matching/statistics 对 M5 的提及是前向约束，由本 change
落地、文字不变，见 proposal「修改功能：无」）。**不改** ARCHITECTURE/INIT/ROADMAP（均已与设计一致）。回滚 = 撤销本
变更（删 `renderer` 模块、回退 `RenderOptions`/`BeadShape`/`ImageEncode` 变体与重导出）。写盘/串联/硬字节 golden 是
既定后续（M6/M7），非本变更。

## Open Questions

- **位图字体的精确字形位（`DIGITS_3X5` 的 10 个 5 字节常量）** = 实现细节，tasks 给出一套标准 3×5 字形；只要清晰 + 确定，
  具体位非规范层关切（spec 只要求「数字存在、1-indexed、每 10、确定性」）。
- **`RenderOptions` 是否最终 `Copy` / 按值传** = 已定按 `&RenderOptions`（与用户给的签名一致；`Copy` derive 仅为便利、
  使按值亦可，不影响契约）。无 open 项。
- **PNG 编码的压缩等级/filter 具体取值** = **已定（review m1）**：钉死命名常量 `PNG_COMPRESSION = CompressionType::Fast`
  + `PNG_FILTER = FilterType::Adaptive`，输出 8-bit RGB 非交错（D3）；取值不影响**同版本**确定性（只要固定），仅影响字节，
  跨版本字节不保证（upstream 明文）、M7 frozen 时锁。
- **M6 对空/手构 pattern 的口径** = M5 只承诺「管线（经 M2 resize）永不产空 pattern，故 render 的空→Err 守卫是不可达兜底」
  （见 D7）；M6 若要直收手构 pattern 再定统一口径（前向约束，非 M5）。
