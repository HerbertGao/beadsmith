# cli 规范（增量）

## MODIFIED Requirements

### Requirement: generate 子命令端到端写出四个文件
`bead-cli generate --input <img> --palette <json> --width <w> --height <h> --output <dir> [--max-colors <N>]` MUST **真实实现**：读取 `--input` 的图片字节与
`--palette` 的调色板（`load_palette`），调用 `pipeline::generate_pattern`，在 `--output` 目录下写出四个文件——`preview.png`（`preview_png` 字节）、
`grid.png`（`grid_png` 字节）、`pattern.json`（报告序列化，见 pipeline 规范）、`summary.txt`（`summary` 字符串）。**所有文件系统读写必须只在 CLI 层**
（`bead-cli`），`bead-core` 保持无 fs。`generate` 暴露 **5 个必需 flag**（`--input`/`--palette`/`--width`/`--height`/`--output`）**加 1 个可选 flag `--max-colors <N>`**：未给 → `GenerateOptions.max_colors = None`（全调色板配色，行为与未引入降色前一致）；给 `N` → `max_colors = Some(N)`，把图案输出限制到 **≤N 种**拼豆色（见 color-reduction / pipeline 规范）。CLI **不**内置品牌档位枚举——`--max-colors` 接任意 `u32`（`Some(0)` 会经 `generate_pattern` 确定性返 `Err`、非零退出，由「CLI 错误带语境」需求覆盖），help **可**提示常见档位 24/36/48/72。`cell_size`/`filter` **仍**用默认（`RenderOptions::default()` 的 `cell_size==10`、
`ResizeOptions::default()` 的 Lanczos3）、**禁止**暴露为 flag（故 `render_grid` 的 `cell_size>=5` 约束恒满足、永不触发）。

#### Scenario: INIT 示例命令写出四个非空文件
- **当** 以一张有效图片、有效调色板与正整数 `--width`/`--height` 运行 `bead-cli generate ... --output <dir>`（**不给** `--max-colors`）
- **那么** 进程退出码为 0，`<dir>` 下存在 `preview.png`、`grid.png`、`pattern.json`、`summary.txt` 四个非空文件；`pattern.json` 为**合法 UTF-8 且以 `{` 起头**
  （CLI 侧只做此轻校验、**不引 `serde_json` 解析**；其「为合法 JSON、键集正确」由 pipeline 层 `pattern_json` 的测试保证——CLI 写出的字节**即** `pattern_json(&result)`
  的 `String`、逐字节相同，故 JSON 有效性经**构造传递**而非 CLI 独立解析）、`summary.txt` 首行为 `Bead Pattern Summary`；**且不给 `--max-colors` 时输出与未引入降色前完全一致**

#### Scenario: --max-colors 限制输出拼豆色数
- **当** 以有效图片/调色板 + `--max-colors N`（`N >= 1`）运行 `generate`
- **那么** 退出码为 0、写出四个文件，且 `pattern.json`/`summary.txt` 反映的图案**不同拼豆色数 ≤ N**（≤ 上限语义，见 color-reduction 规范）
