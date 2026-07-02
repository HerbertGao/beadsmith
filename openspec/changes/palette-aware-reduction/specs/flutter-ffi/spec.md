# flutter-ffi 规范（增量）

## MODIFIED Requirements

### 需求:桥接边界契约(M8 仅 width/height)
`bead-ffi` 必须暴露单一生成入口。输入为:图像字节、调色板 JSON 字符串、以及目标网格尺寸 `width` / `height`。桥接函数必须用调色板 JSON 的字节(`palette_json.as_bytes()`,因为 `load_palette` 接受 `&[u8]`)调 `load_palette`,再以 `GenerateOptions { width, height, ..Default::default() }` 调 `generate_pattern`——**与不传 `--matcher` 的 CLI 默认路径完全相同的构造方式**。该默认路径现在包括 filter=**`Triangle`**、cell_size=`10`、shape=`Square`、matcher=`MatcherKind::Oklab`（引擎默认）。**调色板 JSON 必须是 UTF-8 且无 BOM**:带 BOM 的 UTF-8 调色板在 CLI 与 FFI 两侧**均触发 `PaletteParse`**(`serde_json::from_slice` 不跳过 BOM);真正使 CLI/FFI **分叉**的是非 UTF-8 字节(Dart 解码为 U+FFFD 后再编码 ≠ 原始文件字节)。两类均**不在**「CLI == FFI 逐字节相等」保证内(项目自带调色板均为无 BOM 的 UTF-8,已核验逐字节往返一致)。M8 边界**禁止**暴露 `filter` / `cell_size` / `shape` / `matcher` 作为调用方选项:CLI 对 `filter` / `cell_size` / `shape` 无法表达非默认值,暴露它们会使「CLI == FFI」对非默认输入不可测;`matcher` 虽已由 CLI 暴露给 A/B 验证，但本轮 FFI/mobile 仍只承诺默认 Oklab 路径，避免扩张移动边界。非默认 `--matcher lab|rgb` 的 FFI 对账待后续明确 UI/FFI 需求时再加(届时仍走同一 `generate_pattern`)。输出必须把 `GenerateResult` 完整交回 Dart:pattern 的 `width` / `height` / `cells`(`u16` 数组)、`stats`(`{code, name, count}` 列表)、`summary`、`brand`、`preview_png` 字节、`grid_png` 字节;并必须另外提供 `pattern_json` 字符串。`cells` 与 `stats` 必须以结构化数组(而非 JSON 字符串)跨边界。

#### 场景:一次调用返回全部产物
- **当** Dart 以图像字节、调色板 JSON 字符串、`width`、`height` 调用桥接函数
- **那么** 返回值必须包含 pattern(width/height/cells)、stats、summary、brand、preview_png、grid_png,以及可独立取得的 pattern_json 字符串

#### 场景:结构化数组而非 JSON 化的 cells
- **当** Dart 取用返回结果中的 `cells` 与 `stats`
- **那么** 它们必须是可直接索引的数组(`cells` 为 `u16` 序列、`stats` 为带 code/name/count 的记录序列),而非需要再次解析的 JSON 字符串

#### 场景:选项构造与 CLI 默认一致
- **当** 桥接函数组装 `GenerateOptions`
- **那么** 它必须等价于 CLI 不传 `--matcher` 时的 `GenerateOptions { width, height, ..Default::default() }`,即 filter 为 **`Triangle`**、cell_size 为 `10`、shape 为 `Square`、matcher 为 `Oklab`(引擎默认),且 M8 边界不接受这四者的调用方覆盖值

### 需求:CLI 与 FFI 同机逐字节相等(决定性闸门)
对于同一组输入(图像字节、调色板、`width`、`height`),`bead-ffi` 在同一机器上必须产生与**不传 `--matcher` 的 `bead-cli generate` 默认路径**逐字节相同的四样产物(`pattern.json`、`summary.txt`、`preview.png`、`grid.png`)。默认路径使用 filter=`Triangle` + `MatcherKind::Oklab`；`--matcher lab|rgb` 是 CLI/core A/B 能力,**不**属于本轮 FFI 对账输入域。本变更必须提供/保留一个 Dart 测试,复用 M7 的固定输入(`samples/gradient.png` + `palettes/artkal_s.json`),将 FFI 输出与**同机当场运行的** `bead-cli` 默认输出逐字节比较。测试**规定采用 16×20 与 30×24 两个尺寸**各跑一遍。选这两个尺寸是为满足以下必备约束(任何满足约束的尺寸对都成立,但本变更固定取此对):两尺寸的 **width 与 height 都不同、不落在同一长宽比、且至少一个 off-4:5 尺寸取非正方形**(16≠30、20≠24;16×20 是 fixture 的 4:5、30×24 是 5:4 偏离 4:5 且非正方形)。单一尺寸无法证明 `width`/`height` 真被转发;两个**同长宽比**尺寸也不够——fixture 为 32×40(4:5),一个忽略 `height`、按 width×5/4 推导 height 的桥会通过任意两个 4:5 尺寸(**注意 24×30 也恰是 4:5,是个陷阱**),故必须有一个 off-4:5 尺寸;该尺寸取**非正方形**才能单尺寸即抓 width↔height 调换(正方形对调换是盲的)。off-4:5 尺寸同时强制一次真实的非恒等 crop+resize。比较必须按**原始字节**(二进制读取,禁止文本模式/换行符规范化)对**四个具名文件**逐一进行(`fs::create_dir_all` 不清空既有目录,故禁止断言输出目录「为空」,只比这四个文件)。测试还必须**解析**该次返回的 `pattern_json` **字符串**,断言其内编码的 `width` / `height` / `cells` / `stats` / `brand` 与 Dart 经 FRB 收到的结构化字段逐一相等(防止桥层 marshalling 损坏某结构化字段却仍从正确的 Rust 值序列化 JSON 而漏检);**禁止**用结构化数组的再序列化结果与自身比较(否则断言空洞通过)。

#### 场景:Dart 测试证明 CLI == FFI(多尺寸、跨长宽比)
- **当** Dart 测试在**规定的两个尺寸 16×20 与 30×24**(width/height 各异、跨长宽比、off-4:5 者非正方形)各以 M7 固定输入调用桥接函数,并在同机运行不传 `--matcher` 的 `bead-cli generate` 同输入
- **那么** 每个尺寸下两者的 `pattern.json`、`summary.txt`、`preview.png`、`grid.png` 四个具名文件都必须按原始字节逐一相等

#### 场景:结构化字段与 pattern_json 自洽(解析而非再序列化)
- **当** 测试**解析**同次返回的 `pattern_json` 字符串,并取得 Dart 经 FRB 收到的结构化 `width`/`height`/`cells`/`stats`/`brand`
- **那么** 解析出的各字段必须与对应结构化字段逐一相等;测试禁止以结构化数组的再序列化结果与自身比较

#### 场景:对比当场 CLI 默认输出而非 canonical golden master
- **当** 测试运行在非 canonical 平台(如 macOS arm64 开发机)
- **那么** 测试必须与同机当场运行的 CLI 默认输出比较(而非 `tests/golden/` 的字节 master),因为同机同 libm 保证默认 **`Triangle`**（及默认 `OklabMatcher`）浮点结果一致、逐字节相等与平台无关
