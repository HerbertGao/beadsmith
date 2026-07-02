# pipeline 规范（增量）

## 修改需求

### 需求:GenerateOptions 形状
`GenerateOptions` MUST 含 `width: u32`、`height: u32`、`resize: ResizeOptions`、`render: RenderOptions`、`max_colors: Option<u32>`、`matcher: MatcherKind`、`despeckle: Option<u32>`，并提供 `Default`（`width/height` 为 `0`、`resize`/`render` 各自的 `Default`、`max_colors` 为 `None`、`matcher` 为 `MatcherKind::Oklab`、**`despeckle` 为 `None`**）。它**不是** `#[non_exhaustive]`（同 `RenderOptions` 取舍）。`width==0`/`height==0` 是无效渲染，但**不**由 `GenerateOptions` 自校验——由 `image_to_grid` 的既有零维守卫在 `generate_pattern` 内确定性地返回 `Err`、不 panic。`max_colors == Some(0)` 同样**不**由 `GenerateOptions` 自校验——由 `GreedyReducer::new` 在 `generate_pattern` 内确定性地返回 `Err`（透传既有 `InvalidImage` 变体，reason 含 "max_colors"，见 color-reduction 规范）。`despeckle == Some(0)` 是**合法 no-op**（不存在 0 珠的连通分量、不清理任何东西），**不**报错、**不**新增变体；`despeckle == None` 表示不去斑（默认，逐字节不变）。

#### 场景:默认选项填充 resize/render/max_colors/matcher/despeckle 而维度需调用方设置
- **当** 构造 `GenerateOptions { width: w, height: h, ..Default::default() }`
- **那么** `resize` 等于 `ResizeOptions::default()`、`render` 等于 `RenderOptions::default()`（`cell_size==10`）、`max_colors` 等于 `None`、`matcher` 等于 `MatcherKind::Oklab`、**`despeckle` 等于 `None`（不去斑）**，而 `width/height` 为调用方所设

### 需求:generate_pattern 是唯一的生成/编排入口并忠实串联各原语
`pipeline::generate_pattern` MUST 是 `bead-core` 面向外部调用方（CLI、未来 FFI）的**唯一完整生成/编排入口**：外部**禁止**绕过它、在管线外**自行拼装** `image_to_grid → match_pattern → 珠色减色 →（可选）去斑 → count_colors/generate_summary → render_*` 来重做生成。这**不**意味着 `bead-core` 只暴露这一个公开函数——**输入解析** `load_palette` 与**输出序列化** `pattern_json` 是**允许的公开 helper**；现有原语（`image_to_grid`/`match_pattern`/`render_*`/`despeckle` 等）是**库/复用原语、非生成入口**。约束的是「不在外部重做编排」，而非「只许一个 `pub fn`」。`generate_pattern` 必须接受 `(image_bytes: &[u8], palette: &Palette, opts: &GenerateOptions)`，返回 `Result<GenerateResult, BeadError>`，并**只是忠实串联**既有原语，**固定顺序**为：

`image_to_grid(image_bytes, opts.width, opts.height, &opts.resize)`（缩放）→【**减色器 fail-fast 构造**：当 `opts.max_colors == Some(n)` 时**先于配色**构造 `let reducer = GreedyReducer::new(palette, opts.matcher, n)?`——使 `max_colors==0` 的 `Err(InvalidImage)` **先于匹配器构造发生**（`GreedyReducer::new` 内部**先**校验 `max_colors>=1`、**后**校验 palette，故「有效图 + 非法 palette + `max_colors==0`」仍先命中 `max_colors` 的 `InvalidImage`，见 color-reduction 规范与「管线错误透传」）；`None` 时不构造】→ 按 `opts.matcher` 选定匹配器（`Rgb→RgbMatcher::new`/`Lab→LabMatcher::new`/`Oklab→OklabMatcher::new`，默认 `MatcherKind::Oklab`），以 `&dyn ColorMatcher` 喂 `match_pattern`，得到全调色板的 `BeadPattern` →【**可选珠色减色**（算法 Phase 2）：`Some(n)` 时 `reducer.reduce(&pattern)`（复用上一步已构造的 `reducer`），`None` 时**跳过、pattern 原样**】→【**可选去斑**（despeckle）：`opts.despeckle == Some(s)` 时对（减色后的）`BeadPattern` 调 `despeckle(&pattern, s)`（连通域去斑，见 pattern-cleanup 规范），`None` 时**跳过、pattern 原样**】→ `count_colors`/`total_beads`/`generate_summary` → `render_preview`/`render_grid`。

**顺序关键**：减色发生在配色**之后**、作用于已贴板的 `BeadPattern`；去斑发生在**减色之后**、作用于最终 `BeadPattern`（见 color-reduction / pattern-cleanup 规范）。因此统计与渲染 MUST 基于**减色且去斑后**的 `BeadPattern`。减色与去斑都是**可选**阶段：`max_colors=None`/`despeckle=None` 时对应阶段**恒等跳过**（其上游 pattern 原样）；`despeckle==None` 时管线行为与未引入去斑前**逐字段/逐字节相同**（默认 golden 不变——本变更**不改**默认输出）。去斑只在**已在用的相邻珠色**间重映射，**只减少或持平**不同珠色数（**永不发明中间色**、永不新增珠色），故 `max_colors` 的 ≤N 上限在去斑后**仍成立**。管线层**不内联算法逻辑**——减色（`GreedyReducer`）/ 去斑（`pattern-cleanup`）/ 匹配 / 渲染算法各居其模块，管线只**编排被串联的原语**、**禁止**为去斑新增 `BeadError` 变体（去斑在合法 `BeadPattern` 上是全函数、不失败）。`bead-core` **禁止**读写文件系统：`image_bytes` 与 `&Palette` 必须由调用方读入。FFI 边界**不暴露** `despeckle`、默认 `None`。

#### 场景:generate_pattern 的结果与单独调用各原语逐一相等
- **当** 对 `(image_bytes, palette, opts)` 调用 `generate_pattern`
- **那么** 返回的 `GenerateResult` 的 `pattern`/`stats`/`summary`/`preview_png`/`grid_png` 分别等于：对同输入**按同序**依次调用 `image_to_grid` →（当 `opts.max_colors==Some(n)`：**先**构造 `GreedyReducer::new(palette, opts.matcher, n)?`）→ `match_pattern`（用 `opts.matcher` 选中的 matcher `new(palette)`，默认 `OklabMatcher::new(palette)`）→（当 `opts.max_colors==Some(n)`：**再** `reducer.reduce`）→（当 `opts.despeckle==Some(s)`：**再** `despeckle(&pattern, s)`）、`count_colors`、`generate_summary`、`render_preview`、`render_grid` 各自的结果（pipeline 未引入任何差异），且 `brand == palette.brand`、`pattern.width==opts.width`、`pattern.height==opts.height`、`pattern.cells.len()==opts.width*opts.height`

#### 场景:max_colors=None 时减色阶段恒等跳过
- **当** 以 `opts.max_colors == None`（含 `..Default::default()`）调用 `generate_pattern`
- **那么** 减色阶段被跳过、贴板后的 pattern 原样进（可选去斑与）统计/渲染；对照「同 `opts` 下把减色阶段从管线移除」所得输出**逐字段/逐字节相同**

#### 场景:despeckle=None 时去斑阶段恒等跳过、默认逐字节不变
- **当** 以 `opts.despeckle == None`（含 `..Default::default()`）调用 `generate_pattern`
- **那么** 去斑阶段被跳过、（减色后的）pattern 原样进统计/渲染，`GenerateResult`（含两张 PNG 字节）与**未引入去斑阶段前逐字段/逐字节相同**（默认 golden 不变）

#### 场景:Some(n)/Some(s) 时统计与渲染基于减色且去斑后的 pattern
- **当** 以 `opts.max_colors == Some(n)` 且 / 或 `opts.despeckle == Some(s)` 调用 `generate_pattern`
- **那么** `stats`/`summary`/`preview_png`/`grid_png` 全部由**减色且去斑后**的 `BeadPattern` 派生；若限色，最终不同珠色数**仍 ≤ n**（去斑只减少或持平珠色数、永不新增）
