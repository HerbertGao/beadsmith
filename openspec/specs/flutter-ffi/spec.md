# flutter-ffi 规范

## 目的
定义 `bead-ffi` 薄桥的契约——`bead-core` 到 Dart 的**零逻辑桥**，让 Flutter 端调用与 `bead-cli`
**逐字节相同**的引擎（ROADMAP「CLI == FFI」核心闸门）。规定边界类型（入/出/错误映射）、「无业务逻辑、
唯一生成入口 `pipeline::generate_pattern`」约束（CLAUDE 规则 4）、iOS 交叉编译与 Flutter 运行时装载的打包
范围（桥逻辑零改动，仅新增 `staticlib` crate-type，以 FRB `ExternalLibrary.process()` 装载），以及
「CLI == FFI 同机逐字节相等」的决定性验收。
## 需求
### 需求:bead-ffi 是 generate_pattern 的零逻辑薄桥

`crates/bead-ffi` 必须是 `bead-core` 到 Dart 的薄桥，禁止包含任何算法或业务逻辑。
它对外只能调用 `bead-core` 的既有公共 API:`load_palette`、`pipeline::generate_pattern`、
`pipeline::pattern_json`；禁止触达 pipeline 的内部阶段(`image_to_grid` / matcher /
统计 / 渲染)或在桥层重新编排生成流程(CLAUDE 规则 4)。`bead-core` 禁止因 FFI 引入
任何 UI / 文件系统 / Flutter / 平台依赖；`BeadPattern` / `ColorStat` 已派生 `Clone`,
FFI 跨边界镜像必须在 `bead-ffi` 侧完成(FRB `mirror` 或本地包装),禁止为 FFI 便利而
修改 `bead-core` 的数据模型。

#### 场景:桥只调用既有公共入口
- **当** `bead-ffi` 的桥接函数处理一次生成请求
- **那么** 它必须依次调用 `load_palette` 与 `generate_pattern`(必要时 `pattern_json`),
  禁止自行实现 image→match→stats→render 中的任何一步

#### 场景:core 不被 FFI 污染
- **当** 为支持 FFI 实现桥接
- **那么** `bead-core` 必须保持零改动(`BeadPattern` / `ColorStat` 已派生 `Clone`),
  跨边界镜像在 `bead-ffi` 侧完成；禁止在 core 内出现 Flutter / 平台 / 文件系统 / FFI
  运行时依赖,也禁止把「为何 Clone」之类的下游消费者语境写进 core

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

### 需求:错误在边界扁平化为单一 Dart 异常

`generate_pattern` / `load_palette` 返回的 `BeadError` 必须在 `bead-ffi` 边界被扁平化为
其可读文案(`Display` 字符串)后交给 FRB 抛成 Dart 异常。桥接函数**禁止**把结构化的
`BeadError` 直接跨边界返回:其 `PaletteParse(serde_json::Error)` / `ImageDecode(::image::
ImageError)`(元组变体)与 `ImageEncode { source: ::image::ImageError }`(具名字段变体)
都包裹外部错误负载,FRB 无法跨边界传递这些外部类型。Dart 端只看到一条消息字符串,不
穷举 `BeadError` 变体(`#[non_exhaustive]`)。本需求必须覆盖以下失败输入的既定行为:
无法解码或为空的图像字节 → `ImageDecode`(注意二者共用同一恒定 `Display`「failed to
decode image」,Dart 无法仅凭消息区分空输入与损坏输入);JSON 语法错误的调色板 →
`PaletteParse`;**语义非法**的调色板(零颜色 / 重复 code / 非法 hex)→ `InvalidPalette`
(与 `PaletteParse` 不同变体,同属「坏调色板」用户意图);会让维度为零的 `width`/`height`
→ `InvalidImage`。**已知限制**:引擎对超大 `width × height` 无上限守卫,M8 薄桥不新增
上限(属引擎改动、超薄桥范围),继承引擎现状——极端维度下 `total_beads` 的 `as u32`
截断可能先于 OOM 产生「字节一致但语义错」的 `pattern.json`(CLI 与 FFI 同样截断,闸门
仍过),属记录在案的引擎现状。

#### 场景:非法输入抛出带消息的异常
- **当** Dart 传入无法解码或为空的图像字节、非法调色板 JSON、零颜色调色板,或会让维度
  为零的 `width`/`height`
- **那么** 桥接调用必须使 Dart 端抛出异常,异常消息为对应 `BeadError` 变体的 `Display`
  文案(已在边界扁平化为字符串),且不要求 Dart 按变体分支处理

#### 场景:外部错误负载不跨边界
- **当** 失败来自包裹外部负载的变体(`PaletteParse` / `ImageDecode` / `ImageEncode`)
- **那么** 桥接必须先取其 `Display` 字符串再跨边界,禁止把 `serde_json::Error` /
  `::image::ImageError` 等外部类型结构化地交给 FRB

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

### 需求:移动端交叉编译与 Flutter 装载(iOS 本轮验证,Android 架构就位)

`bead-ffi` 必须能交叉编译为移动端原生库并被 Flutter App 在运行时装载,以兑现 ROADMAP
「CLI == FFI / 同一引擎跨平台」目标。**本里程碑必须交付并验证 iOS**:把 `bead-core`→`bead-ffi`
交叉编译为 iOS 原生库(arm64 真机 + 模拟器),产出 Flutter 可链接/装载的工件(framework 或
静态库 + `staticlib` crate-type),并使 `bead_ffi` Dart 包在 **Flutter 运行时**下正确装载动态/静态库
(M8 的纯 Dart 装载路径只适用于桌面 host)。**Android** 必须按同一架构就位——arm64-v8a /
armeabi-v7a / x86_64 三 ABI 的 jniLibs 交叉编译路径必须在构建配置中表达,但其**本机验证**允许推迟到
Android SDK + NDK 就绪(本变更不强制本轮跑通 Android 真机/模拟器)。

桥的 **Rust 逻辑与边界类型必须零改动**:仍只接受 `width` / `height`,`GenerateOutput` 字段不增减,
`filter` / `cell_size` / `shape` 选项档位继续不暴露(本里程碑 UI 不需要,且暴露会破坏「CLI == FFI」
对非默认输入的可测性)。本需求只新增「打包 / 装载」维度,不改「桥只调用既有公共入口」「错误扁平化」
「结构化数组而非 JSON」等既有契约。`bead-core` 仍禁止因移动端打包引入任何 UI / 文件系统 / Flutter /
平台依赖。

#### 场景:iOS 交叉编译并被 Flutter 装载
- **当** 构建移动端工件
- **那么** 必须把 `bead-ffi` 交叉编译为 iOS arm64(真机)与模拟器原生库,产出 Flutter 可链接的工件,
  且 Flutter App 在 **iOS 模拟器**上能装载该库并成功调用 `generate`(**硬验收**);真机运行需个人开发团队
  签名,属 best-effort(与 mobile-app「满足 INIT 成功标准」需求一致)

#### 场景:Android 架构就位但验证可推迟
- **当** 实施本变更
- **那么** 构建配置必须表达 Android arm64-v8a / armeabi-v7a / x86_64 三 ABI 的 jniLibs 交叉编译路径,
  但允许在 Android SDK/NDK 未就绪时不强制本轮跑通 Android 端验证(留作收尾)

#### 场景:边界与桥逻辑零改动
- **当** 为移动端打包做改动
- **那么** `bead-ffi` 的 Rust 桥逻辑、边界入参(仍只 width/height)、`GenerateOutput` 字段集合
  必须保持不变,禁止借机暴露 `filter` / `cell_size` / `shape` 或修改 `bead-core`

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

