## 上下文

里程碑 M3。M1 已建 `palette`（`Palette { brand, colors: Vec<PaletteColor> }`、`PaletteColor
{ code, name, rgb:[u8;3] }`）：`load_palette` 在解析期校验**合法 hex + 非空 colors + 唯一 code**
（hex 解析成 `[u8;3]`），`validate_palette` 只复检结构不变量（**非空 colors + 唯一 code**；hex 已是
类型保证，无法也无需复检）。注意 validate **不**保证 RGB 唯一——仅 code 唯一（见 D3 精确命中的平局处理）。M2 已建 `image` + `models::PixelGrid`（行优先原始 RGB 中间体）。M3 在 `bead-core`
新增 `matcher` 模块与第二批 `models`：把 `PixelGrid` 逐格映射到最近的真实豆色，产出 `BeadPattern`。
约束不变：纯库、确定性是门、`thiserror + Result<T, BeadError>`、字节进数据出、`BeadPattern` 自此
成为真理源。

设计经探索阶段拍板（Software Architect 提议 + 主 agent 复核纠正——尤其纠正了「为 golden derive
`Eq`」的错误理由：`assert_eq!` 用 `PartialEq` 即可）。

**M3 的关键特殊性**：匹配是**纯整数运算**（`[u8;3]` 差的平方和，无 `f32`），跨架构逐字节一致是
**保证**而非「赌它相同」——对比 M2 `Lanczos3`（`f32`，D8 因此不敢硬编码跨架构 golden）。这一点
贯穿 D3/D8。

## 目标 / 非目标

**目标：** `matcher` 模块（`ColorMatcher` trait + `RgbMatcher` + `find_best_match`）；`match_pattern`
入口（`PixelGrid → BeadPattern`）；`BeadPattern` 模型；RGB 平方欧氏距离匹配；确定性平局规则；
跨架构位精确 golden。

**非目标：** `stats`/`ColorStat`（M4）、CIELAB/ΔE（Phase 2，behind trait）、量化/抖动（Phase 2）、
preview/grid 渲染（M5）、`pipeline::generate_pattern`（M6）、离色阈值/警告（Phase 2，`find_best_match`
不回距离）、`rayon`（Phase 2）、`BeadCell` newtype（用裸 `Vec<u16>`，见 D1）。

## 决策

**D1 — 输出模型：`BeadPattern { width, height, cells: Vec<u16> }`，行优先、无 per-cell 坐标、
不 derive `Eq`、无 `BeadCell` 类型。** 公开：

```rust
/// 配色后的图案，自此成为真理源（CLAUDE 规则 3 / D6）。行优先：
/// `cells[y*width+x]` 是格 (x,y) 的调色板下标。不变量 `cells.len() == width*height`
/// （长度/下标一律 usize 运算）。M3 不含 `stats` 字段（见 D4）。
#[derive(Debug, Clone, PartialEq)]
pub struct BeadPattern {
    pub width: u32,
    pub height: u32,
    pub cells: Vec<u16>, // 行优先调色板下标
}

impl BeadPattern {
    /// (x,y) 处的调色板下标，越界返回 None。索引用 usize。
    pub fn cell_at(&self, x: u32, y: u32) -> Option<u16>;
}
```

- 替代方案 (a)：忠于 ARCHITECTURE 的 `Vec<BeadCell{x,y,color_index}>`。否决：与 M2 `PixelGrid`
  的核心取舍冲突——`PixelGrid` 行优先无坐标，坐标可由 `y*width+x` 推出（每格省 ~8 字节；80×100
  省 ~64KB；300×300 省 ~720KB）。`PixelGrid → BeadPattern` 是相邻两环，用相反存储约定制造无谓
  转换成本；`{x,y}` 是可由位置完全推导的冗余数据。**本 change 内同步修订 ARCHITECTURE.md /
  ROADMAP.md / INIT.md / models doc-comment**（见 proposal「变更内容」）。
  - 前向说明：M2 归档 design D1（`archive/2026-06-23-add-image-grid/design.md`，不可变）当时以
    「`BeadPattern` 是 `Vec<BeadCell{x,y,color_index}>`，与 `Vec<[u8;3]>` 结构不同」论证 M2 不能复用
    `BeadPattern`。M3 把 `cells` 改成 `Vec<u16>` 后该「结构不同」论据部分消解，但两类型仍正当独立——
    区分依据从「存储形状」转为「元素语义」（`PixelGrid` 是 `[u8;3]` 原始色，`BeadPattern` 是 `u16`
    调色板下标）。归档不可改，此处仅作前向记录。
- 替代方案 (b)：`BeadCell { color_index: u16 }` 一字段 newtype。否决（ponytail）：既然砍到一个
  字段，一个无方法、无不变量的 newtype 偏仪式感；`Vec<u16>` + 文档更瘦。**M4 计数用 `color_index`
  (u16) 当键、M5 渲染读下标，都与 `Vec<u16>` 直接工作。** `ponytail:` 升级路径——若将来 cell 真要
  携带 per-cell 数据（去 newtype 是破坏性，加字段也是），再引入 newtype。
- **不 derive `Eq`（同 M2 `PixelGrid`，刻意一致）**：golden / M7 / M8 比较两个 `BeadPattern` 用
  `assert_eq!`，只需 `PartialEq`，**不需要 `Eq`**；`Eq` 仅在「拿 pattern 当 `HashMap`/`BTreeMap`
  键」时才需，而 M4 计数是用下标 `u16` 当键、非 `BeadPattern`。故按 M2「`Eq` 是公开承诺，YAGNI，
  加 `Eq` 非破坏、去掉才破坏」推迟，真有哈希键需求再加。

**D2 — `ColorMatcher` trait：M3 即立 + 一个 `RgbMatcher`（自持 palette 快照），`find_best_match
(&self, [u8;3]) -> u16`。**

```rust
pub trait ColorMatcher {
    /// 返回调色板中与 `target` 最接近的颜色下标。全函数：palette 非空时对任意 [u8;3] 都返回有效下标。
    fn find_best_match(&self, target: [u8; 3]) -> u16;
}

pub struct RgbMatcher { /* 构造时缓存 Vec<[u8;3]> 等，见 D3 */ }
impl RgbMatcher {
    /// 从调色板构造（取一次性顺序快照）。空 / >65536 色 → `BeadError::InvalidPalette`（见 D7）。
    pub fn new(palette: &Palette) -> Result<RgbMatcher, BeadError>;
}
impl ColorMatcher for RgbMatcher { /* 平方欧氏 + 最低下标平局 */ }
```

> **Object-safety 是 `ColorMatcher` 的载重契约**：M3 即立 trait 是为 Phase 2 的 Lab matcher 留接缝，
> 而 D5 取 `&dyn ColorMatcher`（M6 持 `Box<dyn>`）——这要求 `ColorMatcher` **永久保持对象安全**。
> 未来扩展只能走对象安全的方法签名（如下 (c)），**不得**引入泛型方法 / `Self`-返回 / 签名里用关联类型，
> 否则破坏 `dyn` 派发。这是记录在案的契约，不是偶然。

- 替代方案 (a)：只给裸 `pub fn find_best_match(palette, target)`，真有第二个 matcher 再抽 trait。
  否决：**三处真理源**明确把 `ColorMatcher` 列为既定接缝——ARCHITECTURE matcher 段（直接写
  `trait ColorMatcher`，Phase 2 CIELAB「behind the existing traits」）、ARCHITECTURE「Future Plugin
  Architecture」段（`ColorMatcher` 是三大可换 trait 之一，承诺「RGB/Lab/Custom Matcher 不改
  pipeline」）、ROADMAP M3（任务原文「`ColorMatcher` trait + `find_best_match`」）。Phase 2 的 Lab
  matcher 是**已知的第二实现**，不是投机抽象——这正是 ponytail「YAGNI 指不知道会不会要；这里白纸
  黑字要 Lab」的反面。trait 抽象本身一行、零静态成本；运行时的 `dyn` 派发成本是 D5 选 `&dyn` 的取舍，
  与是否立 trait 无关（见 D5：每格一次 vtable，被内层 palette 扫描摊薄）。
- 替代方案 (b)：matcher 无状态、`find_best_match` 每次传 `&Palette`。否决：自持 palette 更顺——
  M6 pipeline 持 `Box<dyn ColorMatcher>` 反复调，无状态版每次传 `&Palette` 是噪声参数；自持让
  `new` 成为**预计算接缝**（D3 缓存 `Vec<[u8;3]>` 避免每格穿过 `PaletteColor` 的 `String` 字段、
  提升缓存局部性；Lab matcher 将来在 `new` 预转 Lab 空间是刚需）。
- 替代方案 (c)：返回 `(u16, 距离)` 留离色警告。否决：Done-when 无阈值语义，离色策略是 Phase 2/
  产品问题（YAGNI）。**留缺口，不留假接缝**——这里并没有一个干净的「默认方法」可填：距离的度量
  随实现而异（RGB = `u32` 平方距离、Lab = `ΔE` 浮点），base trait 写不出一个跨实现正确的默认距离
  （给 Lab matcher 返 RGB 距离是 footgun）。将来若需距离，按 matcher 自有度量扩**新方法签名**
  （对象安全的 `fn find_best_match_with_distance(&self, [u8;3]) -> (u16, MatcherDistance)`，距离类型由
  实现定义）——是**新增**方法（向 trait 加方法默认非破坏，但不是「默认方法」那种零成本接缝）。

**D3 — RGB 平方欧氏距离的确定性细节。**
1. **比较平方距离，无 `sqrt`**：`sqrt` 不改排序、引入 `f32` 破坏整数确定性。纯整数 = 跨架构位精确
   （D8 的基础）。
2. **距离类型 `u32`**：单分量差 `≤255`，平方 `≤65025`，三分量和 `≤195075` ≪ `u32::MAX`；`u16`
   装不下（`65535`）。分量差**先 widening 到 `i32`**（`a as i32 - b as i32`）再平方，防 `u8` 减法
   下溢 panic/wrap。
3. **平局取最低下标（first-wins）**：用严格 `<` 更新最优（`if d < best_d { best_d = d; best_i = i }`，
   相等不更新）→ 天然产出最低下标 = 调色板 JSON 声明序的第一个，与 M1「order matches JSON」+ M2
   「裁剪偏移确定」的「靠位置定序」一致。**这是确定性门（CLAUDE 规则 2），必须写死且测试钉住**。
   - **快照必须保序**：`new()` 把 `palette.colors` 顺序拍平成 `Vec<[u8;3]>`（`.collect()`/顺序 push），
     故快照下标 `i` ≡ `palette.colors[i]` 下标。最低下标平局因此返回的是**调色板声明序下标**，与
     M1「order matches JSON」同一锚点。这是隐含但载重的不变量，明确写下。
   - **精确命中也走平局规则**：validate 只保证 code 唯一、**不保证 RGB 唯一**（D 上下文）。若两个调色板
     色 RGB 相同，对命中像素二者距离都为 0——按严格 `<` 返回**较小下标**。spec「精确命中」场景因此不
     假设唯一命中：精确命中 = 距离 0 的最低下标。
     - **精确命中不是单独的预检分支**：它就是统一距离循环里的 `distance == 0`（0 即最小值），**没有**「先查
       精确命中、再算距离」这种次序，也**不该**为它加特例分支（ponytail——多一条 `if exact` 既是冗余代码、
       又会和距离循环产生两套平局语义）。spec「精确命中（距离 0）」描述的是通用距离算法在 d==0 时的结果，
       不是要求一条独立代码路径。
4. **空 palette 守卫在 `new()`**：M1 validate 已保证非空，但 `Palette` 字段 `pub`、外部可手构空
   palette。`find_best_match` 对空 palette 无合法返回值。决策：在 `RgbMatcher::new` 一次性守卫
   （见 D7），热路径 `find_best_match` 保持全函数 `-> u16`，不在每格背 `Result`（呼应 M2 D5「每
   原语自守卫，但守卫放在唯一入口」）。

**D4 — M3/M4 边界：`BeadPattern` 不含 `stats` 字段。** M3 阶段 `BeadPattern = { width, height,
cells }`。`ColorStat` 与 `stats: Vec<ColorStat>` 全部推到 M4。

- 替代方案：M3 加 `stats: Vec<ColorStat>` 填空 `vec![]`（或 `Option`）。否决：复用 M2 D1「不预造
  将来字段」。空 `stats:[]` 是会撒谎的字段（无法区分「尚未统计」与「真零色」，后者不可能因 w×h≥1）。
  M3 Done-when 不提 stats，不该有此字段。M4 加 `stats` 是**写进路线图的既定演进**（非探索），到时
  一并改 golden——字段越少，M3 golden 越稳。

**D5 — 入口形状与可见性：`pub` 出 trait + `RgbMatcher` + `match_pattern`。**

```rust
pub fn match_pattern(grid: &PixelGrid, matcher: &dyn ColorMatcher) -> BeadPattern;
```

逐格 `grid.pixels[i] → matcher.find_best_match(..) → cells[i]`（同 `i = y*width+x`，零坐标转换），
`grid.width/height` 原样搬进 `BeadPattern`，保持行优先不变量。

- **前置条件与 `cells.len()` 的来源（须显式，否则 spec 的 `cells.len()==width*height` 不变量是空头支票）**：
  `match_pattern` **要求** `grid.pixels.len() == grid.width as usize * grid.height as usize`。该不变量由
  `resize_image` 保证；但 `PixelGrid` 字段 `pub`、外部可手构破坏（`models/mod.rs` 注释已声明「caller is
  responsible for satisfying `pixels.len() == width * height`」）。**决策（最省、与 M2 同口径）**：把
  「调用方拥有 `PixelGrid` 不变量」延续为 `match_pattern` 的**契约前置条件**——遍历 `grid.pixels` 产 `cells`
  （故 `cells.len() == grid.pixels.len()`，前置条件成立时即 `== width*height`），`match_pattern` 保持全函数
  不返 `Result`，**不**在热路径替调用方复检（与 D2/D5 的「match 是全函数原语」一致）。前置条件违约 =
  调用方契约违约，产物 `BeadPattern` 不变量随之不成立——这是 garbage-in，与 M2 `PixelGrid` 的外部手构
  caveat 同性质，文档钉住而非吞掉。`width=0`/`height=0` 的退化网格在前置条件下自然得 `cells.len()==0`，
  合法。（不取「`new` 式 `Result` 校验」是因那会改 `match_pattern` 已定的全函数签名——属 M3 范围外的契约变更。）
- 取 `&dyn ColorMatcher`（非泛型 `<M>`）：M6 pipeline 持 `Box<dyn>` 顺；每格一次 vtable 但内层还要
  扫几百个 palette 色、开销被摊薄。取 `&PixelGrid` + `&dyn ColorMatcher`（**不**直接取 `&Palette`，
  palette 已被 matcher 自持，D2）。
- 替代方案：不要 `match_pattern`、调用方自己循环。否决：「PixelGrid→BeadPattern」是 M3 核心交付
  （ROADMAP「map PixelGrid into a BeadPattern」），封装了「行优先对齐 + width/height 搬运 + 不变量
  保持」这组易错细节，必须是有名字的入口。
- **与「`pipeline::generate_pattern` 是唯一外部入口」的关系（同 M2 D3 口径）**：这三个 `pub`
  （`ColorMatcher`/`RgbMatcher`/`match_pattern`）是**库内 / pipeline 复用原语，非 FFI 入口**。
  `pipeline::generate_pattern`（M6）一旦存在即 CLI/FFI 规范入口，届时 `match_pattern` 降级于其下、
  不承诺作 FFI 对接点。

**D6 — `BeadPattern` 自此成为真理源：语义交接。** M3 起 preview(M5)/stats(M4)/export 一律从
`BeadPattern` 派生，永不从渲染图反推。`PixelGrid` 降为配色前中间体（M2 D1 已预告此交接）。具体：
- M4 `count_colors` **必须**遍历 `BeadPattern.cells` 的下标计数，**不得**碰 `PixelGrid` 原始 RGB
  （原始 RGB 已被量化到调色板，统计的是豆数不是像素色）。
- M5 渲染从 `cells[i]` 查 `palette.colors[idx].rgb` 上色，**不得**用 `PixelGrid` 原始色（否则
  preview 与实际豆色不符）。
- `match_pattern` 是这次交接的**唯一发生点**：进去 `PixelGrid`（原始色），出来 `BeadPattern`（下标）。

**D7 — 错误模型：复用 M1 `InvalidPalette { reason }`，零新增 `BeadError` 变体。** 匹配本身全函数、
不产错误；唯一错误来源是 `RgbMatcher::new` 的两个构造守卫：
- 空 palette → `BeadError::InvalidPalette { reason: "matcher: palette has no colors" }`（复用 M1
  validate 同款语义——空 palette 本就是 `InvalidPalette` 管辖项，见 lib.rs 注释「empty colors…」）。
- `palette.colors.len() > 65536` → `InvalidPalette { reason: "matcher: palette has more than 65536
  colors" }`。**防 `color_index as u16` 静默截断**——`find_best_match` 返回的下标来自 `palette.colors`
  枚举位置（最大下标 = `len-1`），超 `u16` 会 wrap（静默错误，确定性门的大忌），在 `new` 挡掉。
  **边界精确**：合法下标 `0..=65535`（`u16::MAX == 65535`），故 `len == 65536`（下标 `0..=65535`）
  **全部可表示、合法接受**；首个会溢出的是 `len == 65537`（下标 `65536` wrap）。所以守卫是 `> 65536`
  （即 `>= 65537`），reason「more than 65536 colors」字面准确——早先的 `>= 65536` 会误拒一个完全合法的
  最大调色板、且 reason 与 `== 65536` 自相矛盾，故纠正。
- **`u16` 下标是永久契约，非可演进项**：`cells: Vec<u16>` 与 `find_best_match -> u16` 把「≤65535 色」
  这个上界硬编进两个公开签名。不同于 D1(Eq)/D2(c)/D4(stats) 那些「加了非破坏」的可演进缺省，**收窄/
  放宽 `u16` 是破坏性改两处签名**。这是有意冻结（物理豆色量级数百，u16 是正确尺寸）；65536 守卫只是把
  这个永久上界显式化、防静默截断，不是一个「将来再说」的占位。

- 替代方案 (a)：`new` 不返回 `Result`、空/超限 `panic!`。否决：core 不 panic（M2 D5′「不 panic、
  确定」契约），合法可达输入（外部手构空 palette）要返回 `Result`。
- 替代方案 (b)：新增 `EmptyPalette` 变体。否决（弱）：`InvalidPalette { reason }` 已涵盖空 palette；
  新变体是冗余表面扩张。`BeadError` 已 `#[non_exhaustive]`，将来真要按来源细分仍非破坏，此刻不预加。
- 测试只断言变体 + reason 含关键字（「no colors」/「more than」），不断言完整 Display 文案（同 M1/M2）。

**D8 — 确定性强锚点：M3 钉一份跨架构位精确的真 golden（M2 做不到的）。** 因为匹配全程整数、无
`f32`，跨架构逐字节一致是**保证**。双层（同 M2 D8 结构但更强）：
1. **同进程重算比对**（同 M2 `grid_is_deterministic`）：同 `PixelGrid` + 同 `Palette` 跑
   `match_pattern` 两次，断言两个 `BeadPattern` `PartialEq` 相等。基础确定性门。
2. **跨架构位精确 golden（M3 新增、M2 没有）**：固定一个小 `PixelGrid`（含「精确命中调色板色」
   「等距平局」「离色取最近」三类格）+ 固定小 palette，**硬编码**期望 `Vec<u16>` 入测试。纯整数 →
   arm64(dev) 与 x86_64(CI) 保证一致，硬编码不假阴——对比 M2 不敢硬编码 `Lanczos3` 的 `f32`。

- 意义：M7「frozen engine」的 `pattern.json` 里 `cells`（一串 `u16`）是整数、跨架构稳，是 M7 golden
  的天然锚点（不必为 `cells` 担心 `f32` 漂移）；M8「CLI==FFI」的 `cells` 跨架构位精确是**数学保证**，
  若 M8 发现不一致，bug 必在 FFI 序列化层而非匹配算法（同 M2「CLI 即契约」）。
- 替代方案：像 M2 只做「同进程重算」、真 golden 推 M7。否决：M2 推迟是因 `Lanczos3` 的 `f32` 跨架构
  有末位差风险——M3 没有这个风险，推后是浪费「整数运算」给的免费强锚点；M3 钉死让「匹配算法被意外
  改动」在 M3 就响亮失败（early detection）。

## 风险 / 权衡

- [与 ARCHITECTURE 既定 `BeadCell{x,y,color_index}` 分歧（D1）] → 本 change 内同步修订
  `ARCHITECTURE.md` / `ROADMAP.md` / `INIT.md` / `models/mod.rs` doc-comment 四处真理源（见 proposal
  「变更内容」）；依据是 M2 `PixelGrid` 已开行优先无坐标先例并被归档接受。若将来导出/Flutter 渲染硬要
  per-cell 绝对坐标，从行优先现推即可（省内存且与 PixelGrid 一致）。
- [`color_index as u16` 溢出（D7）] → 调色板 `> 65536` 色时下标截断；`RgbMatcher::new` 守卫
  `len <= 65536`（即拒 `> 65536`）挡静默错误。真实调色板量级数百，守卫几乎不触发但必要。
- [matcher 自持 palette 的 RGB 快照（D2/D3）] → `new` 后若改原 `Palette`，matcher 持旧快照；这是
  **有意的值语义**（Palette 是不可变输入，管线内不改），文档注明「matcher 持构造时快照」。
  另一向耦合：matcher 返回的下标指向**构造时**那份 palette；下游 M5 用 `palette.colors[idx].rgb` 上色
  必须是**同一份未被改动的 `Palette`**，否则 index→color 错位。M6 pipeline 应持单一 `Palette` 同时喂
  matcher 与 renderer——M6 落地时作为管线不变量明确。
- [平局规则是确定性门（D3.3）] → 误用 `<=` 更新会变成「最高下标」、跨重构漂移；用严格 `<` + 专门
  的等距平局测试钉死「取最低下标」。golden 之外必须有的单元锚点。
- [`&dyn ColorMatcher` 的每格 vtable（D5）] → 单线程 Phase 1、内层 palette 扫描摊薄开销，可接受；
  若 Phase 2 性能不足，改泛型 `<M>` 单态化是非破坏的局部优化。

## Migration Plan

无运行时迁移：纯新增能力 `color-matching` + 扩 `BeadPattern`，不改 palette / image-grid 已生效规范。
四处文档校正（`ARCHITECTURE.md` / `ROADMAP.md` / `INIT.md` / `models/mod.rs` doc-comment）随本 change
提交。回滚 = 撤销本变更（删 `matcher` 模块、回退 `BeadPattern`、回退四处文档）。M4 给 `BeadPattern`
加 `stats` 字段是既定演进（非本变更）。

## Open Questions

无 —— 探索阶段决策已全部冻结（A 推翻 ARCHITECTURE / B 裸 `Vec<u16>` / C 不 derive `Eq` /
D 复用 `InvalidPalette` / E `find_best_match` 只回 `u16`，均经主 agent 复核拍板）。
