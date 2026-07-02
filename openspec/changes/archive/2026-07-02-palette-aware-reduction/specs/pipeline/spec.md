# pipeline 规范（增量）

## MODIFIED Requirements

### 需求:generate_pattern 是唯一的生成/编排入口并忠实串联各原语
`pipeline::generate_pattern` MUST 是 `bead-core` 面向外部调用方（CLI、未来 FFI）的**唯一完整生成/编排入口**：外部**禁止**绕过它、在管线外**自行拼装** `image_to_grid → match_pattern → 珠色减色 → count_colors/generate_summary → render_*` 来重做生成。这**不**意味着 `bead-core` 只暴露这一个公开函数——**输入解析** `load_palette` 与**输出序列化** `pattern_json` 是**允许的公开 helper**；现有原语（`image_to_grid`/`match_pattern`/`render_*` 等）是**库/复用原语、非生成入口**。约束的是「不在外部重做编排」，而非「只许一个 `pub fn`」。`generate_pattern` 必须接受 `(image_bytes: &[u8], palette: &Palette, opts: &GenerateOptions)`，返回 `Result<GenerateResult, BeadError>`，并**只是忠实串联**既有原语，**固定顺序**为：

`image_to_grid(image_bytes, opts.width, opts.height, &opts.resize)`（缩放）→【**减色器 fail-fast 构造**：当 `opts.max_colors == Some(n)` 时**先于配色**构造 `let reducer = GreedyReducer::new(palette, opts.matcher, n)?`——使 `max_colors==0` 的 `Err(InvalidImage)` 与旧版（`MedianCutQuantizer::new` 在配色前）一样**先于匹配器构造发生**（`GreedyReducer::new` 内部**先**校验 `max_colors>=1`、**后**校验 palette，故「有效图 + 非法 palette + `max_colors==0`」仍先命中 `max_colors` 的 `InvalidImage`，见 color-reduction 规范与「管线错误透传」）；`None` 时不构造】→ 按 `opts.matcher` 选定匹配器（`Rgb→RgbMatcher::new`/`Lab→LabMatcher::new`/`Oklab→OklabMatcher::new`，默认 `MatcherKind::Oklab`），以 `&dyn ColorMatcher` 喂 `match_pattern`，得到全调色板的 `BeadPattern` →【**可选珠色减色**（算法 Phase 2）：`Some(n)` 时 `reducer.reduce(&pattern)`（复用上一步已构造的 `reducer`），`None` 时**跳过、pattern 原样**】→ `count_colors`/`total_beads`/`generate_summary` → `render_preview`/`render_grid`。

**顺序关键变更**：减色发生在配色**之后**、作用于已贴板的 `BeadPattern`（珠色索引），而非配色前的原始像素网格（见 color-reduction 规范）。因此统计与渲染 MUST 基于**减色后**的 `BeadPattern`。减色是**可选**阶段：`max_colors=None` 时**减色阶段是恒等跳过**——贴板后的 pattern **原样**进统计/渲染。此「不变」的对照基准是「同 `opts` 下把减色阶段从管线移除」，**而非历史默认输出**：本变更同时把 `ResizeOptions::default().filter` 翻为 `Triangle`、默认 matcher 为 `Oklab`（后者随 add-oklab），故默认产物字节相对更早版本**会变**、golden 需重烤——「不变」仅指减色阶段不改动其上游 pattern。`Some(n)` 时把 pattern 的不同珠色数降到 ≤n（只在真实珠色间合并、不发明中间色）。管线层**不内联算法逻辑**——减色（`GreedyReducer`）/ 匹配（`RgbMatcher`/`LabMatcher`/`OklabMatcher`）/ 渲染算法各居其模块，管线只**编排被串联的原语**。`bead-core` **禁止**读写文件系统：`image_bytes` 与 `&Palette` 必须由调用方读入。

#### 场景:generate_pattern 的结果与单独调用各原语逐一相等
- **当** 对 `(image_bytes, palette, opts)` 调用 `generate_pattern`
- **那么** 返回的 `GenerateResult` 的 `pattern`/`stats`/`summary`/`preview_png`/`grid_png` 分别等于：对同输入**按同序**依次调用 `image_to_grid` →（当 `opts.max_colors==Some(n)`：**先**构造 `GreedyReducer::new(palette, opts.matcher, n)?`）→ `match_pattern`（用 `opts.matcher` 选中的 matcher `new(palette)`，默认 `OklabMatcher::new(palette)`）→（当 `opts.max_colors==Some(n)`：**再** `reducer.reduce`）、`count_colors`、`generate_summary`、`render_preview`、`render_grid` 各自的结果（pipeline 未引入任何差异），且 `brand == palette.brand`、`pattern.width==opts.width`、`pattern.height==opts.height`、`pattern.cells.len()==opts.width*opts.height`

#### 场景:max_colors=None 时减色阶段恒等跳过
- **当** 以 `opts.max_colors == None`（含 `..Default::default()`）调用 `generate_pattern`
- **那么** 减色阶段被跳过、贴板后的 pattern 原样进统计/渲染，`GenerateResult`（含两张 PNG 字节）与**在相同 `opts`（同 `resize`/`matcher`）下把减色阶段从管线移除**所得的输出**逐字段/逐字节相同**（减色恒等跳过）
- **且** 该断言**不**要求与历史默认输出字节相同——本变更同时翻默认 `filter=Triangle`、`matcher=Oklab`，默认产物相对旧版本会变（golden 已重烤）；测试须对照「移除减色阶段」的同 `opts` 输出，**不得**据此保留旧 `Lanczos3` 默认

#### 场景:Some(n) 时统计与渲染基于减色后的 pattern
- **当** 以 `opts.max_colors == Some(n)`（`n` 小于贴板结果的不同珠色数）调用 `generate_pattern`
- **那么** `stats` 的不同珠色数 ≤ `n`，且 `stats`/`summary`/`preview_png`/`grid_png` 全部由**减色后**的 `BeadPattern` 派生（不存在减色前后不一致的统计或像素）

### 需求:GenerateOptions 形状
`GenerateOptions` MUST 含 `width: u32`、`height: u32`、`resize: ResizeOptions`、`render: RenderOptions`、`max_colors: Option<u32>`、`matcher: MatcherKind`，并提供 `Default`（`width/height` 为 `0`、`resize`/`render` 各自的 `Default`、`max_colors` 为 `None`、`matcher` 为 `MatcherKind::Oklab`）。它**不是** `#[non_exhaustive]`（同 `RenderOptions` 取舍）。`width==0`/`height==0` 是无效渲染，但**不**由 `GenerateOptions` 自校验——由 `image_to_grid` 的既有零维守卫在 `generate_pattern` 内确定性地返回 `Err`、不 panic。`max_colors == Some(0)` 同样**不**由 `GenerateOptions` 自校验——由 `GreedyReducer::new` 在 `generate_pattern` 内确定性地返回 `Err`（透传既有 `InvalidImage` 变体，reason 含 "max_colors"，见 color-reduction 规范）。

#### 场景:默认选项填充 resize/render/max_colors/matcher 而维度需调用方设置
- **当** 构造 `GenerateOptions { width: w, height: h, ..Default::default() }`
- **那么** `resize` 等于 `ResizeOptions::default()`、`render` 等于 `RenderOptions::default()`（`cell_size==10`）、`max_colors` 等于 `None`（不减色）、`matcher` 等于 `MatcherKind::Oklab`，而 `width/height` 为调用方所设

### 需求:管线错误透传，不新增 BeadError 变体
`generate_pattern` MUST 把内部各阶段的 `BeadError` 经 `?` 透传：`image_to_grid` 的 `ImageDecode`（坏图字节）/`InvalidImage`（零维度）、**减色器 fail-fast 构造** `GreedyReducer::new(palette, opts.matcher, n)` 的拒绝（仅当 `opts.max_colors==Some(n)`；`max_colors==0` → `InvalidImage { reason }`（含 "max_colors"，复用零维度同变体、**不新增**），非法 palette → `InvalidPalette`；`GreedyReducer::new` **内部先校验 `max_colors>=1`、后校验 palette**，故 `max_colors==0` 的 `InvalidImage` 优先于 palette 校验，与旧版 `MedianCutQuantizer::new`（配色前量化器）的错误优先级一致）、**由 `opts.matcher` 选定的 matcher `new(palette)`** 的 `InvalidPalette`（空 / 超 65536 色，`RgbMatcher`/`LabMatcher`/`OklabMatcher` 复用同一守卫）、`render_*` 的 `InvalidImage`（**不止零维度——还含输出缓冲过大** `out_*>u32::MAX` 或 `bytes>isize::MAX`，见 renderer 规范）/`ImageEncode`。**禁止**为管线新增 `BeadError` 变体（管线无新失败语义；`match_pattern`/`count_colors`/`total_beads`/`generate_summary` 是全函数、不失败；珠色减色 `reduce` 在 `GreedyReducer::new` 校验通过、且输入是管线内 `match_pattern` 产出的**合法 `BeadPattern`**（`cells` 均为 `< palette.colors.len()` 的合法下标）时亦为全函数、不 panic、不失败——见 color-reduction「BeadReducer trait」的前置条件；matcher 选择是枚举分支，三变体均合法、无新增错误语义）。

**错误优先级（fail-fast，顺序固定）**：`image_to_grid`（坏图 / 零维）→ `GreedyReducer::new`（`max_colors==0`，仅当 `Some(0)`）→ 匹配器 `new`（非法 palette）→ `match_pattern` → `reduce` → 统计 / `render_*`。即减色器构造在配色**之前**：`max_colors==0` 的 `Err` 确定性地先于匹配器的 `InvalidPalette`，`match_pattern`/`reduce`/统计/渲染在任一早失败时都不可达。空网格（`width==0`/`height==0`）由 `image_to_grid` 的零维守卫返回 `Err(InvalidImage)`、**不 panic**、且**先于配色/渲染失败**。病态巨大的 `width`/`height` 可能在 `render_*` 的过大缓冲守卫返 `Err(InvalidImage)`（透传、不 panic），或在更早的 resize 处 OOM-abort（同 renderer 规范接受的病态 OOM 边界；现实维度远不及）。

#### 场景:坏图字节与零维度返回确定性 Err 而非 panic
- **当** 用无法解码的字节，或 `width==0`/`height==0` 调用 `generate_pattern`
- **那么** 分别返回 `Err(BeadError::ImageDecode(..))` 与 `Err(BeadError::InvalidImage { .. })`、**不 panic**；且零维度的 `InvalidImage` 的 `reason` **源自 `image_to_grid` 的目标维度守卫**（如含 "target width"/"target height"），确定性地证明失败发生在 match/render 之前

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
