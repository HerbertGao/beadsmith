# cli 规范（增量）

## 修改需求

### 需求:generate 子命令端到端写出四个文件
`bead-cli generate --input <img> --palette <json> --width <w> --height <h> --output <dir> [--max-colors <N>] [--matcher <rgb|lab|oklab>] [--despeckle <N>]` MUST **真实实现**：读取 `--input` 的图片字节与 `--palette` 的调色板（`load_palette`），调用 `pipeline::generate_pattern`，在 `--output` 目录下写出四个文件——`preview.png`（`preview_png` 字节）、`grid.png`（`grid_png` 字节）、`pattern.json`（报告序列化，见 pipeline 规范）、`summary.txt`（`summary` 字符串）。**所有文件系统读写必须只在 CLI 层**（`bead-cli`），`bead-core` 保持无 fs。`generate` 暴露 **5 个必需 flag**（`--input`/`--palette`/`--width`/`--height`/`--output`）**加 3 个可选 flag `--max-colors <N>`、`--matcher <rgb|lab|oklab>` 与 `--despeckle <N>`**。

`--max-colors`：未给 → `GenerateOptions.max_colors = None`（全调色板配色，行为与未引入减色前一致）；给 `N` → `max_colors = Some(N)`，把图案输出限制到 **≤N 种**拼豆色（见 color-reduction / pipeline 规范）。CLI **不**内置品牌档位枚举——`--max-colors` 接任意 `u32`（`Some(0)` 会经 `generate_pattern` 确定性返 `Err`、非零退出），help **可**提示常见档位 24/36/48/72。

`--matcher`：用 `clap::ValueEnum` 接 `rgb`/`lab`/`oklab` 三值之一，CLI 侧手写映射到 core 的 `MatcherKind`（core **不**依赖 clap）；**未给时默认 `oklab`**（与引擎默认 `MatcherKind::Oklab` 一致）。非法值由 clap 在参数解析阶段拒绝、以**参数错误退出码 2** 结束、不 panic。`cell_size`/`filter` **仍**用默认（`RenderOptions::default()` 的 `cell_size==10`、`ResizeOptions::default()` 的 **`Triangle`**）、**仍禁止**暴露为 flag（故 `render_grid` 的 `cell_size>=5` 约束恒满足、永不触发）。

`--despeckle`：接任意 `u32`，映射到 `GenerateOptions.despeckle`——**未给** → `None`（不去斑，行为与未引入该 flag 前一致）；给 `N` → `Some(N)`，清理珠数 ≤ `N` 的小同色连通分量、并入多数邻色（见 pattern-cleanup / pipeline 规范）。**`--despeckle 0` 是合法 no-op**（不清理任何分量、退出 0、图案与不去斑一致）。非 `u32` 值由 clap 在参数解析阶段拒绝、以**参数错误退出码 2** 结束、不 panic。

#### 场景:INIT 示例命令写出四个非空文件
- **当** 以一张有效图片、有效调色板与正整数 `--width`/`--height` 运行 `bead-cli generate ... --output <dir>`（**不给** `--max-colors`、`--matcher`、`--despeckle`）
- **那么** 进程退出码为 0，`<dir>` 下存在 `preview.png`、`grid.png`、`pattern.json`、`summary.txt` 四个非空文件；`pattern.json` 为**合法 UTF-8 且以 `{` 起头**（CLI 侧只做此轻校验、**不引 `serde_json` 解析**）、`summary.txt` 首行为 `Bead Pattern Summary`；**且不给 `--matcher` 时配色用默认 `oklab`、`filter` 用默认 `Triangle`、`--despeckle` 未给时不去斑（`None`）**

#### 场景:--max-colors 限制输出拼豆色数
- **当** 以有效图片/调色板 + `--max-colors N`（`N >= 1`）运行 `generate`
- **那么** 退出码为 0、写出四个文件，且 `pattern.json`/`summary.txt` 反映的图案**不同拼豆色数 ≤ N**（≤ 上限语义，见 color-reduction 规范）

#### 场景:--matcher 各值成功、非法值退出码 2
- **当** 以 `--matcher rgb`、`--matcher lab`、`--matcher oklab` 分别运行有效 `generate`
- **那么** 三者均退出码 0、写出四个文件，且配色分别用 `RgbMatcher`/`LabMatcher`/`OklabMatcher`
- **且** 以 `--matcher <非三值之一>`（如 `--matcher hsv`）运行时，进程以**退出码 2**（参数错误）结束、stderr 含可选值提示、**不 panic**、不写出文件

#### 场景:--despeckle 清理小斑点、0 为 no-op、非 u32 退出 2
- **当** 以有效图片/调色板 + `--despeckle N`（`N >= 1`）运行 `generate`
- **那么** 退出码 0、写出四个文件，图案中珠数 ≤ N 的孤立小同色分量被并入邻色（见 pattern-cleanup 规范）
- **且** `--despeckle 0` 退出码 0、写出四个文件，且图案与不给 `--despeckle` 时**一致**（合法 no-op）
- **且** 以 `--despeckle <非 u32>`（如 `--despeckle x`）运行时以**退出码 2** 结束、**不 panic**、不写出文件
