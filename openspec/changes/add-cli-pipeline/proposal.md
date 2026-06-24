## 为什么

里程碑 **M6 — CLI + pipeline**。M1–M5 已建齐引擎各原语（`palette` / `image`(含 `image_to_grid`) / `matcher` / `statistics` / `renderer`），但它们目前是**散装库原语**、无统一入口、无法从命令行端到端跑。M6 做两件事：① `bead-core` 新增 `pipeline::generate_pattern`——**唯一的完整生成/编排入口**（ARCHITECTURE 规则 4：外部不在管线外自行拼装 image→match→stats→render；`load_palette`/`pattern_json` 等输入解析/序列化 helper 仍是允许的公开函数），把 load→resize→match→stats→render 串成一条确定性管线、产出一个打包结果；② `bead-cli generate` 薄壳调用它、把结果写成 INIT 约定的四个文件。这是「引擎可被人用、可被验证」的第一步，也是 M7 golden 冻结与 M8「CLI == FFI」的共同地基（**规则 5**：CLI 是契约，前端与 `bead-cli` 不一致即前端 bug）。

## 变更内容

- `bead-core` 新增 `pipeline` 模块（**唯一生成/编排入口**；`load_palette`/`pattern_json` 等输入解析/序列化 helper 仍是允许的公开函数）：
  - `pub fn generate_pattern(image_bytes: &[u8], palette: &Palette, opts: &GenerateOptions) -> Result<GenerateResult, BeadError>`——内部按序调
    `image_to_grid → RgbMatcher::new(palette) → match_pattern → count_colors/total_beads/generate_summary → render_preview/render_grid`，
    **把同一份 `palette` 引用喂给 matcher / stats / render**（落地 M4/M5 推迟的「管线持单一 Palette」不变量，**从类型上消灭** M4-D4 的「配错调色板」隐患）。
  - `pub struct GenerateResult { pattern: BeadPattern, stats: Vec<ColorStat>, summary: String, brand: String, preview_png: Vec<u8>, grid_png: Vec<u8> }`——
    pipeline **内部渲染两张 PNG**，使 CLI 与 FFI 都只「写出」、消费同一个结果对象、**不可能分歧**（规则 5 / M8「Dart 拿到与 CLI 相同结果」）。`brand`（取自入参 palette）也入结果，使
    `pattern_json` 只取 `&GenerateResult`、**无需**再传一份可能配错的 `palette`——把 D1「传错调色板结构性不可能」的保证延伸到序列化层（review M6-R3/Codex）。
  - `pub struct GenerateOptions { width: u32, height: u32, resize: ResizeOptions, render: RenderOptions }`（derive `Default`、**非** `#[non_exhaustive]`，同
    `RenderOptions` 取舍）。`bead-core` **不碰文件系统**（规则 1）——`image_bytes` 与 `&Palette` 由调用方读入。
- `bead-cli` 新增 `clap` 子命令（`anyhow`，**所有 fs/IO 在 CLI**）：
  - `generate --input --palette --width --height --output`——**真实实现**：读图字节 + `load_palette`，调 `generate_pattern`，`create_dir_all(--output)` 后
    **覆盖写** `preview.png` / `grid.png` / `pattern.json` / `summary.txt`。仅暴露 INIT 这 5 个 flag（`cell_size`/`filter` 用默认 10/Lanczos3、不暴露）。
  - `palette validate <path>`——**真实接上**（`load_palette` 即足、其内部已含完整校验）。`palette list` 与 `inspect`——**桩**为显式 `coming soon` 报错（exit 1，非静默成功）。
- `pattern.json` 形状（INIT/ARCHITECTURE 均未定义，本 change 定）：**完整报告**键集 `{ brand, width, height, cells, total, stats }`——`cells` 是真相，`brand` 是**指回原
  调色板的指针**（不内嵌完整色表，YAGNI）。**注（review M6-R4/Codex）**：这是完整的*报告*、但**非独立可 render**——`cells` 的整数下标须靠 `brand` 标识的**同一份调色板（同序）**
  才能映射回 RGB（`stats` 按 count 排序、不带下标，单凭 JSON 无法解码任意 `cells[i]`→色）；M6 **只写不读**（不实现读回、见非目标），故不追求独立可 render。键序由 serde
  字段序确定（`flatten` 把 `width/height/cells` 注入在 `brand` 后、`total` 前），**同 serde 版本内**逐字节确定、由 M7 frozen golden 锁；规范只钉键集与含义（JSON 对象键序语义无关）。
- 序列化接线：给 `models` 的 `BeadPattern` / `ColorStat` 加 **`#[derive(Serialize)]`**（仅 Serialize、不加 Deserialize）；`pipeline` 用局部
  `PatternFile { brand, #[serde(flatten)] pattern, total, stats }` 承载顶层 `brand`/`total`、避免 DTO 漂移（规则 3 单一真相源），并由 `pattern_json(&GenerateResult) -> String`
  （**不可失败**，纯数据序列化；`brand` 取自 `result.brand`、**不再单收 `palette`**，杜绝配错）产出——CLI 不依赖 `serde_json`、`BeadError` 无须序列化错误变体。`serde`/`serde_json` 已是 `bead-core` 依赖。
- 新增 `samples/` 下一张**小的确定性合成 PNG** 作为一条命令即可跑的 demo 输入（兼作 M7 的固定 golden 输入）。

## 功能 (Capabilities)

### 新增功能
- `pipeline`: `bead-core` 的唯一**生成/编排**入口 `generate_pattern`（输入解析 `load_palette` / 序列化 `pattern_json` 等 helper 仍公开）——确定性串联 load→resize→match→stats→render，持单一 `Palette` 喂三方，产出打包的
  `GenerateResult`（pattern + stats + summary + brand + 两张 PNG 字节）；并定义 `pattern.json` 的序列化形状。
- `cli`: `bead-cli` 命令面与**文件输出契约**——`generate`（真实，写四文件、覆盖、建目录）、`palette validate`（真实）、`palette list`/`inspect`（显式桩）；
  所有 fs/IO + `anyhow` 错误语境在此层，`bead-core` 保持无 fs。

- `statistics`：「ColorStat 输出形状」需求——`ColorStat` 现**额外** derive `Serialize`（进入 `pattern.json` 的 `stats`，规则 3：序列化真相源本身、不另立 DTO）。
  规范性其余不变：仍 `code:String`/`name:String`/`count:u32`、仍 `Debug+Clone+PartialEq`、**仍不 `Eq`**；仅追加 `Serialize`。经 `## 修改需求` delta 修正该需求文字（它确切钉了 derive 集）。
- **注**：`BeadPattern` 也加 `Serialize`，但 `color-matching` 的「BeadPattern 输出形状」需求**未**钉 derive 集（已查证：该规范无 derive/Eq/Serialize 字样）→ 加 `Serialize` 不触动任何已生效需求，**无需 color-matching delta**；仅改 `models/mod.rs` 的 doc-comment（tasks，非规范变更）。

## 非目标（Non Goals）

按 YAGNI 推迟 / 不做：

- **读回 `pattern.json`（`load_pattern` / `inspect` 真实实现）** → M6 只写不读；结构前向可读，未来加 `Deserialize` + loader 是纯增量、非破坏，推迟到 Flutter「SaveProject」阶段（M9+）。
- **`--cell-size` / `--filter` 等渲染/缩放 flag** → M6 用默认（cell_size 10 / Lanczos3）；故 grid 的 `cell_size>=5` 报错在 M6 永不触发。要可配后续非破坏加 flag。
- **`palette list` 真实实现** → 需要尚不存在的「调色板目录约定」；M6 桩为显式报错。
- **quantizer（降色，Phase 2）/ rayon（并行，Phase 2）/ CSV 导出（INIT 明确不要）** → 不在 M6。
- **`tests/golden/*` 固化** → M7。M6 只交付 demo 输入图（`samples/`）与端到端可跑，golden 冻结归 M7。
- **`grid.png` 的 golden 状态** → INIT golden 清单是 `preview.png` + `pattern.json` + `summary.txt`（**不含** `grid.png`）。M6 仍**写** `grid.png`，但是否**冻结**它由 M7 决定；本 change 显式标注，避免把它不在 golden 清单里读成 M6 缺口（见 design）。

## 影响

- **代码**：
  - `crates/bead-core/src/pipeline/mod.rs`（新）——`generate_pattern` / `GenerateResult` / `GenerateOptions` / 局部 `PatternFile` + `pattern.json` 序列化。
  - `crates/bead-core/src/models/mod.rs`（改）——`BeadPattern` / `ColorStat` 加 `#[derive(serde::Serialize)]`。
  - `crates/bead-core/src/lib.rs`（改）——`pub mod pipeline;` + 重导出 `generate_pattern / GenerateResult / GenerateOptions`；**不新增 `BeadError` 变体**（管线复用各阶段既有错误）。
  - `crates/bead-cli/src/main.rs`（改/扩）——`clap` 子命令 `generate` / `palette validate|list` / `inspect`；所有 fs + `anyhow` 语境。
  - `samples/`（新）——一张确定性合成 PNG。
- **依赖**：无新增（`serde`/`serde_json`/`clap`/`anyhow`/`image` 均已在 workspace；`pattern.json` 用 `serde_json`，CLI 用 `clap`+`anyhow`）。
- **确定性**（规则 2，硬门）：整条管线**可复现**（各阶段已是）——同**平台 + 同依赖版本**下，同 `(bytes,palette,opts)` 重跑产逐字节相同的 `pattern.json` / `summary.txt` / 两张 PNG。
  **范围说明（review M6-R3/Codex）**：M6 默认用 `Lanczos3`（f32 重采样），其输出**未保证跨架构逐字节一致**（M2 的 determinism 测试为此**刻意只用 `Nearest` 作跨架构 golden**、不钉 Lanczos3 f32 输出，见 `image/mod.rs`）；`cells` 及其下游（`stats`/`summary`/`pattern.json`）都源自这次 resize，故**端到端逐字节一致以「同平台 + 同依赖版本」为界**（resize 之**后**全是纯整数、跨架构稳；不稳的链是 resize 本身）。这对 M7 golden（在固定平台冻结、或 demo 输入改用 `Nearest`）与 M8「CLI==FFI」（同机同设备）**足够**——二者都不要求跨架构 byte 同。跨架构 golden 策略由 M7 决（钉平台 / 改 `Nearest` / 加跨架构校验门）。
- **里程碑 / Phase**：里程碑 M6；Phase 1（单线程；Quantize 属 Phase 2、不启用）。
- **文档**：无真理源结构反转（不像 M4）。ARCHITECTURE 的 pipeline 段（`generate_pattern(...)`、「唯一入口」）、ROADMAP M6、INIT「CLI Requirements / Summary Format」均与本设计一致；**注**：ARCHITECTURE 的 pipeline 流程图含一个 **Quantize** 步（Resize→Quantize→Match），M6 链**有意省略**（降色是 Phase 2、不启用，见非目标）——是既定 Phase 分档、非不一致。仅 **statistics 一处**需求经 delta 追加 `Serialize`（`color-matching` 未钉 derive 集、无 delta；`BeadPattern` 的 `Serialize` 仅改 models doc-comment）。见「修改功能」。
