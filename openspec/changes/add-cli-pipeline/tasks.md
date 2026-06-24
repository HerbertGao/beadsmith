## 1. models：BeadPattern / ColorStat 加 Serialize

- [x] 1.1 `crates/bead-core/src/models/mod.rs`：给 `BeadPattern` 加 `serde::Serialize`（`#[derive(Debug, Clone, PartialEq, serde::Serialize)]`，
  **保留**不 `Eq`、**不**加 `Deserialize`，D5）；更新其 doc-comment 提及现可序列化进 `pattern.json`（M6-D4/D5）
- [x] 1.2 同文件给 `ColorStat` 加 `serde::Serialize`（同上：保留 `Debug+Clone+PartialEq`、不 `Eq`、不 `Deserialize`）；与 statistics 规范 delta 一致（5.x）
  - `// ponytail: 序列化真相源本身、不立 DTO 镜像（规则 3 / M4-D1 反脱节）；只 Serialize 不 Deserialize（M6 只写不读）`

## 2. pipeline 模块：GenerateOptions / GenerateResult / generate_pattern / PatternFile

- [x] 2.1 新建 `crates/bead-core/src/pipeline/mod.rs`，模块 doc：唯一**生成/编排**入口（规则 4；`load_palette`/`pattern_json` 等 helper 仍公开）；忠实串联既有原语、无新算法；持单一 `Palette` 喂
  matcher/stats/render（消灭 M4-D4 隐患）；`bead-core` 无 fs（规则 1，`image_bytes`/`&Palette` 由调用方读入）
- [x] 2.2 `pub struct GenerateOptions { pub width: u32, pub height: u32, pub resize: ResizeOptions, pub render: RenderOptions }`
  （derive `Debug, Clone, PartialEq`；`impl Default` → `width:0,height:0,resize:Default,render:Default`，**非** `#[non_exhaustive]`，D3）
  - `// ponytail: Default 的 0×0 非「能跑配置」、是 ..default() 填充便利；维度非法由 image_to_grid 既有 0-守卫干净返 Err、不 panic`
- [x] 2.3 `pub struct GenerateResult { pub pattern: BeadPattern, pub stats: Vec<ColorStat>, pub summary: String, pub brand: String, pub preview_png: Vec<u8>, pub grid_png: Vec<u8> }`
  （derive `Debug`；**不 derive `Clone`**——无消费方需克隆整个含两块 PNG 字节的结果，CLI 写完即弃，YAGNI，review m4；M8 真要再非破坏加）
  - `// ponytail: brand 入结果（= palette.brand 克隆）→ pattern_json 只取 &GenerateResult、不单收 palette，杜绝配错（D2/M6-R3-Codex）；代价一次 String 克隆，相对两块 PNG 可忽略`
- [x] 2.4 `pub fn generate_pattern(image_bytes: &[u8], palette: &Palette, opts: &GenerateOptions) -> Result<GenerateResult, BeadError>`：按序
  `let grid = image_to_grid(image_bytes, opts.width, opts.height, &opts.resize)?;` → `let m = RgbMatcher::new(palette)?;` →
  `let pattern = match_pattern(&grid, &m);` → `let stats = count_colors(&pattern, palette);` `let summary = generate_summary(&pattern, palette);` →
  `let preview_png = render_preview(&pattern, palette, &opts.render)?;` `let grid_png = render_grid(&pattern, palette, &opts.render)?;` → 组装 `GenerateResult`
  （含 `brand: palette.brand.clone()`，D2/M6-R3）。**同一 `palette` 喂三方**（D1/单一-Palette 不变量）；`?` 透传各阶段 `BeadError`、**不新增变体**（D7）
- [x] 2.5 私有 `#[derive(serde::Serialize)] struct PatternFile<'a> { brand: &'a str, #[serde(flatten)] pattern: &'a BeadPattern, total: u32, stats: &'a [ColorStat] }`
  （字段序 → JSON 键序 `brand, width, height, cells, total, stats`；`flatten` 注入在 `brand` 后、`total` 前，review M4/Codex 对齐广告序）
  + `pub fn pattern_json(result: &GenerateResult) -> String`（**返 `String`、不返 `Result`**，review M1/Codex/RC；**只取 `&GenerateResult`、不单收 `palette`**——`brand` 已在 `result.brand`，杜绝配错，M6-R3/Codex）：
  `serde_json::to_string(&PatternFile { brand: &result.brand, pattern: &result.pattern, total: total_beads(&result.pattern), stats: &result.stats })`
  `.expect("PatternFile 是纯数据（无非字符串 map 键/无会失败的 Serialize），序列化不可失败")`。
  `// ponytail: 纯数据序列化不可失败 → 返 String、不引 Result/新错误变体（BeadError 无序列化变体、不误用 PaletteParse）；flatten 承载顶层 brand/total、不立 BeadPattern 的 DTO 镜像（D4/D5）`
  - `// ponytail: 前向约束——PatternFile 须保持纯数据；任一可达字段加自定义 Serialize/map 键会使 .expect() 可达、panic（review M6-R2/CR nit）`

## 3. lib.rs：重导出

- [x] 3.1 `crates/bead-core/src/lib.rs`：`pub mod pipeline;` + 重导出 `pub use pipeline::{generate_pattern, GenerateResult, GenerateOptions};`
  （紧邻现有 `pub use renderer::{…}` 风格）；**不新增 `BeadError` 变体**（D7）。`pattern_json` 经 `pub mod pipeline` 以模块路径 `pipeline::pattern_json` 触达（CLI 即如此调）、**无需** crate 根重导出（单一既定访问路径，review M6-R4/CR）
- [x] 3.2 改 `crates/bead-core/src/statistics/mod.rs` 模块 doc（约 L8-9）：把 `generate_pattern` 的 "single external entry" 措辞改为 "single **generation/orchestration** entry"
  （与 M3 收窄一致——`load_palette`/`pattern_json` 等 helper 仍公开；这是 M6 创建 `generate_pattern` 时该一并更正的前向引用 doc，review M6-R3/Codex）

## 4. bead-cli：clap 子命令 + 文件 I/O + anyhow（全部 fs 在此层）

- [x] 4.1 `crates/bead-cli/src/main.rs`：用 `clap`(derive) 定义子命令枚举——`Generate { input, palette, width, height, output }`、
  `Palette { #[command(subcommand)] PaletteCmd }`（`Validate { path }` / `List`）、`Inspect { path }`。`main` 用 `anyhow::Result<()>`，按子命令分发
- [x] 4.2 `generate`（**真实**，D6）：`let img_bytes = std::fs::read(&input).with_context(|| format!("failed to read input image {input:?}"))?;`
  `let pal_bytes = std::fs::read(&palette).with_context(|| format!("failed to read palette {palette:?}"))?;`
  `let palette = load_palette(&pal_bytes).with_context(|| format!("invalid palette {palette:?}"))?;`（`load_palette` 收 **`&[u8]`**——用 `fs::read` 取**字节**，**非** `read_to_string`，review m1/CR/RC/Codex）；
  `let opts = GenerateOptions { width, height, ..Default::default() };`（默认 cell_size 10 / Lanczos3，不暴露这俩 flag）；
  `let result = generate_pattern(&img_bytes, &palette, &opts).context("failed to generate pattern")?;`
  `fs::create_dir_all(&output).with_context(|| format!("failed to create output dir {output:?}"))?;` 然后**覆盖写**四文件（各 `.with_context` 点名路径）：
  `preview.png`←`result.preview_png`、`grid.png`←`result.grid_png`、`pattern.json`←`pipeline::pattern_json(&result)`（**`String`**，无 `?`、CLI 不依赖 serde_json）、`summary.txt`←`result.summary`
- [x] 4.3 `palette validate <path>`（**真实**，D6）：`load_palette(&fs::read(path).with_context(|| format!("failed to read palette {path:?}"))?)`
  `.with_context(|| format!("invalid palette {path:?}"))?` **即足**——`load_palette` 内部已调 `validate_palette`（非空 + hex + 唯一 code，已查证）。
  **读侧 fs 失败也要 `.with_context` 点名路径**（不裸 `fs::read(path)?`——否则 OS 错误无路径语境，review M6-R4/Codex）：path 不存在/不可读 → 非零退出、stderr 含路径 + OS 错误、不 panic。
  ok 打印成功、退出 0；校验失败把 `BeadError` 的 `reason` 经 `anyhow` 写 stderr、非零退出（业务失败 1）。**不必再单独调 `validate_palette`**（冗余 no-op，review m2/CR/RC/Codex）
- [x] 4.4 `palette list` 与 `inspect <path>`（**桩**，D6）：`anyhow::bail!("... coming soon (not implemented in M6)")` → exit 1；
  **禁** 静默成功 / `unimplemented!`（后者 panic）。`// ponytail: 桩成显式非零退出，不假绿、不 panic`
- [x] 4.5 退出码语义：成功 0 / 业务失败 1 / clap 参数错误 2（clap 默认）。确认 `main` 的 `anyhow` 错误以非零退出且打印到 stderr

## 5. samples/：demo / M7 输入图

- [x] 5.1 新增 `samples/`（首次建）下一张**确定性合成 PNG**：**32×40 RGB8、像素 `r=x%256, g=y%256, b=(x+y)%256`**（同 M2 `gradient_rgb8.png` 公式家族；
  32:40==4:5、对 4:5 目标为 no-op crop），纯整数像素源、字节提交入库，使
  `cargo run -p bead-cli -- generate --input samples/<img> --palette palettes/artkal_s.json --width .. --height .. --output ./result` 一条命令可跑。
  `// ponytail: 尺寸+像素公式钉死（兼作 M7 固定 golden 输入、免 M7 再决），review M6-R3/CR；demo 默认 Lanczos3 仅供肉眼看，M7 决 golden 用 Nearest 还是钉平台；tests/golden/* 固化归 M7`

## 6. 测试（映射 spec 需求 + 确定性门）

- [x] 6.1 `generate_pattern_chains_faithfully`（pipeline 规范「忠实串联」/ D10.1）：对小图字节 + 构造 palette + 已知 w/h 调 `generate_pattern`，断言
  `pattern.width/height==w/h`、`cells.len()==w*h`、`stats == count_colors(...)`、`summary == generate_summary(...)`、`brand == palette.brand`、两张 PNG 非空且能解码，
  且与单独调各原语结果逐一相等
- [x] 6.2 `single_palette_invariant`（pipeline「单一 Palette 不变量」/ D10.2）：构造能区分的 palette，断言 stats 的 code/name 与渲染像素 rgb 来自同一份
  （`ColorStat` 只带 `code/name`、**不带 palette 下标**——测试经 `palette.colors.iter().position(|c| c.code == stat.code)`[唯一 code 保证] 还原下标、再取该格渲染 rgb 比对，review M6-R3/RC F1）
- [x] 6.3 `pattern_json_shape`（pipeline「pattern.json 形状」/ D10.3）：调 `pattern_json(&result)` → **解析**（用 bead-core 内的 `serde_json`，测试侧）断言**含键** `brand/width/height/cells/total/stats`（不假定顺序）、
  `total==cells.len()==Σ stats.count`、`cells` 为整数数组、`stats[i]` 含 `code/name/count`；同次/两次序列化逐字节相同（`pattern_json` 返 `String`、不可失败）
- [x] 6.4 `models_serialize`（statistics delta / D10.3b）：断言 `serde_json::to_string(&ColorStat{..})` 与 `&BeadPattern{..}` 成功且形状正确
- [x] 6.5 `pipeline_errors_passthrough`（pipeline「错误透传」/ D10.4）：① 坏图字节→`Err(ImageDecode)`；② `width==0`/`height==0`→`Err(InvalidImage)`、
  **不 panic**，且断言 `reason` **含 "target width"/"target height"**（证失败先于 match/render、空网格不可达，review m3）；③ 非法 palette→对应 `BeadError`
- [x] 6.6 `pipeline_deterministic`（pipeline「确定性」/ D10.5）：同 `(bytes,palette,opts)` 两次 → `pattern`/`stats`/`summary` 相等、两张 PNG 字节相等
- [x] 6.7 CLI 端到端（D10.6，集成测试 `crates/bead-cli/tests/`；**无新依赖**——用 `std::process::Command` 跑 `env!("CARGO_BIN_EXE_bead-cli")`，临时目录用 `env!("CARGO_TARGET_TMPDIR")` 下建唯一子目录、用后清理，**不引 `tempfile`**，review M6-R2/RC F2）：
  `generate --input samples/<img> --palette palettes/artkal_s.json --width .. --height .. --output <tmpdir>` → 退出 0、四文件非空、`pattern.json` **为合法 UTF-8 且以 `{` 起头**
  （**不在 bead-cli 解析 JSON**——`serde_json` 仅 bead-core 直接依赖、**不**给 bead-cli 加[连 dev-dep 也不加]；键集解析断言留在 bead-core 测 6.3、那侧 `serde_json` 已在作用域，review M6-R2 Codex MAJOR/RC F1）、
  `summary.txt` 首行 `Bead Pattern Summary`；重跑覆盖产同字节。`palette validate <good>`→0、`<bad>`→非0 **且 stderr 含底层 `reason` 子串**（如重复 code / "no colors"，机测 cli §3「core `reason` 透出 stderr」、非仅 prose，review M6-R3/CR）；`palette list`/`inspect`→1 且 stderr 含 coming soon
- [x] 6.8 `cli_fs_failures_nonzero_not_panic`（cli「文件系统失败语义」/ M2；临时路径同 6.7 用 `CARGO_TARGET_TMPDIR`、零依赖）：`--output` 指向已存在的**普通文件** → 非零退出、stderr 含路径语境、不 panic；
  `--input` 指向不存在的文件 → 非零退出、语境点名。**仅机测这两个代表性用例**；规范「至少覆盖」的「父目录不可写 / 磁盘满」环境脆弱（权限/满盘难便携且确定地构造）、**靠论证覆盖**——它们与已测用例走**同一条** `anyhow .context` 非零退出 catch-all 路径（见 cli 规范）、非另起分支（非原子半写姿态文档化、不测原子性，review M6-R2/CR nit1）

## 7. 收尾验证 + delta 确认

- [x] 7.1 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（debug **与** `--release`）全绿；`cargo run -p bead-cli -- generate ...` 手动跑通、肉眼看 `result/preview.png`/`grid.png`
- [x] 7.2 确认**无新依赖**（`serde`/`serde_json`/`clap`/`anyhow`/`image` 均已在；`cargo tree` 不新增 crate）——其中 **`serde_json` 仅 `bead-core` 直接依赖、不出现在 `bead-cli` 的 `[dependencies]` 或 `[dev-dependencies]`**（CLI 测试不解析 JSON、见 6.7；守住 bead-cli 仅依赖 bead-core/clap/anyhow，review M6-R2 Codex/RC）、**无 `rayon`/`HashMap`/`HashSet`/随机** 进 pipeline；
  `bead-core` 仍无 fs/UI/平台依赖（fs 只在 `bead-cli`）
- [x] 7.3 确认 delta 与文档：`statistics` 规范经本 change 的 `## 修改需求` delta 给 ColorStat 追加 `Serialize`（归档时应用到 `openspec/specs/statistics/spec.md`）；
  `color-matching` **无 delta**（未钉 derive 集）、仅 `models` 的 `BeadPattern` doc-comment 改；ARCHITECTURE(pipeline 段)/ROADMAP(M6)/INIT(CLI/Summary) 均已一致、未改；
  `grid.png` 不在 INIT golden 清单是既定（design D9，M7 决冻结）
