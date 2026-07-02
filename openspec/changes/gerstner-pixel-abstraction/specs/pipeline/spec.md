# pipeline 规范（增量）

## 修改需求

### 需求:GenerateOptions 形状
`GenerateOptions` MUST 含 `width: u32`、`height: u32`、`resize: ResizeOptions`、`render: RenderOptions`、`max_colors: Option<u32>`、`matcher: MatcherKind`、`despeckle: Option<u32>`、`generator: GeneratorKind`，并提供 `Default`（`width/height` 为 `0`、`resize`/`render` 各自的 `Default`、`max_colors` 为 `None`、`matcher` 为 `MatcherKind::Oklab`、`despeckle` 为 `None`、**`generator` 为 `GeneratorKind::Staged`**）。它**不是** `#[non_exhaustive]`（同 `RenderOptions` 取舍）。`width==0`/`height==0` 由 `generate_pattern` **顶层维度守卫（⓪，先于 generator 分支与解码、两模式一致）**确定性返回 `Err(InvalidImage)`、不 panic。`max_colors == Some(0)` 由 `GreedyReducer::new` 确定性返回 `Err`（`InvalidImage`，reason 含 "max_colors"）。`despeckle == Some(0)` 是合法 no-op（见 pattern-cleanup 规范）。`generator` **无非法值**（枚举两变体均合法），默认 `Staged` 保证不指定时行为与本变更前一致。**注**：`opts.matcher` 只作用于 `Staged` 前段配色与**共用后段的减色度量**；`Gerstner` 前段**恒用 Oklab**（见 gerstner-superpixel 规范），故 `--generator gerstner --matcher rgb` = Oklab 超像素 + RGB 度量减色。

#### 场景:默认选项填充各字段而维度需调用方设置
- **当** 构造 `GenerateOptions { width: w, height: h, ..Default::default() }`
- **那么** `resize`/`render` 为各自 `Default`（`cell_size==10`）、`max_colors==None`、`matcher==Oklab`、`despeckle==None`、**`generator==GeneratorKind::Staged`**，而 `width/height` 为调用方所设

### 需求:generate_pattern 是唯一的生成/编排入口并忠实串联各原语
`pipeline::generate_pattern` MUST 是 `bead-core` 面向外部调用方（CLI、未来 FFI）的**唯一完整生成/编排入口**：外部**禁止**绕过它、在管线外**自行拼装**「前段（`image_to_grid → match_pattern` 或 Gerstner 超像素）→ 珠色减色 →（可选）去斑 → count_colors/generate_summary → render_*」来重做生成。**输入解析** `load_palette` 与**输出序列化** `pattern_json` 是**允许的公开 helper**；现有原语（`image_to_grid`/`match_pattern`/`render_*`/`despeckle`/Gerstner 超像素入口 等）是**库/复用原语、非生成入口**。约束的是「不在外部重做编排」。`generate_pattern` 必须接受 `(image_bytes: &[u8], palette: &Palette, opts: &GenerateOptions)`，返回 `Result<GenerateResult, BeadError>`，**只是忠实串联**既有原语，**固定顺序**为：

【**⓪ 目标维度守卫（先于任何 generator 分支与解码）**：`generate_pattern` **先**校验 `opts.width>0 && opts.height>0`——零维 → `InvalidImage`（reason 点名维度），**先于分支、解码、减色器构造**，两种 generator **一致**（镜像 `image_to_grid` 的解码前维度守卫，避免 Staged 解码前拒零维、Gerstner 解码后拒的分叉）】→【**① 前段图像预处理（按 `opts.generator` 分支，维度已非零）**：`Staged` = `image_to_grid(image_bytes, opts.width, opts.height, &opts.resize)`（解码〔坏图 `ImageDecode`〕+ 裁剪 + 缩放）得 `PixelGrid`；`Gerstner` = 解码（坏图 `ImageDecode`）+ `crop_center`（**与 Staged 同一 `crop_center`**）+ **上采样守卫**（`W<w || H<h → InvalidImage`，见 gerstner-superpixel 规范）得裁剪源图】→【**② 减色器 fail-fast 构造**：当 `opts.max_colors == Some(n)` 时构造 `let reducer = GreedyReducer::new(palette, opts.matcher, n)?`——**在 ① 图像预处理之后、③ 配色之前**：图像错误（坏图 / 零维 / Gerstner 上采样）已在 ① 先行，`max_colors==0` 的 `InvalidImage` 在**配色之前**、且**先于两种 generator 的匹配器构造**（消除按模式分叉的 `InvalidPalette` 优先级）；`None` 不构造】→【**③ 配色 / 贴板（按 `opts.generator` 分支）**：`Staged` = 按 `opts.matcher` 选匹配器 `new(palette)`（空/超大板 `InvalidPalette` 在此）以 `&dyn ColorMatcher` 喂 `match_pattern(grid)`；`Gerstner` = `gerstner` 超像素 + Oklab-argmin palette-constrained 分配（内部 `OklabMatcher::new` 的 `InvalidPalette` 在此），均得**全板** `BeadPattern`】→【**④ 共用后段**：**可选珠色减色**（`Some(n)` 时 `reducer.reduce(&pattern)`）→ **可选去斑**（`despeckle==Some(s)` 时 `despeckle(&pattern, s)`）→ `count_colors`/`total_beads`/`generate_summary` → `render_preview`/`render_grid`】。

**错误优先级（两种 generator 一致）**：`InvalidImage`（**零维**，⓪ 守卫、**先于解码**）→ `ImageDecode`（坏图字节）→ `InvalidImage`（Gerstner **上采样**，解码后；Staged 该输入放大成功、有意不对称）→ `InvalidImage`（`max_colors==0`）→ `InvalidPalette`（空 / 超 65536 板）。**顺序关键**：前段（① 预处理、③ 配色）按 `generator` 二选一，**④ 减色 → 去斑 → 统计/渲染的共用后段两分支一致**；统计与渲染 MUST 基于**减色且去斑后**的 `BeadPattern`。`Staged` 全程行为与本变更前**逐字节相同**（`generator` 默认 `Staged`，默认输出不变、默认 golden 不动）。减色 / 去斑 / **Gerstner 超像素** / 匹配 / 渲染算法各居其模块，管线**不内联算法**、只**编排被串联的原语**；**禁止**为 Gerstner 新增 `BeadError` 变体（其失败经既有变体透传）。`bead-core` **禁止**读写文件系统。FFI 边界**不暴露** `generator`/`despeckle`、默认 `Staged`/`None`。

#### 场景:generate_pattern 的结果与单独调用各原语逐一相等（按所选 generator）
- **当** 对 `(image_bytes, palette, opts)` 调用 `generate_pattern`
- **那么** 返回的 `GenerateResult` 各字段分别等于对同输入**按同序**依次调用：① 前段图像预处理（`Staged` 为 `image_to_grid`；`Gerstner` 为 `crop_center` + 上采样守卫）→（`max_colors==Some(n)`：`GreedyReducer::new(...)?`）→ ③ 配色/贴板（`Staged` 为选定 matcher `new(palette)` + `match_pattern`；`Gerstner` 为超像素 + Oklab-argmin 贴板）→（`Some(n)`：`reducer.reduce`）→（`despeckle==Some(s)`：`despeckle`）→ `count_colors`、`generate_summary`、`render_preview`、`render_grid` 各自的结果（pipeline 未引入任何差异），且 `brand == palette.brand`、`pattern.width==opts.width`、`pattern.height==opts.height`、`pattern.cells.len()==opts.width*opts.height`

#### 场景:max_colors=None 时减色阶段恒等跳过
- **当** 以 `opts.max_colors == None`（含 `..Default::default()`）调用 `generate_pattern`
- **那么** 减色阶段被跳过、前段 pattern 原样进（可选去斑与）统计/渲染；对照「同 `opts` 下把减色阶段从后段移除」所得输出**逐字段/逐字节相同**

#### 场景:despeckle=None 时去斑阶段恒等跳过、默认逐字节不变
- **当** 以 `opts.despeckle == None`（含 `..Default::default()`）调用 `generate_pattern`
- **那么** 去斑阶段被跳过、（减色后的）pattern 原样进统计/渲染，`GenerateResult`（含两张 PNG 字节）与**未引入去斑阶段前逐字段/逐字节相同**（默认 golden 不变）

#### 场景:Some(n)/Some(s) 时统计与渲染基于减色且去斑后的 pattern
- **当** 以 `opts.max_colors == Some(n)` 且 / 或 `opts.despeckle == Some(s)` 调用 `generate_pattern`（任一 generator）
- **那么** `stats`/`summary`/`preview_png`/`grid_png` 全部由**减色且去斑后**的 `BeadPattern` 派生；若限色，最终不同珠色数**仍 ≤ n**

#### 场景:generator=Staged 默认逐字节不变
- **当** 以 `opts.generator == GeneratorKind::Staged`（含 `..Default::default()`）调用 `generate_pattern`
- **那么** 走现分段前段 + 共用后段，`GenerateResult`（含两张 PNG 字节）与**未引入 generator 分支前逐字段/逐字节相同**

#### 场景:generator=Gerstner 走超像素前段、共用后段
- **当** 以 `opts.generator == GeneratorKind::Gerstner` 调用 `generate_pattern`
- **那么** 前段由 `gerstner` 超像素 + Oklab-argmin 贴板产**全板** `BeadPattern`，其后减色 / 去斑 / 统计 / 渲染与 `Staged` **共用同一后段**；`max_colors==Some(n)` 时最终不同珠色数 ≤ `n`

#### 场景:错误优先级两种 generator 一致（图像错误先于 max_colors 先于 InvalidPalette）
- **当** 以 `generator` 分别为 `Staged`、`Gerstner`：(a) `width==0`/`height==0` → 均 `InvalidImage`（零维），由 ⓪ 守卫在**解码之前**返回（故「坏图字节 **且** 零维」两模式都先得零维、**不分叉**）；(b) 可解码有效图 + 坏图字节两类分别：可解码零维已由 (a) 覆盖，坏图非零维 → 均 `ImageDecode`；Gerstner 上采样 `W<w`（解码后）→ `InvalidImage`（Staged 同输入放大成功、有意不对称）；(c) 有效图 + **非法 palette** + `max_colors==Some(0)` → 均 `InvalidImage`（reason 含 "max_colors"，源自 `GreedyReducer::new`）而**非** `InvalidPalette`
- **那么** 错误优先级在**两种 generator 下一致**：零维（先于解码）→ 坏图 `ImageDecode` → Gerstner 上采样 → `max_colors` → `InvalidPalette`，**不 panic**、**不**新增变体

## 新增需求

### 需求:生成模式选择（GeneratorKind，默认 Staged）
`generate_pattern` MUST 由 `opts.generator: GeneratorKind { Staged, Gerstner }`（`Default == Staged`）选择**前段生成策略**：`Staged` = 现分段路径（`image_to_grid → match_pattern`）、行为与本变更前**逐字节相同**；`Gerstner` = 超像素 + palette-constrained 分配（见 gerstner-superpixel 规范）。两模式产出**同形状**全板 `BeadPattern`（`cells` 均合法珠板下标）、共用同一后段（减色 / 去斑 / 统计 / 渲染）。`generator` 无非法值。FFI 边界**不暴露** `generator`、默认 `Staged`。

#### 场景:generator 选择前段、默认 Staged
- **当** 分别以 `GeneratorKind::Staged`、`GeneratorKind::Gerstner` 生成
- **那么** 前段分别用现分段路径 / Gerstner 超像素，各产合法同形状全板 `BeadPattern`；未指定时取 `Staged`（默认路径）
