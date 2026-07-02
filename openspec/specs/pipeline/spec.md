# pipeline 规范

## 目的
定义 `bead-core` 面向外部调用方（CLI、未来 FFI）的**唯一生成/编排入口** `generate_pattern`：确定性串联既有原语
（`image_to_grid` → matcher → **可选珠色减色** →（可选去斑）→ 统计 → 渲染），把入参的**同一份 `Palette`** 喂给 matcher / `count_colors`/`generate_summary` /
`render_*`（单一-`Palette` 不变量，从类型上消灭「配错调色板」），产出打包的 `GenerateResult`（`pattern` + `stats` + `summary` +
`brand` + 两张 PNG 字节），使 CLI 与 FFI 消费**同一个**结果对象、不可能静默分歧（「CLI == FFI」的结构前提）。并定义 `pattern.json`
的序列化形状——由 `pattern_json(&GenerateResult) -> String`（纯数据、不可失败）产出。管线**不含算法**、**不新增 `BeadError` 变体**
（透传各阶段既有错误）、**不碰文件系统**（`image_bytes`/`&Palette` 由调用方读入）；确定性以「同平台 + 同依赖版本」为界（默认
链含浮点源——默认 `Triangle` f32 重采样 + 默认 `OklabMatcher` 的 `cbrt`/`powf`，外加**可选珠色减色**在 `Lab`/`Oklab` 路径的 f32 度量（`Rgb` 减色为纯整数、跨架构稳）——均非跨架构 byte 稳，canonical=arm64-Linux 字节 golden + 同机 CLI==FFI 兜底）。
## 需求
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

### 需求:GenerateResult 打包 pattern、stats、summary、brand 与两张 PNG 字节
`GenerateResult` 必须含 `pattern: BeadPattern`、`stats: Vec<ColorStat>`、`summary: String`、`brand: String`（= 入参 `palette.brand` 的克隆，供 `pattern_json`
顶层 `brand`、使序列化入口无需再单收 `palette`）、`preview_png: Vec<u8>`、`grid_png: Vec<u8>`。pipeline
**必须在内部完成两张 PNG 的渲染**并把字节装入结果，使 CLI 与 FFI 都只「写出」、消费同一个结果对象——这是「CLI == FFI」成立的结构性前提（任一前端
**禁止**自行渲染，否则可能用不同 `RenderOptions` 静默分歧）。

#### 场景:结果自带可写出的两张 PNG 字节
- **当** `generate_pattern` 成功返回
- **那么** `preview_png` 与 `grid_png` 均为非空 PNG 字节、可被解码为图像，调用方无需再调用任何渲染函数即可写出 `preview.png`/`grid.png`

### 需求:GenerateOptions 形状
`GenerateOptions` MUST 含 `width: u32`、`height: u32`、`resize: ResizeOptions`、`render: RenderOptions`、`max_colors: Option<u32>`、`matcher: MatcherKind`、`despeckle: Option<u32>`、`generator: GeneratorKind`，并提供 `Default`（`width/height` 为 `0`、`resize`/`render` 各自的 `Default`、`max_colors` 为 `None`、`matcher` 为 `MatcherKind::Oklab`、`despeckle` 为 `None`、**`generator` 为 `GeneratorKind::Staged`**）。它**不是** `#[non_exhaustive]`（同 `RenderOptions` 取舍）。`width==0`/`height==0` 由 `generate_pattern` **顶层维度守卫（⓪，先于 generator 分支与解码、两模式一致）**确定性返回 `Err(InvalidImage)`、不 panic。`max_colors == Some(0)` 由 `GreedyReducer::new` 确定性返回 `Err`（`InvalidImage`，reason 含 "max_colors"）。`despeckle == Some(0)` 是合法 no-op（见 pattern-cleanup 规范）。`generator` **无非法值**（枚举两变体均合法），默认 `Staged` 保证不指定时行为与本变更前一致。**注**：`opts.matcher` 只作用于 `Staged` 前段配色与**共用后段的减色度量**；`Gerstner` 前段**恒用 Oklab**（见 gerstner-superpixel 规范），故 `--generator gerstner --matcher rgb` = Oklab 超像素 + RGB 度量减色。

#### 场景:默认选项填充各字段而维度需调用方设置
- **当** 构造 `GenerateOptions { width: w, height: h, ..Default::default() }`
- **那么** `resize`/`render` 为各自 `Default`（`cell_size==10`）、`max_colors==None`、`matcher==Oklab`、`despeckle==None`、**`generator==GeneratorKind::Staged`**，而 `width/height` 为调用方所设

### 需求:单一 Palette 不变量（matcher、统计、渲染同喂一份调色板）
`generate_pattern` 必须把**入参的同一份 `palette` 引用**喂给由 `opts.matcher` 选定 matcher 的 `new`（`RgbMatcher::new` / `LabMatcher::new` / `OklabMatcher::new`）、
`render_grid`。调用方**无法**提供多份不同调色板——签名只有一个 `&Palette`。因此 matcher 的 `index→color`、统计的 `index→{code,name}`、渲染的
`index→rgb` 必然锚到同一份调色板，「传错/不一致调色板」在管线内**结构性不可能**（这是 M4/M5 推迟到 pipeline 落地的不变量）。

#### 场景:stats 的 code/name 与 render 的 rgb 来自同一调色板
- **当** `generate_pattern` 产出 `GenerateResult`
- **那么** 对任一格下标，`stats` 中该色的 `code`/`name` 与 `preview_png`/`grid_png` 中该格的 `rgb` 来自 `generate_pattern` 收到的同一份 `palette`

### 需求:pattern.json 序列化形状（完整报告、仅写出、不可失败）
pipeline 必须能把结果序列化为一个**完整报告**性质的 `pattern.json`（自带 `brand` + 维度 + `cells` + `total` + `stats`），其**键集**为 `{ brand, width, height, cells, total, stats }`。
**非独立可 render**：`cells` 的整数下标须靠 `brand` 标识的**同一份调色板（同序）**才能映射回 RGB——`stats` 按 count 排序、不带下标，单凭本 JSON 无法解码任意 `cells[i]`→色；M6
**有意不内嵌色表**（YAGNI、只写不读），故是完整*报告*而非独立可重建的工程文件。`brand` 取自
`result.brand`（= 产出该 `result` 的 `palette.brand`）、`total` 等于 `total_beads(&pattern)`、`cells` 是行优先调色板下标的整数数组（真相源）、`stats` 是
`[{ code, name, count }]`。序列化入口**必须是 `pattern_json(result: &GenerateResult) -> String`**：**只取 `&GenerateResult`、不单收 `palette`**（`brand`
已在 `result.brand`）——杜绝「传一份与 `result` 不一致的 palette、写出错 `brand`」这一会重开「单一 Palette」保证的缺口。序列化
**必须复用真相源类型本身**（`BeadPattern`/`ColorStat` derive `Serialize`），**禁止**另立会与真相源漂移的 DTO 镜像；顶层 `brand`/`total` 由一个序列化
专用包装承载（`#[serde(flatten)]` 摊平 `pattern` 的 `width/height/cells`），不重列。**键序由 serde 字段声明序决定、确定性**（JSON 对象键序语义无关；
规范只钉**键集与含义**，逐字节 key 序的 frozen golden 归 M7）。入口**必须返回 `String`（不可失败）、不返回 `Result`**：`PatternFile`（标量 + 借用的
`cells`/`stats`）的序列化无非字符串 map 键、无会失败的 `Serialize` → 不可失败；故**不得**为序列化失败新增 `BeadError` 变体（也**禁止**误用解析变体
`PaletteParse`）。M6 **只写不读**——**禁止**在 M6 实现反序列化/读回。

#### 场景:序列化产出字段一致的完整报告 JSON
- **当** 用 `pattern_json(&result)` 序列化为 `pattern.json`
- **那么** 返回的 `String` 解析后**含键** `brand`、`width`、`height`、`cells`（整数数组）、`total`、`stats`（每项含 `code`/`name`/`count`），且
  `total == cells.len() == width*height`，且各 `stats[i].count` 之和等于 `total`（对 pipeline 产出的结果**恒成立**——`total_beads == cells.len()`，而
  `cells` 由 `match_pattern` 在 `opts.matcher` 选定 matcher 的 `new(palette)` 对**该 palette 的快照**上产出、每个下标**必 `< palette.colors.len()`**，故 `count_colors` 用**同一份**
  palette 计数时无一越界被跳过 → `Σ count == total`。**定理锚在 matcher 的下标值域**（不是泛泛的「单一 Palette」，对任何 `ColorMatcher` 实现的同序快照均成立）；对**外部手搓**的 `BeadPattern`（含越界下标）
  `count_colors` 会跳过越界项、`Σ count < total`，正是 statistics-D4 的「传错调色板」可观测信号）

### 需求:管线错误透传，不新增 BeadError 变体
`generate_pattern` MUST 把内部各阶段的 `BeadError` 经 `?` 透传：**顶层 ⓪ 维度守卫**的 `InvalidImage`（零维度、先于 generator 分支与解码、两 generator 一致；镜像 `image_to_grid` 的维度 reason）、`image_to_grid` 的 `ImageDecode`（坏图字节，仅 Staged 前段；Gerstner 前段解码同样 `ImageDecode`）、**减色器 fail-fast 构造** `GreedyReducer::new(palette, opts.matcher, n)` 的拒绝（仅当 `opts.max_colors==Some(n)`；`max_colors==0` → `InvalidImage { reason }`（含 "max_colors"，复用零维度同变体、**不新增**），非法 palette → `InvalidPalette`；`GreedyReducer::new` **内部先校验 `max_colors>=1`、后校验 palette**，故 `max_colors==0` 的 `InvalidImage` 优先于 palette 校验，与旧版 `MedianCutQuantizer::new`（配色前量化器）的错误优先级一致）、**由 `opts.matcher` 选定的 matcher `new(palette)`** 的 `InvalidPalette`（空 / 超 65536 色，`RgbMatcher`/`LabMatcher`/`OklabMatcher` 复用同一守卫）、`render_*` 的 `InvalidImage`（**不止零维度——还含输出缓冲过大** `out_*>u32::MAX` 或 `bytes>isize::MAX`，见 renderer 规范）/`ImageEncode`。**禁止**为管线新增 `BeadError` 变体（管线无新失败语义；`match_pattern`/`count_colors`/`total_beads`/`generate_summary` 是全函数、不失败；珠色减色 `reduce` 在 `GreedyReducer::new` 校验通过、且输入是管线内 `match_pattern` 产出的**合法 `BeadPattern`**（`cells` 均为 `< palette.colors.len()` 的合法下标）时亦为全函数、不 panic、不失败——见 color-reduction「BeadReducer trait」的前置条件；matcher 选择是枚举分支，三变体均合法、无新增错误语义）。

**错误优先级（fail-fast，顺序固定）**：**顶层 ⓪ 维度守卫**（零维、先于解码与 generator 分支）→ 前段解码（坏图 `ImageDecode`）/ Gerstner 上采样守卫（`InvalidImage`）→ `GreedyReducer::new`（`max_colors==0`，仅当 `Some(0)`）→ 匹配器 `new`（非法 palette）→ `match_pattern` → `reduce` → 统计 / `render_*`。即减色器构造在配色**之前**：`max_colors==0` 的 `Err` 确定性地先于匹配器的 `InvalidPalette`，`match_pattern`/`reduce`/统计/渲染在任一早失败时都不可达。空网格（`width==0`/`height==0`）由 `generate_pattern` **顶层 ⓪ 维度守卫**返回 `Err(InvalidImage)`、**不 panic**、**先于解码 / 配色 / 渲染失败**、两 generator 一致。病态巨大的 `width`/`height` 可能在 `render_*` 的过大缓冲守卫返 `Err(InvalidImage)`（透传、不 panic），或在更早的 resize 处 OOM-abort（同 renderer 规范接受的病态 OOM 边界；现实维度远不及）。

#### 场景:坏图字节与零维度返回确定性 Err 而非 panic
- **当** 用无法解码的字节，或 `width==0`/`height==0` 调用 `generate_pattern`
- **那么** 分别返回 `Err(BeadError::ImageDecode(..))` 与 `Err(BeadError::InvalidImage { .. })`、**不 panic**；且零维度的 `InvalidImage` 的 `reason` **源自 `generate_pattern` 顶层 ⓪ 维度守卫**（镜像 `image_to_grid` 的维度 reason、含 "target width"/"target height"、两 generator 一致），确定性地证明失败发生在解码 / match/render 之前

#### 场景:max_colors==0 返回确定性 Err 而非 panic（先于配色）
- **当** 以 `opts.max_colors == Some(0)`（其余有效）调用 `generate_pattern`
- **那么** 返回 `Err(BeadError::InvalidImage { reason })`（`reason` 含 "max_colors"，**源自 `GreedyReducer::new`**），**不 panic**、**不**新增变体；且**因减色器 fail-fast 构造在配色之前**，该 `Err` 发生在 `match_pattern`/`reduce`/统计/渲染之前

#### 场景:有效图 + 非法 palette + max_colors==0 优先命中 max_colors 而非 palette
- **当** 以有效图片、**非法 palette**（空 `colors` 或 `colors.len()==65537`）且 `opts.max_colors == Some(0)` 调用 `generate_pattern`
- **那么** 返回 `Err(BeadError::InvalidImage { reason })`（`reason` 含 "max_colors"），**而非** `InvalidPalette`——因 `GreedyReducer::new` 内部先校验 `max_colors>=1`；此优先级与旧版（配色前量化器 `MedianCutQuantizer::new`）一致

#### 场景:非法调色板经减色器 / matcher 透传
- **当** 以有效图片和非法调色板（空 `colors` 或 `colors.len() == 65537`）调用 `generate_pattern`，`opts.max_colors` 为 `None`，且 `opts.matcher` 分别为 `Rgb`/`Lab`/`Oklab`
- **那么** 三者均返回 `Err(BeadError::InvalidPalette { reason })`（`reason` 分别含 "no colors" 或 "more than"，源自匹配器 `new`；当 `max_colors==Some(n>=1)` 时同一 `InvalidPalette` 改由更早的 `GreedyReducer::new` 抛出、`reason` 相同），**不 panic**、**不**新增变体，且失败发生在统计/渲染之前

### 需求:管线确定性（可复现，范围 = 同平台 + 同依赖版本）
同一 `(image_bytes, palette, GenerateOptions)` 必须产生**可复现**的 `GenerateResult`：在**同平台 + 同依赖版本**下 `pattern`/`stats`/`summary`/`brand` 逐字段相等、两张 PNG 字节逐字节相等。实现**禁止**引入非确定性来源（随机、`rayon` 并行、`HashMap`/`HashSet` 迭代顺序泄漏）。**跨架构范围（不可过度声称，按 matcher / 减色空间分档）**：默认链的**浮点 / 非跨架构-byte-稳环节**为——① `image_to_grid` 默认 **`Triangle`**（f32 重采样；取代原 `Lanczos3`，仍是 f32、跨架构 byte 不保证）、② 默认匹配器 **`OklabMatcher`**（Oklab + ΔEok²，`f32` 的 `cbrt`；选 `LabMatcher` 时同为 f32，选 `RgbMatcher` 时退化为纯整数）、③ **可选珠色减色 `GreedyReducer::reduce`**（仅当 `max_colors==Some(n)`）：`MatcherKind::Rgb` 路径为**纯整数**（跨架构 byte 稳），`Lab`/`Oklab` 路径复用匹配器 f32 度量 → 跨架构 byte 不保证、**同机确定性**（与 ① ② 同档）。端到端逐字节一致**以「同平台 + 同依赖版本」为界**——因 ① `Triangle` 是 f32，即便选 `RgbMatcher` + `Rgb` 减色，端到端仍 canonical-only；`cells` 及其下游 `stats`/`summary`/`pattern.json`/两张 PNG **全部源自这些步骤**，减色**之后**的链（stats→summary→render→编码）是纯整数、跨架构稳，跨架构不稳的环节是 resize、match、以及 Lab/Oklab 减色本身。**两档不因本变更整体降级**：RGB matcher + RGB 减色的**单元级**度量仍纯整数、可钉跨架构位精确 golden（见 color-matching / color-reduction 规范）；仅**端到端** default 链受 f32 resize/matcher 限为 canonical-only，与引入减色前**同档、未降级**。这对 golden（canonical = arm64 Linux 字节冻结 / 非 canonical 平台只断结构不变量）与「CLI==FFI」（**同机同设备** → 同 libm → 同结果）**已足**；跨架构 byte 一致**非**保证项，由 canonical arm64 golden 承担（golden-tests 规范）。

#### 场景:同输入重复生成结果相等
- **当** 对同一 `(image_bytes, palette, opts)` 多次调用 `generate_pattern`
- **那么**（同平台 + 同依赖版本下）每次的 `pattern`/`stats`/`summary`/`brand` 相等，且 `preview_png`/`grid_png` 逐字节相等

### 需求:生成模式选择（GeneratorKind，默认 Staged）
`generate_pattern` MUST 由 `opts.generator: GeneratorKind { Staged, Gerstner }`（`Default == Staged`）选择**前段生成策略**：`Staged` = 现分段路径（`image_to_grid → match_pattern`）、行为与本变更前**逐字节相同**；`Gerstner` = 超像素 + palette-constrained 分配（见 gerstner-superpixel 规范）。两模式产出**同形状**全板 `BeadPattern`（`cells` 均合法珠板下标）、共用同一后段（减色 / 去斑 / 统计 / 渲染）。`generator` 无非法值。FFI 边界**不暴露** `generator`、默认 `Staged`。

#### 场景:generator 选择前段、默认 Staged
- **当** 分别以 `GeneratorKind::Staged`、`GeneratorKind::Gerstner` 生成
- **那么** 前段分别用现分段路径 / Gerstner 超像素，各产合法同形状全板 `BeadPattern`；未指定时取 `Staged`（默认路径）

