# flutter-ffi 规范（增量）

## MODIFIED Requirements

### 需求:桥接边界契约(M8 仅 width/height)
`bead-ffi` 必须暴露单一生成入口。输入为:图像字节、调色板 JSON 字符串、目标网格尺寸 `width` / `height`,**以及三个可选项** `max_colors: Option<u32>`、`despeckle: Option<u32>`、`generator`(镜像 `GeneratorKind`,取值 `staged` | `gerstner`,默认 `staged`)。桥接函数必须用调色板 JSON 的字节(`palette_json.as_bytes()`,因为 `load_palette` 接受 `&[u8]`)调 `load_palette`,再以 `GenerateOptions { width, height, max_colors, despeckle, generator, ..Default::default() }` 调 `generate_pattern`。**三项均未设置时(`max_colors = None`、`despeckle = None`、`generator = staged`)该构造必须逐字段等价于旧的 `GenerateOptions { width, height, ..Default::default() }`**,从而输出与放宽前的 FFI、及不带对应旗标的 CLI 默认路径**逐字节相同**——既有闸门不回退。该默认路径包括 filter=**`Triangle`**、cell_size=`10`、shape=`Square`、matcher=`MatcherKind::Oklab`(引擎默认)。**调色板 JSON 必须是 UTF-8 且无 BOM**:带 BOM 的 UTF-8 调色板在 CLI 与 FFI 两侧**均触发 `PaletteParse`**(`serde_json::from_slice` 不跳过 BOM);真正使 CLI/FFI **分叉**的是非 UTF-8 字节(Dart 解码为 U+FFFD 后再编码 ≠ 原始文件字节)。两类均**不在**「CLI == FFI 逐字节相等」保证内(项目自带调色板均为无 BOM 的 UTF-8,已核验逐字节往返一致)。放宽后的边界**只**新开放 `max_colors` / `despeckle` / `generator` 三项——它们恰是 **CLI 已暴露(`--max-colors` / `--despeckle` / `--generator`)且移动端 UI 需要** 的集合,故「CLI == FFI」对它们可测;`generator` 的 FFI 镜像枚举→`bead_core::GeneratorKind` 的映射属平凡 marshalling(与 CLI 的 `From<CliGenerator>` 同性质),不违反「零逻辑薄桥」。边界仍**禁止**暴露 `filter` / `cell_size` / `shape` / `matcher` 作为调用方选项:CLI 对 `filter` / `cell_size` / `shape` 无法表达非默认值,暴露它们会使「CLI == FFI」对非默认输入不可测;`matcher` 虽已由 CLI 暴露给 A/B 验证,但本轮 FFI/mobile 仍只承诺默认 Oklab 路径,避免扩张移动边界。非默认 `--matcher lab|rgb` 的 FFI 对账待后续明确 UI/FFI 需求时再加(届时仍走同一 `generate_pattern`)。输出必须把 `GenerateResult` 完整交回 Dart:pattern 的 `width` / `height` / `cells`(`u16` 数组)、`stats`(`{code, name, count}` 列表)、`summary`、`brand`、`preview_png` 字节、`grid_png` 字节;并必须另外提供 `pattern_json` 字符串。`cells` 与 `stats` 必须以结构化数组(而非 JSON 字符串)跨边界。

#### 场景:一次调用返回全部产物
- **当** Dart 以图像字节、调色板 JSON 字符串、`width`、`height`、以及可选的 `max_colors` / `despeckle` / `generator` 调用桥接函数
- **那么** 返回值必须包含 pattern(width/height/cells)、stats、summary、brand、preview_png、grid_png,以及可独立取得的 pattern_json 字符串

#### 场景:结构化数组而非 JSON 化的 cells
- **当** Dart 取用返回结果中的 `cells` 与 `stats`
- **那么** 它们必须是可直接索引的数组(`cells` 为 `u16` 序列、`stats` 为带 code/name/count 的记录序列),而非需要再次解析的 JSON 字符串

#### 场景:三可选项透传进 GenerateOptions
- **当** 桥接函数收到已设置的 `max_colors` / `despeckle` / `generator`
- **那么** 它必须把三者原样填入 `GenerateOptions` 的同名字段(`generator` 经 FFI 镜像→`bead_core::GeneratorKind` 映射)后调 `generate_pattern`,不得在桥层实现任何减色 / 去斑 / 生成算法

#### 场景:未设置即默认,与旧边界逐字节一致
- **当** 三项均未设置(`max_colors = None`、`despeckle = None`、`generator = staged`)
- **那么** 组装出的 `GenerateOptions` 必须逐字段等价于 `GenerateOptions { width, height, ..Default::default() }`(filter=`Triangle`、cell_size=`10`、shape=`Square`、matcher=`Oklab`),使输出与放宽前逐字节相同;且边界不接受 `filter` / `cell_size` / `shape` / `matcher` 的调用方覆盖值

#### 场景:越界选项值经既有错误扁平化(不对称的 Some(0))
- **当** 调用方传入引擎会拒绝的选项值——`max_colors = Some(0)` 被 `GreedyReducer::new` 拒为 `BeadError::InvalidImage`(注意 `despeckle = Some(0)` 相反,是**合法空操作**,二者不对称)
- **那么** 该 `BeadError` 必须经**既有的**「错误在边界扁平化为单一 Dart 异常」需求抛为 Dart 异常(与 CLI `--max-colors 0` 同样报错,CLI==FFI 仍一致),桥层不新增任何校验、不 panic

### 需求:CLI 与 FFI 同机逐字节相等(决定性闸门)
对于同一组输入(图像字节、调色板、`width`、`height`,以及可选的 `max_colors` / `despeckle` / `generator`),`bead-ffi` 在同一机器上必须产生与**带等价旗标的 `bead-cli generate`**逐字节相同的四样产物(`pattern.json`、`summary.txt`、`preview.png`、`grid.png`):三项未设置时对齐**不传 `--matcher` 的 CLI 默认路径**(filter=`Triangle` + `MatcherKind::Oklab`);设置 `max_colors` / `despeckle` / `generator=gerstner` 时对齐 CLI 的 `--max-colors N` / `--despeckle S` / `--generator gerstner`。`--matcher lab|rgb` 是 CLI/core A/B 能力,**不**属于本轮 FFI 对账输入域。**决定性档位澄清**:`despeckle` 是纯整数路径(多数邻居票选,跨架构位精确);`max_colors` 的减色在**默认 Oklab 匹配器**下复用 matcher 的 **f32 感知度量**(`GreedyReducer` 走 `ColorSnapshot::Perceptual`,非整数路径),与 `generator=gerstner` 同属 f32 **同机 canonical**(非跨目标 byte-exact)。但本闸门是**同机** FFI-vs-CLI 比较,host 同 libm 下 f32 与整数路径**均**逐字节一致,故三者的对账都成立——档位差异只影响「跨目标是否 byte-exact」,不影响本同机闸门。本变更必须提供/保留一个 Dart 测试,复用 M7 的固定输入(`samples/gradient.png` + `palettes/artkal_s.json`),将 FFI 输出与**同机当场运行的** `bead-cli` 输出逐字节比较。测试**必须保留原有两个默认路径尺寸 16×20 与 30×24**(width/height 各异、跨长宽比、off-4:5 者非正方形——理由见下),**并至少新增一个「选项已设置」用例**(如 `max_colors` + `despeckle` 于默认 `staged` 路径),与同机 `bead-cli` 加对应旗标的输出逐字节比较,以证明 `max_colors` / `despeckle` 被转发且未在桥层被改写;`generator` 的转发由下方独立的 `generator=gerstner` 同机对账场景单独证明(不由本 max_colors/despeckle 用例覆盖)。选默认两尺寸是为满足以下必备约束(任何满足约束的尺寸对都成立,但本变更固定取此对):两尺寸的 **width 与 height 都不同、不落在同一长宽比、且至少一个 off-4:5 尺寸取非正方形**(16≠30、20≠24;16×20 是 fixture 的 4:5、30×24 是 5:4 偏离 4:5 且非正方形)。单一尺寸无法证明 `width`/`height` 真被转发;两个**同长宽比**尺寸也不够——fixture 为 32×40(4:5),一个忽略 `height`、按 width×5/4 推导 height 的桥会通过任意两个 4:5 尺寸(**注意 24×30 也恰是 4:5,是个陷阱**),故必须有一个 off-4:5 尺寸;该尺寸取**非正方形**才能单尺寸即抓 width↔height 调换(正方形对调换是盲的)。off-4:5 尺寸同时强制一次真实的非恒等 crop+resize。比较必须按**原始字节**(二进制读取,禁止文本模式/换行符规范化)对**四个具名文件**逐一进行(`fs::create_dir_all` 不清空既有目录,故禁止断言输出目录「为空」,只比这四个文件)。测试还必须**解析**该次返回的 `pattern_json` **字符串**,断言其内编码的 `width` / `height` / `cells` / `stats` / `brand` 与 Dart 经 FRB 收到的结构化字段逐一相等(防止桥层 marshalling 损坏某结构化字段却仍从正确的 Rust 值序列化 JSON 而漏检);**禁止**用结构化数组的再序列化结果与自身比较(否则断言空洞通过)。

#### 场景:Dart 测试证明 CLI == FFI(多尺寸、跨长宽比)
- **当** Dart 测试在**默认两个尺寸 16×20 与 30×24**(width/height 各异、跨长宽比、off-4:5 者非正方形)各以 M7 固定输入、三项均未设置调用桥接函数,并在同机运行不传 `--matcher` 的 `bead-cli generate` 同输入
- **那么** 每个尺寸下两者的 `pattern.json`、`summary.txt`、`preview.png`、`grid.png` 四个具名文件都必须按原始字节逐一相等

#### 场景:max_colors/despeckle 已设置时与 CLI 对应旗标逐字节相等
- **当** Dart 测试以设置了 `max_colors` / `despeckle`(默认 `staged` 路径)的一组固定输入调用桥接函数,并在同机运行带等价 `--max-colors` / `--despeckle` 旗标的 `bead-cli generate` 同输入
- **那么** 两者的 `pattern.json`、`summary.txt`、`preview.png`、`grid.png` 四个具名文件必须按原始字节逐一相等,以证明 `max_colors` / `despeckle` 被转发进 `GenerateOptions` 且桥层未改写

#### 场景:generator 转发经同机 Gerstner 对账
- **当** Dart 测试以 `generator = gerstner`、其余项默认的固定输入调用桥接函数,并在同机运行 `bead-cli generate --generator gerstner` 同输入
- **那么** 两者四个具名文件必须按原始字节逐一相等(f32 同机 canonical),以证明 `generator` 镜像枚举被正确映射并转发进 `GenerateOptions`;此项**不**承诺跨目标 byte-exact(f32 路径,仅 host 同 libm 成立)

#### 场景:结构化字段与 pattern_json 自洽(解析而非再序列化)
- **当** 测试**解析**同次返回的 `pattern_json` 字符串,并取得 Dart 经 FRB 收到的结构化 `width`/`height`/`cells`/`stats`/`brand`
- **那么** 解析出的各字段必须与对应结构化字段逐一相等;测试禁止以结构化数组的再序列化结果与自身比较

#### 场景:对比当场 CLI 输出而非 canonical golden master
- **当** 测试运行在非 canonical 平台(如 macOS arm64 开发机)
- **那么** 测试必须与同机当场运行的 CLI 输出比较(而非 `tests/golden/` 的字节 master),因为同机同 libm 保证默认 **`Triangle`** / `Oklab` 及 `gerstner` 的浮点结果一致、逐字节相等与平台无关

### 需求:移动端交叉编译与 Flutter 装载(iOS 本轮验证,Android 架构就位)
`bead-ffi` 必须能交叉编译为移动端原生库并被 Flutter App 在运行时装载,以兑现 ROADMAP
「CLI == FFI / 同一引擎跨平台」目标。**本里程碑必须交付并验证 iOS**:把 `bead-core`→`bead-ffi`
交叉编译为 iOS 原生库(arm64 真机 + 模拟器),产出 Flutter 可链接/装载的工件(framework 或
静态库 + `staticlib` crate-type),并使 `bead_ffi` Dart 包在 **Flutter 运行时**下正确装载动态/静态库
(M8 的纯 Dart 装载路径只适用于桌面 host)。**Android** 必须按同一架构就位——arm64-v8a /
armeabi-v7a / x86_64 三 ABI 的 jniLibs 交叉编译路径必须在构建配置中表达,但其**本机验证**允许推迟到
Android SDK + NDK 就绪(本变更不强制本轮跑通 Android 真机/模拟器)。

桥的 **Rust 桥逻辑(零算法)与打包/装载维度必须不受本次边界放宽影响**:边界入参现为 `width` / `height` **加** `max_colors` / `despeckle` / `generator` 三可选项(三者未设置即退回旧默认,输出逐字节不变),`GenerateOutput` 字段集合不增减,`filter` / `cell_size` / `shape` / `matcher` 选项档位继续**不**暴露(移动端 UI 不需要,且暴露会破坏「CLI == FFI」对非默认输入的可测性)。本需求只覆盖「打包 / 装载」维度,不改「桥只调用既有公共入口」「错误扁平化」「结构化数组而非 JSON」等既有契约。`bead-core` 仍禁止因移动端打包或本次边界放宽引入任何 UI / 文件系统 / Flutter / 平台依赖。

#### 场景:iOS 交叉编译并被 Flutter 装载
- **当** 构建移动端工件
- **那么** 必须把 `bead-ffi` 交叉编译为 iOS arm64(真机)与模拟器原生库,产出 Flutter 可链接的工件,
  且 Flutter App 在 **iOS 模拟器**上能装载该库并成功调用 `generate`(**硬验收**);真机运行需个人开发团队
  签名,属 best-effort(与 mobile-app「满足 INIT 成功标准」需求一致)

#### 场景:Android 架构就位但验证可推迟
- **当** 实施本变更
- **那么** 构建配置必须表达 Android arm64-v8a / armeabi-v7a / x86_64 三 ABI 的 jniLibs 交叉编译路径,
  但允许在 Android SDK/NDK 未就绪时不强制本轮跑通 Android 端验证(留作收尾)

#### 场景:打包维度与桥零算法不受边界放宽影响
- **当** 为移动端打包做改动,或为本次放宽新增 `max_colors` / `despeckle` / `generator` 入参
- **那么** `bead-ffi` 的 Rust 桥必须保持零算法(仅透传三项进 `GenerateOptions` 并调既有入口)、`GenerateOutput` 字段集合不变、打包/装载路径不变,且禁止借机暴露 `filter` / `cell_size` / `shape` / `matcher` 或修改 `bead-core`

#### 场景:依赖隔离(承接自原 host-only 需求)
- **当** 引入移动端交叉编译与 Flutter 集成所需的依赖
- **那么** Rust 侧新增依赖只能出现在 `crates/bead-ffi`,`bead-core` 与 `bead-cli` 的依赖集必须不变;
  Flutter 端依赖(`image_picker` / `crop_your_image` / `flutter_riverpod` / `go_router` 等)只能出现在
  `apps/mobile`,禁止污染任何 crate

#### 场景:iOS 与 host CLI 跨目标不保证浮点逐字节相等(决定性边界)
- **当** 在 iOS(模拟器/真机)上运行 App 并与 host `bead-cli` 比较同输入的产出
- **那么** 禁止要求浮点路径派生物(`cells` / `stats` / `summary` / `preview` / `grid`)逐字节/逐值等同
  CLI——iOS 是不同编译目标、不同 libm,ARCHITECTURE Rule 3 仅保证整数路径跨目标一致;byte-exact
  「CLI == FFI」闸门**仅在 host 同 libm 成立**(M8 既有 Dart 决定性测试,不因本里程碑回退),iOS 端只验证
  结构不变量(生成成功、`pattern.cells` 长度 = width×height〔即总豆数,由 `pattern.cells.length` / `Σ stats.count`
  派生;DTO 无 `total_beads` 字段〕、stats schema、summary 格式)
