# pipeline 规范（增量）

## 修改需求

### 需求:generate_pattern 是唯一的生成/编排入口并忠实串联各原语
`pipeline::generate_pattern` 必须是 `bead-core` 面向外部调用方（CLI、未来 FFI）的**唯一完整生成/编排入口**：外部**禁止**绕过它、在管线外**自行拼装**
`image_to_grid → match_pattern → count_colors/generate_summary → render_*` 来重做生成。这**不**意味着 `bead-core` 只暴露这一个公开函数——**输入解析**
`load_palette`（调用方用它读出 `&Palette` 再传入）与**输出序列化** `pattern_json` 是**允许的公开 helper**；现有原语（`image_to_grid`/`match_pattern`/`render_*`
等）是**库/复用原语、非生成入口**。约束的是「不在外部重做编排」，而非「只许一个 `pub fn`」。`generate_pattern` 必须接受
`(image_bytes: &[u8], palette: &Palette, opts: &GenerateOptions)`，返回 `Result<GenerateResult, BeadError>`，并**只是忠实串联**既有原语、**不引入新算法**：
`image_to_grid(image_bytes, opts.width, opts.height, &opts.resize)` → `LabMatcher::new(palette)` → `match_pattern` → `count_colors`/`total_beads`/
`generate_summary` → `render_preview`/`render_grid`。其中 `LabMatcher`（实现 `ColorMatcher` trait，CIELAB + ΔE76）是引擎的**默认且唯一**内部匹配器（算法 Phase 3）——管线**不**暴露匹配器选择项（`GenerateOptions` 无 matcher 字段），切换匹配器属未来变更。`bead-core` **禁止**读写文件系统：`image_bytes` 与 `&Palette` 必须由调用方读入。

#### 场景:generate_pattern 的结果与单独调用各原语逐一相等
- **当** 对 `(image_bytes, palette, opts)` 调用 `generate_pattern`
- **那么** 返回的 `GenerateResult` 的 `pattern`/`stats`/`summary`/`preview_png`/`grid_png` 分别等于：对同输入依次调用 `image_to_grid`+`match_pattern`（用 `LabMatcher::new(palette)`）、
  `count_colors`、`generate_summary`、`render_preview`、`render_grid` 各自的结果（pipeline 未引入任何差异），且 `brand == palette.brand`（`brand` 无对应原语、
  就是入参 palette 的 brand 克隆）、`pattern.width==opts.width`、`pattern.height==opts.height`、`pattern.cells.len()==opts.width*opts.height`

### 需求:单一 Palette 不变量（matcher、统计、渲染同喂一份调色板）
`generate_pattern` 必须把**入参的同一份 `palette` 引用**喂给内部的 `LabMatcher::new`、`count_colors`/`generate_summary` 与 `render_preview`/
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
  `cells` 由 `match_pattern` 在 `LabMatcher::new(palette)` 对**该 palette 的快照**上产出、每个下标**必 `< palette.colors.len()`**，故 `count_colors` 用**同一份**
  palette 计数时无一越界被跳过 → `Σ count == total`。**定理锚在 matcher 的下标值域**（不是泛泛的「单一 Palette」，对任何 `ColorMatcher` 实现的同序快照均成立）；对**外部手搓**的 `BeadPattern`（含越界下标）
  `count_colors` 会跳过越界项、`Σ count < total`，正是 statistics-D4 的「传错调色板」可观测信号）

### 需求:管线错误透传，不新增 BeadError 变体
`generate_pattern` 必须把内部各阶段的 `BeadError` 经 `?` 透传：`image_to_grid` 的 `ImageDecode`（坏图字节）/`InvalidImage`（零维度）、`LabMatcher::new`
的 `InvalidPalette`（空 / 超 65536 色，与 `RgbMatcher::new` 同守卫）、`render_*` 的 `InvalidImage`（**不止零维度——还含输出缓冲过大** `out_*>u32::MAX` 或 `bytes>isize::MAX`，见 renderer 规范）
/`ImageEncode`。**禁止**为管线新增 `BeadError` 变体（管线无新失败语义；`match_pattern`/`count_colors`/`total_beads`/`generate_summary` 是全函数、不失败）。
空网格（`width==0`/`height==0`）必须由 `image_to_grid` 的零维守卫返回 `Err(InvalidImage)`、**不 panic**、且**先于配色/渲染失败**（故 render 与 stats 的空网格
分歧在管线不可达）。病态巨大的 `width`/`height` 可能在 `render_*` 的过大缓冲守卫返 `Err(InvalidImage)`（透传、不 panic），或在更早的 resize 处 OOM-abort
（同 renderer 规范接受的病态 OOM 边界；现实维度远不及）。

#### 场景:坏图字节与零维度返回确定性 Err 而非 panic
- **当** 用无法解码的字节，或 `width==0`/`height==0` 调用 `generate_pattern`
- **那么** 分别返回 `Err(BeadError::ImageDecode(..))` 与 `Err(BeadError::InvalidImage { .. })`、**不 panic**；且零维度的 `InvalidImage` 的 `reason`
  **源自 `image_to_grid` 的目标维度守卫**（如含 "target width"/"target height"），确定性地证明失败发生在 match/render 之前

### 需求:管线确定性（可复现，范围 = 同平台 + 同依赖版本）
同一 `(image_bytes, palette, GenerateOptions)` 必须产生**可复现**的 `GenerateResult`：在**同平台 + 同依赖版本**下 `pattern`/`stats`/`summary`/`brand` 逐字段相等、
两张 PNG 字节逐字节相等。实现**禁止**引入非确定性来源（随机、`rayon` 并行、`HashMap`/`HashSet` 迭代顺序泄漏）。**跨架构范围（不可过度声称）**：默认链含**两个**
浮点 / 非跨架构-byte-稳的环节——① `image_to_grid` 默认 `Lanczos3`（f32 重采样），② 默认匹配器 `LabMatcher`（CIELAB + ΔE76，`f32` 的 `cbrt`/`powf`）。二者跨架构（arm64 / x86_64）
**均不保证 byte 一致**；`cells` 及其下游 `stats`/`summary`/`pattern.json`/两张 PNG **全部源自这两步**，故端到端逐字节一致**以「同平台 + 同依赖版本」为界**——resize 与 match
**之后**的链（stats→summary→render→编码）是纯整数、跨架构稳，跨架构不稳的环节是 resize 与 match 本身。这对 M7 golden（canonical = arm64 Linux 字节冻结 / 非 canonical 平台只断结构不变量）
与 M8「CLI==FFI」（**同机同设备** → 同 libm → 同结果）**已足**；跨架构 byte 一致**非**保证项，由 canonical arm64 golden 承担（golden-tests 规范）。

#### 场景:同输入重复生成结果相等
- **当** 对同一 `(image_bytes, palette, opts)` 多次调用 `generate_pattern`
- **那么**（同平台 + 同依赖版本下）每次的 `pattern`/`stats`/`summary`/`brand` 相等，且 `preview_png`/`grid_png` 逐字节相等
