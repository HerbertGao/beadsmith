# cli 规范

## 目的
定义 `bead-cli` 的**命令面与文件输出契约**——CLI 是契约（规则 5：前端与 `bead-cli` 不一致即前端 bug）。`generate` 子命令读
图片字节 + 调色板，调 `pipeline::generate_pattern`，在输出目录写出 INIT 约定的四个文件（`preview.png`/`grid.png`/`pattern.json`/
`summary.txt`；自动建目录、**覆盖写**以保确定性重跑）；`palette validate` 真实校验（`load_palette` 即完整校验）；`palette list`/
`inspect` 为显式「尚未实现」桩（非零退出、不静默成功、不 panic）。**所有文件系统读写与 `anyhow` 错误语境只在 CLI 层**，
`bead-core` 保持无 fs（规则 1）；fs 失败按读/写两段划分、一律非零退出 + 点名涉事路径、不 panic；退出码语义化（0 成功 / 1 业务
失败 / 2 参数错误）。

## 需求
### 需求:generate 子命令端到端写出四个文件
`bead-cli generate --input <img> --palette <json> --width <w> --height <h> --output <dir>` 必须**真实实现**：读取 `--input` 的图片字节与
`--palette` 的调色板（`load_palette`），调用 `pipeline::generate_pattern`，在 `--output` 目录下写出四个文件——`preview.png`（`preview_png` 字节）、
`grid.png`（`grid_png` 字节）、`pattern.json`（报告序列化，见 pipeline 规范）、`summary.txt`（`summary` 字符串）。**所有文件系统读写必须只在 CLI 层**
（`bead-cli`），`bead-core` 保持无 fs。M6 **仅暴露这 5 个 flag**；`cell_size`/`filter` 用默认（`RenderOptions::default()` 的 `cell_size==10`、
`ResizeOptions::default()` 的 Lanczos3）、**禁止**在 M6 暴露为 flag（故 `render_grid` 的 `cell_size>=5` 约束恒满足、永不触发）。

#### 场景:INIT 示例命令写出四个非空文件
- **当** 以一张有效图片、有效调色板与正整数 `--width`/`--height` 运行 `bead-cli generate ... --output <dir>`
- **那么** 进程退出码为 0，`<dir>` 下存在 `preview.png`、`grid.png`、`pattern.json`、`summary.txt` 四个非空文件；`pattern.json` 为**合法 UTF-8 且以 `{` 起头**
  （CLI 侧只做此轻校验、**不引 `serde_json` 解析**；其「为合法 JSON、键集正确」由 pipeline 层 `pattern_json` 的测试保证——CLI 写出的字节**即** `pattern_json(&result)`
  的 `String`、逐字节相同，故 JSON 有效性经**构造传递**而非 CLI 独立解析）、`summary.txt` 首行为 `Bead Pattern Summary`

### 需求:输出目录自动创建且重跑覆盖（确定性可复现）
`generate` 必须在 `--output` 目录不存在时创建它（含父目录）。对已存在的四个输出文件必须**覆盖写**、而非报错——因为确定性要求同输入重跑产出
**逐字节相同**的文件（这是 M7 golden 重跑的前提）。

#### 场景:对同一输入重跑产出相同文件
- **当** 用相同参数连续运行 `generate` 两次到同一 `--output`
- **那么** 两次均退出 0、覆盖写四个文件，且对应文件内容逐字节相同

### 需求:CLI 错误带文件/参数语境，core 错误被透出
CLI 必须用 `anyhow` 为每个可失败步骤添加语境（点名涉及的文件/参数）：读 `--input`、读+解析 `--palette`、`generate_pattern`、四个写出。
`bead-core` 返回的 `BeadError`（含确定性 `reason` 字段，如非法调色板的原因、无法解码的图片）必须被透出到 stderr。失败时退出码必须非 0
（业务失败 1 / 参数错误 2，沿用项目 CLI 约定）。

#### 场景:无效输入产出带语境的非零退出
- **当** `--input` 指向不存在/无法解码的文件，或 `--palette` 非法
- **那么** 进程退出码非 0，stderr 含点名该文件/参数的语境信息与底层 `reason`，且**不 panic**

### 需求:CLI 文件系统失败语义与非原子写
`generate` 的全部 fs 失败**必须**按**读 / 写两段**划分处理、两段并起覆盖其全部 fs 操作、一律**非零退出 + stderr 点名涉事路径与 OS 错误、不 panic**：**读侧**
（`fs::read(--input)` / `fs::read(--palette)`，含「文件不存在 / 是目录 / 不可读」）的 `io::Error` 由前条「CLI 错误带文件/参数语境」需求负责。**写侧**——`create_dir_all`
与四个 `File::create`/写出步骤产生的**任何** `std::io::Error`——必须经 `anyhow` 语境化（同上语义），这是一条**写侧 catch-all**；写侧**代表性（非穷举）**用例：`--output` 已存在为**普通文件**（非目录，`create_dir_all`
报错）、`--output` 父目录**不可写**、任一输出文件的目标路径**本身是目录或不可写**（`File::create` 报错）、任一文件**写盘失败**（如磁盘满）。（`--input` 是目录/不可读属
**读侧**，由前条覆盖、不在本条。）四个输出文件**按序写、非事务/非原子**（不用 temp+rename）：写到一半被打断（磁盘满 / 进程被杀）**可能留下半写的输出集**（如截断的
`pattern.json`）——这是**有意接受的边界**，靠重跑覆盖恢复；**事务/原子写是 non-goal**（YAGNI，越出 M6；要原子另开 change）。「重跑产逐字节相同」这一保证以**单次运行完成**为前提。

#### 场景:输出路径是已存在的普通文件时非零退出
- **当** `--output` 指向一个已存在的**普通文件**（非目录）运行 `generate`
- **那么** 进程**非零退出**，stderr 含点名该路径与 OS 错误的语境，且**不 panic**（不静默成功、不写坏数据）

### 需求:palette validate 子命令（真实）
`bead-cli palette validate <path>` 必须**真实实现**：读取文件字节后 `load_palette(&bytes)`——`load_palette` **本身已完成完整校验**（非空、hex、唯一
`code`），校验通过则打印成功并退出 0，否则把 `BeadError` 的确定性 `reason` 打印到 stderr 并以非零退出。**读取 `<path>` 的 fs 失败**（不存在 / 是目录 / 不可读）
也必须经 `anyhow` 语境化为**非零退出 + stderr 点名路径与 OS 错误、不 panic**（与 `generate` 读侧同语义、不裸 `?` 丢路径）。**不必再单独调 `validate_palette`**（成功的
`load_palette` 即意味「合法」；再调是冗余的不变量复检，非必需第二阶段）。

#### 场景:合法与非法调色板的退出码
- **当** 对一个合法调色板 JSON 运行 `palette validate`，再对一个非法的运行
- **那么** 前者退出 0，后者退出非 0 且 stderr 含失败原因

### 需求:未实现子命令显式报错而非静默或 panic
`bead-cli palette list` 与 `bead-cli inspect <path>` 在 M6 必须是**显式的「尚未实现」桩**：打印一条「coming soon」类消息到 stderr 并以非零退出码
（1）结束，**禁止**静默成功、**禁止** `panic!`/`unimplemented!`。

#### 场景:桩命令以非零退出并提示未实现
- **当** 运行 `bead-cli palette list` 或 `bead-cli inspect <path>`
- **那么** 进程退出码为 1，stderr 含「coming soon / 尚未实现」类提示，且不 panic

