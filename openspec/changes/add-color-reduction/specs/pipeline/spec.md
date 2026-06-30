# pipeline 规范（增量）

## MODIFIED Requirements

### Requirement: generate_pattern 是唯一的生成/编排入口并忠实串联各原语
`pipeline::generate_pattern` MUST 是 `bead-core` 面向外部调用方（CLI、未来 FFI）的**唯一完整生成/编排入口**：外部**禁止**绕过它、在管线外**自行拼装**
`image_to_grid → match_pattern → count_colors/generate_summary → render_*` 来重做生成。这**不**意味着 `bead-core` 只暴露这一个公开函数——**输入解析**
`load_palette`（调用方用它读出 `&Palette` 再传入）与**输出序列化** `pattern_json` 是**允许的公开 helper**；现有原语（`image_to_grid`/`match_pattern`/`render_*`
等）是**库/复用原语、非生成入口**。约束的是「不在外部重做编排」，而非「只许一个 `pub fn`」。`generate_pattern` 必须接受
`(image_bytes: &[u8], palette: &Palette, opts: &GenerateOptions)`，返回 `Result<GenerateResult, BeadError>`，并**只是忠实串联**既有原语：
`image_to_grid(image_bytes, opts.width, opts.height, &opts.resize)` →【**可选降色**（算法 Phase 2）：当 `opts.max_colors == Some(n)` 时 `MedianCutQuantizer::new(n)?.quantize(&grid)`，`None` 时**跳过、grid 原样**】→ `LabMatcher::new(palette)` → `match_pattern` → `count_colors`/`total_beads`/
`generate_summary` → `render_preview`/`render_grid`。其中 `LabMatcher`（实现 `ColorMatcher` trait，CIELAB + ΔE76）是引擎的**默认且唯一**内部匹配器（算法 Phase 3）——管线**不**暴露匹配器选择项（`GenerateOptions` 无 matcher 字段），切换匹配器属未来变更。降色是**可选**阶段：`max_colors=None` 时管线行为与未引入降色前**逐字节相同**（恒等跳过）；`Some(n)` 时量化在配色**之前**、把网格降到 ≤n 个代表色（见 color-reduction 规范）。管线层**不内联算法逻辑**——量化（`MedianCutQuantizer`）/ 匹配（`LabMatcher`）/ 渲染算法各居其模块，管线只**编排被串联的原语**（这正是原「不引入新算法」约束的当前形态：新增的是一个被调用的量化器原语，不是管线层算法）。`bead-core` **禁止**读写文件系统：`image_bytes` 与 `&Palette` 必须由调用方读入。

#### Scenario: generate_pattern 的结果与单独调用各原语逐一相等
- **当** 对 `(image_bytes, palette, opts)` 调用 `generate_pattern`
- **那么** 返回的 `GenerateResult` 的 `pattern`/`stats`/`summary`/`preview_png`/`grid_png` 分别等于：对同输入**按同序**依次调用 `image_to_grid`（→ 当 `opts.max_colors==Some(n)` 再 `MedianCutQuantizer::new(n)?.quantize`）+ `match_pattern`（用 `LabMatcher::new(palette)`）、
  `count_colors`、`generate_summary`、`render_preview`、`render_grid` 各自的结果（pipeline 未引入任何差异），且 `brand == palette.brand`（`brand` 无对应原语、
  就是入参 palette 的 brand 克隆）、`pattern.width==opts.width`、`pattern.height==opts.height`、`pattern.cells.len()==opts.width*opts.height`

#### Scenario: max_colors=None 时默认路径逐字节不变
- **当** 以 `opts.max_colors == None`（含 `..Default::default()`）调用 `generate_pattern`
- **那么** 量化阶段被跳过、grid 原样进 `match_pattern`，`GenerateResult`（含两张 PNG 字节）与**未引入降色阶段前逐字段/逐字节相同**

### Requirement: GenerateOptions 形状
`GenerateOptions` MUST 含 `width: u32`、`height: u32`、`resize: ResizeOptions`、`render: RenderOptions`、`max_colors: Option<u32>`，并提供 `Default`（`width/height` 为 `0`、
`resize`/`render` 各自的 `Default`、`max_colors` 为 `None`）。它**不是** `#[non_exhaustive]`（同 `RenderOptions` 取舍）。`width==0`/`height==0` 是无效渲染，但**不**由
`GenerateOptions` 自校验——由 `image_to_grid` 的既有零维守卫在 `generate_pattern` 内确定性地返回 `Err`、不 panic。`max_colors == Some(0)` 同样**不**由 `GenerateOptions` 自校验——由 `MedianCutQuantizer::new` 在 `generate_pattern` 内确定性地返回 `Err`（透传既有变体，见 color-reduction 规范）。

#### Scenario: 默认选项填充 resize/render/max_colors 而维度需调用方设置
- **当** 构造 `GenerateOptions { width: w, height: h, ..Default::default() }`
- **那么** `resize` 等于 `ResizeOptions::default()`、`render` 等于 `RenderOptions::default()`（`cell_size==10`）、`max_colors` 等于 `None`（不降色），而 `width/height` 为调用方所设

### Requirement: 管线错误透传，不新增 BeadError 变体
`generate_pattern` MUST 把内部各阶段的 `BeadError` 经 `?` 透传：`image_to_grid` 的 `ImageDecode`（坏图字节）/`InvalidImage`（零维度）、`MedianCutQuantizer::new` 的 `max_colors==0` 拒绝（仅当 `opts.max_colors==Some(0)`；透传 `InvalidImage { reason }`——复用零维度同变体、不新增，见 color-reduction 规范）、`LabMatcher::new`
的 `InvalidPalette`（空 / 超 65536 色，与 `RgbMatcher::new` 同守卫）、`render_*` 的 `InvalidImage`（**不止零维度——还含输出缓冲过大** `out_*>u32::MAX` 或 `bytes>isize::MAX`，见 renderer 规范）
/`ImageEncode`。**禁止**为管线新增 `BeadError` 变体（管线无新失败语义；`match_pattern`/`count_colors`/`total_beads`/`generate_summary` 是全函数、不失败；降色 `quantize` 在 `new` 校验通过后亦为全函数、不失败）。
空网格（`width==0`/`height==0`）必须由 `image_to_grid` 的零维守卫返回 `Err(InvalidImage)`、**不 panic**、且**先于配色/渲染失败**（故 render 与 stats 的空网格
分歧在管线不可达）。病态巨大的 `width`/`height` 可能在 `render_*` 的过大缓冲守卫返 `Err(InvalidImage)`（透传、不 panic），或在更早的 resize 处 OOM-abort
（同 renderer 规范接受的病态 OOM 边界；现实维度远不及）。

#### Scenario: 坏图字节与零维度返回确定性 Err 而非 panic
- **当** 用无法解码的字节，或 `width==0`/`height==0` 调用 `generate_pattern`
- **那么** 分别返回 `Err(BeadError::ImageDecode(..))` 与 `Err(BeadError::InvalidImage { .. })`、**不 panic**；且零维度的 `InvalidImage` 的 `reason`
  **源自 `image_to_grid` 的目标维度守卫**（如含 "target width"/"target height"），确定性地证明失败发生在 match/render 之前

#### Scenario: max_colors==0 返回确定性 Err 而非 panic
- **当** 以 `opts.max_colors == Some(0)`（其余有效）调用 `generate_pattern`
- **那么** 返回 `Err(BeadError::InvalidImage { reason })`（`reason` 含 "max_colors"，源自 `MedianCutQuantizer::new`），**不 panic**、**不**新增变体；且失败发生在配色/渲染之前
