## 上下文

里程碑 M6。引擎各原语已就位且公开（`lib.rs` 重导出）：`image_to_grid(bytes,w,h,&ResizeOptions)->Result<PixelGrid>`（已含 decode+center-crop+resize）、
`RgbMatcher::new(&Palette)->Result<RgbMatcher>` + `match_pattern(&PixelGrid,&dyn ColorMatcher)->BeadPattern`、`count_colors/total_beads/generate_summary`、
`render_preview/render_grid(&BeadPattern,&Palette,&RenderOptions)->Result<Vec<u8>>`、`load_palette/validate_palette`。M4-D1 已把 `GenerateResult { pattern, stats,
summary }` 定为「M6 打包类型」、并把 grid+stats 的打包推迟到此层；M4-D4/M5-D6 把「管线持单一 `Palette`、同喂 matcher/stats/render」记为 M6 落地的不变量；
M5-D7 查证 `image_to_grid` 经 `resize_image` 先拒 0 维度 → 管线不产空 pattern。

M6 交付 `pipeline::generate_pattern`（唯一**生成/编排**入口，规则 4；`load_palette`/`pattern_json` 等 helper 仍公开）+ `bead-cli generate` 薄壳。约束不变：`bead-core` 无 fs/UI/平台（规则 1，IO 全在 CLI）、
确定性是门（规则 2）、`BeadPattern` 是真相源（规则 3）、CLI 是契约（规则 5）、`thiserror`(core)/`anyhow`(cli)。设计经探索 + Software Architect 拍板 + 主 agent QA。

## 目标 / 非目标

**目标：** `pipeline` 模块（`generate_pattern` + `GenerateResult` + `GenerateOptions`）；`pattern.json` 报告写出形状；`models` 加 `Serialize`；`bead-cli`
`generate`（真实，写四文件）+ `palette validate`（真实）+ `palette list`/`inspect`（桩）；单一-Palette 不变量落地；`samples/` demo 输入；端到端 INIT 示例可跑。

**非目标：** 读回/`load_pattern`（③，留 M9+）；`--cell-size`/`--filter` flag；`palette list`/`inspect` 真实实现；quantizer（Phase 2）；rayon（Phase 2）；CSV；
`tests/golden/*` 固化（M7）；`grid.png` 是否进 golden（M7 决）。

## 决策

**D1 — `pipeline::generate_pattern` 唯一入口；签名取 `(&[u8], &Palette, &GenerateOptions)`，内部固定串联、持单一 `Palette` 喂三方。**

```rust
pub fn generate_pattern(
    image_bytes: &[u8],
    palette: &Palette,
    opts: &GenerateOptions,
) -> Result<GenerateResult, BeadError>;
```
内部顺序（全用既有原语，无新算法）：
```text
image_to_grid(image_bytes, opts.width, opts.height, &opts.resize)?   // decode+crop+resize → PixelGrid
RgbMatcher::new(palette)?                                            // 同一份 palette ↓
match_pattern(&grid, &matcher)                                       // → BeadPattern（真相源）
count_colors(&pattern, palette) / total_beads(&pattern) / generate_summary(&pattern, palette)
render_preview(&pattern, palette, &opts.render)? / render_grid(&pattern, palette, &opts.render)?
→ GenerateResult { … }
```
- **理由**：① 规则 4——`generate_pattern` 是唯一的**完整生成/编排入口**：CLI/FFI 不得绕过它去**自行拼装 image→match→stats→render 重做生成**。**注（review M3/Codex）**：这**不**意味着 `bead-core` 只有这一个公开函数——**输入解析** `load_palette`（CLI 用来读 `&Palette` 喂进来）与**输出序列化** `pattern_json` 是**允许的公开 helper**；现有原语（`image_to_grid`/`match_pattern`/`render_*` 等）是**库/复用原语、非生成入口**（同 M3/M4/M5 规范的措辞）。规则 4 约束的是「不在外部重做编排」，而非「只许一个 pub fn」。② **单一 `&Palette` 参数从类型上消灭 M4-D4 隐患**：matcher 的 `index→color`、stats 的 `index→{code,name}`、render 的 `index→rgb` 全锚到这**同一份** `palette`，调用方**无法**传三份不同的——M4/M5 反复记的「管线持单一 Palette」不变量在此**结构性**实现，不再是文档前向约束。③ 取 `&[u8]`(图字节) + `&Palette` 而非路径：规则 1，core 不读文件；与 `image_to_grid(bytes,…)` 一致。④ 串联是 5 个既有原语，pipeline **不含算法**（ARCHITECTURE「CLI/pipeline 无算法」）。
- **替代方案：`generate_pattern(path, palette_path, …)` 读文件。** 否决：违规则 1（core 碰 fs）。读文件是 CLI 的活。
- **替代方案：分别暴露 `generate_pattern`(只算 pattern) 与单独 render。** 否决：见 D2——会让 CLI/FFI 各自渲染、违规则 5。

**D2 — `GenerateResult` 打包 pattern + stats + summary + **两张 PNG 字节**；pipeline 内部渲染，CLI/FFI 纯写出。**

```rust
pub struct GenerateResult {
    pub pattern: BeadPattern,
    pub stats: Vec<ColorStat>,
    pub summary: String,
    pub brand: String,            // = palette.brand（克隆一次），供 pattern_json 顶层 brand
    pub preview_png: Vec<u8>,
    pub grid_png: Vec<u8>,
}
```
- **理由**：① 规则 5 + M8 done-when（「Dart 拿到与 CLI **相同**结果」）**只有**当两个前端消费**同一个**结果对象才结构性成立。若各自渲染，M8 Flutter 可能传与 CLI 不同的 `RenderOptions` 而静默分歧——正是规则 5 要防的 bug。把 PNG 字节装进结果 → 「CLI==FFI」成为**数据的属性**、不依赖前端自律。② 直接拓宽 M4-D1 既定的 `GenerateResult{pattern,stats,summary}`——M6 只加 M5 渲染器已产出的两个 `Vec<u8>`。③ `summary` 入结果（非只 `stats`）：CLI 直接写 `summary.txt`，且 summary 与 stats 同源自 pipeline 一次计算、不分歧。④ **`brand` 入结果（review M6-R3/Codex）**：`pattern.json` 顶层唯一需要从 `palette` 取的就是 `brand`；把它克隆进结果后，`pattern_json` 只取 `&GenerateResult`、**不再单收 `palette`** → 杜绝「传一份与产出 `result` 不一致的 palette、写出错 brand」这个本会重开 D1/§4「传错调色板结构性不可能」保证的缺口（同 ① 的「数据属性、非调用方自律」哲学；代价仅一次 `String` 克隆，相对两块 PNG 可忽略）。
- **代价（自觉接受）**：调用方即便只想要 stats 也付两次渲染开销。M6 的活就是「全量 generate」，可接受；真有「只要 stats」的消费者是未来另开的轻量入口（非破坏），非 M6。
- **替代方案：`GenerateResult{pattern,stats,summary}`、CLI 自行渲染。** 否决：见①，CLI/FFI 两条渲染路径会漂、危及规则 5。

**D3 — `GenerateOptions` 结构体（`width,height,resize,render`），derive `Default`、非 `#[non_exhaustive]`。**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateOptions {
    pub width: u32,
    pub height: u32,
    pub resize: ResizeOptions,
    pub render: RenderOptions,
}
impl Default for GenerateOptions { /* width:0, height:0, resize/render 各自 Default */ }
```
- **理由**：① 单一 options 结构体让签名随未来旋钮（Phase-2 quantizer 选项等）增长而**稳定**、不必重穿每个调用方——同 `ResizeOptions`/`RenderOptions` 立结构体的理由。② 非 `#[non_exhaustive]`：同 `RenderOptions`(M5-D2) 的自觉取舍——保字面构造便利、加字段技术上破坏但靠 `..Default::default()` 缓解、且 M6 阶段无外部字面构造者。
- **`Default` 的瑕疵（文档化）**：`width/height` 默认 `0` 是**无效渲染**（`image_to_grid` 会拒 0 维度、干净返 `Err`）。`Default` 仍有用：调用方设维度、用 `..Default::default()` 填 `resize`/`render`。即「`Default` 不是能直接跑的配置、是填充便利」。
- **替代方案：松散参数 `generate_pattern(bytes, palette, w, h, resize, render)`。** 否决：6 参数、加旋钮即破坏签名、重穿所有调用方。
- **替代方案：`width`/`height` 用 `NonZeroU32`。** 否决：与 `image_to_grid(u32,u32)` 不一致、`Default` 要 `unwrap` 噪声；维度非法由 `image_to_grid` 既有 0-守卫处理（同 M5-D2 对 `cell_size` 的取舍）。

**D4 — `pattern.json` 形状 = 完整报告（非独立可 render）、键集 `{ brand, width, height, cells, total, stats }`；仅写出；用 `#[serde(flatten)]` 的局部 `PatternFile`；序列化由 `pattern_json -> String`（**不可失败**）产出。**

```rust
#[derive(serde::Serialize)]
struct PatternFile<'a> {
    brand: &'a str,                              // = palette.brand
    #[serde(flatten)] pattern: &'a BeadPattern,  // 摊平出 width / height / cells（顺序在 total 之前）
    total: u32,                                  // = total_beads(&pattern)
    stats: &'a [ColorStat],
}
// 序列化不可失败 → 返 String（不返 Result），同渲染器「不可达 assert」姿态（review M1/Codex/RC）：
// 只取 &GenerateResult（brand 已在结果里）——不单收 palette，杜绝配错（review M6-R3/Codex）：
// pub fn pattern_json(result: &GenerateResult) -> String {
//     serde_json::to_string(&PatternFile { brand: &result.brand, pattern: &result.pattern,
//         total: total_beads(&result.pattern), stats: &result.stats })
//         .expect("PatternFile 是纯数据（无非字符串 map 键、无自定义 Serialize），序列化不可失败")
// }
```
- **键序由 `serde` 字段声明序决定 → 确定性**（`{ brand, width, height, cells, total, stats }`——`flatten` 把 `pattern` 的 `width/height/cells` 注入在 `brand` 之后、`total` 之前；**`total` 紧邻 `stats`**）。**规范层只钉「含哪些键 + 其含义」（JSON 对象键序语义无关）；逐字节 key 序是确定的 serde 字段序、**同 serde/serde_json 版本内**稳定（跨版本 key 序同 PNG 编码字节——归 M7 frozen golden + 「dep bump 响亮失败」承接，review M6-R4/CR/RC），由 M7 frozen golden 锁**（review M4/Codex：早稿广告序 `{…,total,cells,…}` 与 `flatten` 实出序不符，会让 M7 byte-golden 对不上——现把广告序对齐实出序）。
- **`pattern_json -> String`（不可失败，review M1/Codex/RC）**：`PatternFile` = 3 标量（`&str`/`u32`）+ `&BeadPattern{u32,u32,Vec<u16>}` + `&[ColorStat{String,String,u32}]`，无非字符串 map 键、无会失败的自定义 `Serialize` → `serde_json::to_string` **不可失败**。故返 `String`、**不返 `Result`**——`BeadError`（`#[non_exhaustive]`）**无序列化错误变体**，唯一带 `serde_json::Error` 的 `PaletteParse` 语义是「解析 palette」、误用即说谎（D7「不新增变体」与「返 Result」自相矛盾，靠返 `String` 化解）。`serde_json::to_string(...).expect(...)` 的 panic 路径不可达（纯数据），同 M5 渲染器 D8 的「不可达 assert」。
- **理由**：① 规则 3——`BeadPattern{width,height,cells}` 是真相源，故 `cells` 必在且权威。② `total`/`stats` 是 pipeline 手头已有的派生物，入文件使其成为完整报告（内容对齐 INIT「Summary Format」）。`total` 虽 `= cells.len() = Σcount`（冗余）但便宜、对报告读者友好——保留。③ `brand` 是把 `cells` 这堆整数下标**重对回**调色板的最小指针（同 M4-D6 从 `&Palette` 读 brand 写 summary）；**不**内嵌完整色表——那是调色板文件的活（YAGNI，brand 串足够重新配对）。④ `#[serde(flatten)]` 让 JSON 顶层既有 `brand/total`、又摊出 `pattern` 的 `width/height/cells`，**不重列**字段、不立 `BeadPattern` 的 DTO 镜像（见 D5）。
- **替代方案：只 `{width,height,cells}`。** 否决：丢了 `stats`/`brand`，文件不自洽、整数下标无从解读。
- **替代方案：内嵌完整 palette colors。** 否决：与调色板文件重复、`pattern.json` 膨胀；YAGNI，brand 足够。

**D5 — 序列化真相源本身：给 `BeadPattern`/`ColorStat` 加 `#[derive(Serialize)]`（仅 Serialize、不 Deserialize）；不立 DTO。**

- `models/mod.rs`：`BeadPattern`、`ColorStat` 各加 `serde::Serialize`（保留 `Debug+Clone+PartialEq`、**仍不 `Eq`**）。**不**加 `Deserialize`（M6 只写不读，D 见非目标③）。
- **理由**：① 镜像 `BeadPattern`/`ColorStat` 字段的 DTO 是**会漂移的重复真相**——正是 M4-D1 整段论证要避免的脱节隐患（DTO 与真相源各存一份、改一处忘改另一处即说谎）。`BeadPattern`/`ColorStat` 已是纯数据、序列化它们**就是**序列化真相源（规则 3）。② `serde` 已是 `bead-core` 依赖（palette 用 `Deserialize`），加 `Serialize` 非破坏、无新依赖。③ 顶层 `brand`/`total` 不属于这两个模型，故由 D4 的 pipeline 局部 `PatternFile` 承载（`flatten` 不重列）。
- **不加 `Deserialize`（自觉）**：读回是非目标③；只 `Serialize` 让公开面最小、且未来加 `Deserialize`+`load_pattern` 是纯增量非破坏。
- **跨规范影响（delta）——只有 statistics，不含 color-matching（已查证）**：`statistics` 的「ColorStat 输出形状」需求**明文钉了 derive 集**（`Debug+Clone+PartialEq`、不 `Eq`），加 `Serialize` 是追加一个 derive，须经 `## 修改需求` delta 显式说明（规范性「`code/name/count:u32`」「不 `Eq`」不变、被保持）。而 `color-matching` 的「BeadPattern 输出形状」需求**未**钉 derive 集（grep 确认无 `derive`/`Eq`/`Serialize`/`PartialEq` 字样）→ 给 `BeadPattern` 加 `Serialize` **不触动任何已生效需求**，**无 color-matching delta**；仅改 `models/mod.rs` 的 `BeadPattern` doc-comment（tasks，非规范变更）。见 tasks。

**D6 — CLI：`generate` 真实（仅 5 个 INIT flag）+ `palette validate` 真实 + `palette list`/`inspect` 桩；建目录、覆盖写、anyhow 语境。**

- `bead-cli`（`clap` derive 子命令；**所有 fs/IO + `anyhow`**）：
  - **`generate --input --palette --width --height --output`（真实）**：`anyhow` 逐步——`fs::read(--input)` 取**字节**、`load_palette(&fs::read(--palette))`（`load_palette` 收 `&[u8]`、且**已含完整校验**——非空/hex/唯一 code，见 D6 下「validate」）、构 `GenerateOptions{w,h, ..Default::default()}`、`generate_pattern(...)`、`create_dir_all(--output)`、**覆盖写** `preview.png`(`preview_png`)/`grid.png`(`grid_png`)/`pattern.json`(`pipeline::pattern_json(&result)` 的 `String`，CLI 不依赖 `serde_json`)/`summary.txt`(`summary`)。**仅暴露这 5 个 flag**；`cell_size`/`filter` 用默认（10/Lanczos3）、**不暴露** → 默认 `cell_size=10>=5`，故 grid 的 `cell_size<5→Err` 在 M6 **永不触发**、CLI 无须为它专门处理。
  - **`palette validate <path>`（真实）**：`load_palette(&fs::read(path))` 即足——`load_palette` 内部已调 `validate_palette`（非空 + hex + 唯一 code，已查证 `palette/mod.rs`）。ok 打印成功 / 失败把 `BeadError` 的确定性 `reason` 透出到 stderr、退出码语义化。**不必再单独调 `validate_palette`**（冗余 no-op，review m2/CR/RC/Codex）。
  - **`palette list` / `inspect <path>`（桩）**：`anyhow::bail!("... coming soon")`、exit 1（**非静默成功**）。`list` 需尚不存在的「调色板目录约定」；`inspect` 需读回路径（非目标③）。
- **覆盖写 + 建目录**：`--output` 不存在则 `create_dir_all`；已存在文件**直接覆盖**——确定性（规则 2）要求重跑复现同样的四个文件，**覆盖才是对的**（不是报错）；这也是 M7 golden 重跑的前提。
- **文件系统失败 + 非原子写（review M2/CR/RC）**：`create_dir_all` 与四个 `File::create`/写出的**任何** `io::Error` 必须经 `anyhow` 语境化为**非零退出 + stderr 点名涉事路径与 OS 错误、不 panic**（catch-all，沿用 C3 的逐步 `.context`）；代表性非穷举用例——`--output` 已存在为**普通文件**（非目录，`create_dir_all` 报错）/ `--output` 父目录**不可写** / `--input` 是**目录或不可读** / 任一输出目标路径**本身是目录或不可写** / 写盘失败（磁盘满）。**四文件按序写、非事务/非原子**（无 temp+rename）：写到一半被打断（磁盘满 / SIGKILL）会留**半写的输出集**（如截断的 `pattern.json`），这是**有意接受的边界**——重跑覆盖即恢复；**事务写（temp+原子 rename）是 non-goal**（YAGNI、越出 M6 范围；要原子另开 change）。把此姿态写进 cli 规范，使其是**决策**而非沉默。
- **理由**：ROADMAP M6 done-when = 「INIT 示例端到端跑通、写出四文件」——这 5 个 flag 恰是该面、不多（YAGNI）。推迟 `--cell-size`/`--filter` 既让 M7 golden 输入固定（默认）、又彻底绕开 `cell_size>=5` 报错。便宜的 `validate` 接真、需约定/读路径的 `list`/`inspect` 桩——同全局「便宜的接真、其余桩成显式报错」。
- **替代方案：现在就暴露 `--cell-size`/`--filter`。** 否决：YAGNI + 引入 `cell_size<5` 报错处理 + 动摇 M7 golden 输入；后续非破坏加。
- **替代方案：`list`/`inspect` 静默成功或 `unimplemented!`。** 否决：静默成功是假绿；`unimplemented!` 会 panic（CLI 不该 panic）。用 `anyhow::bail!` 给确定性退出 1。

**D7 — 错误模型：管线复用各阶段既有 `BeadError`、**不新增变体**；CLI 用 `anyhow` 加语境。**

- `generate_pattern` 返回 `Result<GenerateResult, BeadError>`——内部 `?` 透传各阶段既有 `BeadError`：`image_to_grid`(`ImageDecode` 坏图字节 / `InvalidImage` 零维度)、`RgbMatcher::new`(`InvalidPalette` 空/超 65536)、`render_*`(`InvalidImage` **不止零维度——还含输出缓冲过大** `out_*>u32::MAX`/`bytes>isize::MAX`，M5；以及 `ImageEncode`)。`match_pattern`/`count_colors`/`total_beads`/`generate_summary` 是全函数（不失败）。**不新增 `BeadError` 变体**。
  - **注（review m5/CR）**：CLI 给的 `--width`/`--height` 是 `u32`；病态巨值（如 100000×100000）可能在 `image_to_grid` 的 resize 处 OOM-abort（同 M5-R4-B1 接受的 OOM 边界），或先撞到 `render_*` 的过大缓冲守卫返 `InvalidImage`——后者经管线透传、确定性返 `Err`、不 panic；前者是接受的病态 OOM（现实 `--width/--height` 远不及）。
- **`pattern_json` 不引入新错误维度（review M1）**：它返 `String`（不可失败，D4）、**不返 `Result`**——故 `BeadError` **无须**也**不应**有序列化错误变体（避免误用 `PaletteParse`），D7「不新增变体」与之自洽。
- CLI（`anyhow`）逐步 `.context(...)`：「failed to read input image {path}」「failed to load palette {path}」「failed to generate pattern」「failed to write {path}」——点名文件/参数，core 的 `reason` 被透出。
- **理由**：① 管线无新失败模式——所有失败来自被它调用的原语，复用其错误（M3-D7「复用不加」）。② 规则 1：fs/IO 错误 + `anyhow` 语境只在 CLI；core 只返 `BeadError`。
- **替代方案：新 `PipelineError`/包装层。** 否决：无新失败语义、徒增变体；`#[non_exhaustive]` 的 `BeadError` 已留门、真要管线级诊断再非破坏加。

**D8 — 确定性 + 两个「不可达」由构造成立（空网格、`cell_size<5`）。**

- 整条管线**可复现**：各阶段已确定（M2 resize 同输入同 `PixelGrid`、M3 match 整数最近色、M4 stats 整数计数、M5 render 像素整数 + 编码参数钉死）。故 `GenerateResult` 对同 `(bytes,palette,opts)` 在**同平台 + 同依赖版本**下逐字段确定、四文件逐字节可复现。
  - **跨架构范围（review M6-R3/Codex）**：M6 默认 `Lanczos3`（f32 重采样）的输出**未保证跨架构 byte 一致**——M2 的 `grid_is_deterministic` 测试**刻意用 `Nearest`（整数精确、无 f32）作跨架构 inline golden、明言不钉 Lanczos3 f32 输出**（`image/mod.rs`）。`cells` 及其下游 `stats`/`summary`/`pattern.json`/两张 PNG **全部源自这次 resize**，故端到端逐字节一致**以「同平台 + 同依赖版本」为界**；resize **之后**的链（match→stats→summary→render-像素→编码）是纯整数、跨架构稳，唯一跨架构不稳的环节是 resize 本身。这对 M7 golden（固定平台冻结 / 或 demo 改 `Nearest`）与 M8「CLI==FFI」（同机同设备）**已足**——二者均不要求跨架构 byte 同。**不**在 M6 声称跨架构 byte 一致；跨架构 golden 策略归 M7（M5-D3 的「dep bump 响亮失败」承接跨版本一支）。
- **空网格管线不可达**（M5-D7 已查证）：`image_to_grid` 内 `resize_image` 先拒 `width==0||height==0`（返 `InvalidImage`）→ `generate_pattern` 在配色前即 `Err`、**永不产空 `BeadPattern`**。故 render 的「空网格→Err」与 stats 的「空网格→Ok([])」分歧在管线**不可达**；M6 **不加**空网格特例，靠 `image_to_grid` 既有守卫。
- **`render_grid` 的 `cell_size<5→Err` 不可达**：M6 不暴露 `--cell-size`、固定用 `RenderOptions::default()`（`cell_size=10`）→ `10>=5` 恒成立、该报错永不触发。
- **理由**：规则 2/4 + M5-D7 查证。把「不可达」写明，免得 reviewer 误以为缺特例处理。

**D9 — `samples/` demo 输入 + `grid.png` 的 golden 状态归 M7（显式标注）。**

- 新增 `samples/`（首次需要时建）下一张**小的确定性合成 PNG**（如生成的渐变/色块，纯整数像素、跨架构稳），使 INIT 示例「一条命令可跑」、且**兼作 M7 的固定 golden 输入**。M6 只加这张输入图；`tests/golden/*` 固化归 M7。
- **`grid.png` golden 状态**：INIT 的 golden 清单（`tests/golden/`）是 `preview.png` + `pattern.json` + `summary.txt`——**不含** `grid.png`。M6 仍**写** `grid.png`（CLI 输出契约要求四文件），但是否**冻结**它由 M7 决（PNG 字节跨版本不稳、M5-D3 已记，M7 须决定 `Cargo.lock` 钉版 + 「dep bump 响亮失败」）。**显式标注**：`grid.png` 不在 golden 清单是 INIT 既定、非 M6 缺口。
- **理由**：ROADMAP M6 done-when（端到端可跑）需要一个真实输入；INIT「Project Structure」已预留 `samples/`。小且确定 → 兼 M7 输入。
- **替代方案：放真实照片。** 否决：体积大、且非确定（来源不可控）；合成图小、确定、可复现。

**D10 — M6 须钉死的边界（确定性门 / 测试清单）。**

1. **管线串联正确**：对小输入（用一张已知小图字节 + 内置/构造 palette + 已知 w/h）调 `generate_pattern`，断言 `GenerateResult`：`pattern.width/height==w/h`、`cells.len()==w*h`、`stats` 与 `count_colors` 一致、`summary == generate_summary`、`preview_png`/`grid_png` 非空且能解码、且**与单独调各原语的结果逐一相等**（证明 pipeline 只是忠实串联、未引入差异）。
2. **单一-Palette 不变量**：`generate_pattern` 内部仅用入参 `palette` 一份；测试「同 `cells` 下标在 stats 的 code/name 与 render 的 rgb 来自同一 palette」（构造能区分的 palette 断言）。
3. **`pattern.json` 形状**：`pattern_json` 产出的 JSON **含键集** `{ brand, width, height, cells, total, stats }`（断言键存在、不假定顺序——JSON 对象键序语义无关）、`total==cells.len()==Σstats.count`、`cells` 是整数数组、`stats[i]` 含 `code/name/count`；**同次/重复序列化逐字节相同**（key 序由 serde 字段序确定、确定性；逐字节 frozen golden 归 M7）。
3b. **`BeadPattern`/`ColorStat` 可序列化且不 `Eq`**：编译期保证 derive `Serialize`；断言 `serde_json::to_string` 成功。
4. **错误透传**：① 坏图字节 → `Err`（`ImageDecode`）；② `width==0`/`height==0` → `Err(InvalidImage)`，**不 panic**——断言其 `reason` **源自 `image_to_grid` 的目标维度守卫**（如 `reason` 含 "target width"/"target height"），从而**确定性地证明失败发生在配色/渲染之前**（即空网格不可达是「先于 match/render 失败」、由构造成立，而非靠断言一个不可测的「分支永不进入」负命题，review m3/CR）；③ 非法 palette（经 CLI 或直接构造）→ 对应 `BeadError`。
5. **确定性**：同 `(bytes,palette,opts)` 两次 `generate_pattern` → `pattern`/`stats`/`summary` 相等、两张 PNG 字节相等。
6. **CLI 端到端**（集成测试，临时目录用 `CARGO_TARGET_TMPDIR`、零依赖）：`generate --input samples/<img> --palette palettes/artkal_s.json --width .. --height .. --output <tmp>` → 退出 0、`<tmp>` 下出现四文件且非空、`pattern.json` **为合法 UTF-8 且以 `{` 起头**（**不在 bead-cli 解析 JSON**——`serde_json` 仅 bead-core 直接依赖、CLI 侧连 dev-dep 也不加；键集解析断言留在 bead-core 测 D10.3、那侧 `serde_json` 已在作用域，守住 bead-cli 仅 bead-core/clap/anyhow，review M6-R2 Codex/RC）、`summary.txt` 首行 `Bead Pattern Summary`；重跑覆盖产同样字节。`palette validate <good>` 退 0、`<bad>` 退非 0；`palette list`/`inspect` 退 1 且 stderr 含 coming soon。

## 风险 / 权衡

- [D5 给真相源加 `Serialize`] → 仅 `statistics` 的「ColorStat 输出形状」一条需求（明文钉 derive 集）经 `## 修改需求` delta 追加 `Serialize`（「不 Eq」不变）；`BeadPattern` 因 color-matching 未钉 derive 集而无 delta、仅改 models doc-comment（逐处见 tasks）；不靠 prose 顺带改已生效需求。
- [D2 总在内部渲染两张 PNG（即便调用方只想要 stats）] → M6 的活是全量 generate，可接受；未来「只要 stats」的轻量入口非破坏另加。
- [D4 `cells` 作 JSON 整数数组] → 80×100=8000 数（~30KB）现实可接受；超大图模式是 Phase-2 关切，非 M6。
- [grid.png 不在 golden 清单（D9）] → INIT 既定；M6 标注、M7 决定是否冻结。PNG 字节跨版本不稳由 M7 的「dep bump 响亮失败」承接（M5-D3 已立）。
- [`GenerateOptions::default()` 的 0×0（D3）] → 非「能跑的配置」、是填充便利；调用方不设维度则 `image_to_grid` 干净返 `Err`，不 panic。

## Migration Plan

无运行时迁移：新增 `pipeline` + `cli` 两能力 + 新结构（`GenerateResult`/`GenerateOptions`/局部 `PatternFile`）+ `samples/` 输入图；`models` 两个结构（`BeadPattern`/`ColorStat`）**追加** `Serialize`（非破坏）。规范层仅 `statistics` 一处**非破坏**措辞修正（`## 修改需求` delta 给 ColorStat 追加 `Serialize`）；`color-matching` 无 delta（未钉 derive 集）。**不改** palette/image-grid/renderer 的任何需求、不新增 `BeadError` 变体、不加依赖。回滚 = 撤销本变更。M7 golden 固化 / M8 FFI 桥是既定后续、非本变更。

## Open Questions

- **demo 输入图的具体内容** = 已在 tasks §5.1 **钉死**（32×40 RGB8、`r=x%256,g=y%256,b=(x+y)%256`，同 M2 `gradient_rgb8.png` 公式家族），兼作 M7 固定 golden 输入、免 M7 再决；不影响规范层（review M6-R3/CR）。
- **`grid.png` 是否进 M7 golden** = M7 决策；M6 只承诺写出它、标注其 golden 状态待定。
- **退出码语义**（`palette validate` 失败用 1 还是 2）= 沿用全局 CLI 约定（0 成功 / 1 业务失败 / 2 参数错误）；tasks 钉死。
