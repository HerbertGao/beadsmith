## 上下文

M7 用 golden 测试把 Phase-1 引擎冻结，`bead-core` 现在可信但只有 CLI 一个消费者。
M8 是 ROADMAP 上「Rust 核心」到「Flutter 移动」的唯一桥：新建 `crates/bead-ffi`，
把 `pipeline::generate_pattern` 暴露给 Dart，且**逐字节复刻** CLI 的输出。

约束（来自 ARCHITECTURE.md 五条硬规则 + CLAUDE.md）：
- `bead-core` 不感知 Flutter/平台/文件系统——FFI 相关代码全在 `bead-ffi`，core **零改动**。
- `bead-ffi` **零业务逻辑**：只过 `load_palette` + `generate_pattern` + `pattern_json`
  三个既有公共 API，不触达内部 pipeline 阶段（规则 4）。
- 确定性是闸门：FFI 与 CLI 同机同 target 编译、共用 `Cargo.lock` 与同一
  `generate_pattern` → 同输入逐字节同输出。

已知边界类型（M7 实测，已对照真实代码核验）：
- 入：`image_bytes: &[u8]`、`palette: &Palette`（由 `load_palette(bytes: &[u8]) ->
  Result<Palette, BeadError>` 得到——**接受字节，非字符串**）、`GenerateOptions { width:
  u32, height: u32, resize: ResizeOptions { filter }, render: RenderOptions { cell_size,
  shape } }`。
- `ResizeOptions.filter` 是**外部 `image` crate** 的 `::image::imageops::FilterType`
  （注意与渲染器用的 `::image::codecs::png::FilterType` 不是同一类型）；`render.shape`
  是本地 `#[non_exhaustive] BeadShape`（仅 `Square`）。**两者都不是 FRB 能直接镜像的
  类型**——这是 D-Entry 选择最小边界的直接原因。
- 出：`GenerateResult { pattern: BeadPattern{width,height,cells:Vec<u16>}, stats:
  Vec<ColorStat{code,name,count}>, summary: String, brand: String, preview_png: Vec<u8>,
  grid_png: Vec<u8> }`，外加 `pattern_json(&result) -> String`。`BeadPattern` / `ColorStat`
  **已派生 `Clone`**；`GenerateResult` 刻意不派生 `Clone`。
- 错误：`BeadError`（`#[non_exhaustive]` enum）；`PaletteParse` / `ImageDecode` /
  `ImageEncode` 变体**包裹外部错误负载**（`serde_json::Error` / `::image::ImageError`）。
- CLI 实测：`generate` 子命令**只有** `--input/--palette/--width/--height/--output`，硬编码
  `GenerateOptions { width, height, ..Default::default() }`，**无** filter/cell_size/shape
  标志。四样产物均为 `result` 字段的逐字节透传（`pattern.json` 无尾换行，`summary.txt`
  的尾换行来自 core 的 `generate_summary`）。

## 目标 / 非目标

**目标：**
- `crates/bead-ffi`：单一桥接函数 + `flutter_rust_bridge`（FRB）codegen 出 Dart glue。
- Dart 侧拿到结构化结果（pattern/stats/summary/brand + 两块 PNG bytes + pattern_json）。
- 一个 Dart 测试过「CLI == FFI 同机逐字节相等」闸门，复用 M7 固定输入。
- host 动态库（macOS `.dylib` / Linux `.so`）能被 Dart 加载。

**非目标：**
- iOS/Android 交叉编译、XCFramework/jniLibs 打包、签名（→ M9 开头）。
- Flutter App / UI / riverpod / go_router（→ M9）。
- M8 边界暴露 filter/cell_size/shape 选项（→ M9 真有 UI 需要时加，仍走 generate_pattern）。
- 新算法、配色档位变更、palette 句柄缓存、通用 C ABI/WASM 消费者、给引擎加维度上限。

## 决策

### D-Scope — M8 = host-only，交叉编译并入 M9

`bead-ffi` 桥接 + FRB codegen + host 动态库 + Dart 单测过决定性闸门即算 M8 完成。
iOS arm64 / Android 三 ABI 的交叉编译与打包推到 M9 开头。

- **替代方案**：A — 按 ROADMAP 字面，M8 背上全平台交叉编译 + XCFramework/jniLibs。
- **否决理由**：交叉编译的天然验证手段是「真机加载 `.so`/`.a` 并跑通」，而 M8 阶段
  没有任何 App 能加载库——会在没有反馈回路的情况下调 NDK/签名，烧时间且易留隐藏 bug。
  ROADMAP done-when 原文「a Dart unit test … gets the same result the CLI produces」
  并不要求真机，host 单测即满足。把交叉编译与「真 App 加载验证」绑到 M9 开头，反馈
  回路最短。M8 的价值（桥接正确 + 决定性闸门）host 上即可完全证明。
- **linkage 注记**：M8 的 `crate-type` 只声明 host 实际构建的 `["cdylib", "lib"]`，**不**
  预声明 `staticlib`——声明一个本里程碑不构建不验证的产物是 YAGNI；真机静态库的 linkage
  由 M9 连同真机加载一起验证。

### D-Entry — 单一桥接函数，最小边界（仅 width/height），palette 传 JSON 字符串

桥只暴露一个 `generate`：入 `image_bytes` + `palette_json: String` + `width` + `height`，
内部 `load_palette(palette_json.as_bytes())?`（注意 `load_palette` 接受 `&[u8]`，故对
`String` 取 `.as_bytes()`）→ 以 `GenerateOptions { width, height, ..Default::default() }`
（**与 CLI 完全相同的构造**）→ `generate_pattern(..)?` → 回交结果。

- **M8 边界禁止暴露 filter/cell_size/shape**。三条互锁理由：
  1. **CLI 是契约（规则 5）且无法表达非默认值**：CLI 只接受 width/height、硬编码
     `..Default::default()`。一旦 FFI 接受非默认 filter/cell_size/shape，就产出 CLI 根本
     无法产出的输出——「CLI == FFI」对这些输入**不可测**（不是没测、是测不了）。
  2. **filter/shape 不是 FRB 能镜像的类型**：`filter` 是外部 `::image::imageops::FilterType`
     （FRB 不镜像第三方 crate 的枚举），`shape` 是 `#[non_exhaustive] BeadShape`（外部
     无法穷举构造/匹配）。暴露它们必然要在 `bead-ffi` 手写本地枚举 + `From` 转换——而手写
     映射正是「写错→静默偏离 Lanczos3 默认」的危险点，且 default-only 的闸门测试抓不到非
     默认值的误映射。
  3. **YAGNI**：M9 真有 UI 让用户选 filter/shape 时再加（届时同样走 `generate_pattern`，
     并补一个对 Rust 内 oracle 而非 CLI 的非默认映射测试）。
  把边界收成 width/height，一举消解上述 FRB 镜像难题与测试盲区，且严格少写代码。
- **替代方案**：①暴露 `load_palette` 得 palette 句柄、`generate` 复用句柄避免重解析；
  ②把 filter/shape 当本地 DTO 枚举暴露给 Dart。
- **否决理由**：①重解析几百色 = 微秒级，YAGNI，且引入跨边界状态——profiling 说话再加。
  ②即上面三条理由：CLI 测不了 + FRB 镜像成本 + 当前无 UI 需求。
- **palette 句柄的前向触发器（记录而非现在做）**：若 M9 的 GeneratePage 让用户反复调
  width/height 重生成、profiling 显示 palette 重解析成热点，则**新增**一个
  `load_palette_handle` 入口与 `generate` 并存（加法，不改 `generate` 签名），而非把句柄
  塞进现有签名——故此处的 YAGNI 是有界、可逆、有命名触发点的延迟，不是隐性赌注。

### D-Errors — BeadError 在边界扁平化为 Display 字符串

桥接函数内部拿到 `Result<_, BeadError>` 后，**在边界把 `Err` 扁平化为其 `Display` 字符串**
再交给 FRB 抛 Dart 异常。Dart 侧只见一条消息，不穷举 variant。

- **为何必须扁平化（不是可选）**：`PaletteParse(serde_json::Error)` / `ImageDecode(::image::
  ImageError)`（元组变体）与 `ImageEncode { source: ::image::ImageError }`（**具名字段**变体，
  非元组——`lib.rs` 故意不给它 `#[from]`）都包裹外部负载。FRB **无法**把这些外部类型结构化
  地送到 Dart；若试图结构化返回整个 `BeadError`，codegen 会在这些外部负载上失败。故桥接
  必须先 `to_string()`（用 `Display`）再跨边界——`to_string()` 对元组/具名变体一致，不依赖
  变体形状，故扁平化天然规避了具名-vs-元组的差异。
- **错误变体的既定映射（供 D-Test 的 Rust 侧测试断言）**：JSON 语法错 → `PaletteParse`；
  **语义非法**调色板（零颜色 / 重复 code / 非法 hex）→ `InvalidPalette`（与 `PaletteParse`
  不同变体）；无法解码或空图像 → `ImageDecode`（二者共用恒定 Display，不可凭消息区分）；
  零维度 → `InvalidImage`。
- **替代方案**：把 `BeadError` 各 variant 映射成 Dart sealed class / 错误码枚举。
- **否决理由**：`BeadError` 是 `#[non_exhaustive]`，Dart 端穷举会随 core 加 variant 而
  脆裂；且外部负载无法跨边界。M9 的 App 对失败只需展示可读 message。
- **已知限制**：`PaletteParse` / `ImageEncode` 的 `Display` 较粗（不含底层 serde/image 细节），
  故 Dart 用户看到的消息较笼统——M8 接受（改 Display 属 core 改动、超范围）。

### D-Output — cells/stats 原样过 FRB，pattern_json 另行暴露

`cells: Vec<u16>`、`stats: Vec<ColorStat>` 以结构化数组过 FRB 给 Dart（App 要直接用），
**不**在边界把 cells JSON 化。另把 `pattern_json(&result) -> String` 暴露给 Dart 供 M9
落盘 `pattern.json`——Dart 侧禁止自己拼 JSON（会偏离 CLI 合约/规则 5）。

- **替代方案**：边界只回一个 `pattern_json` 字符串，Dart 自己 parse。
- **否决理由**：App 渲染预览/统计需要结构化的 cells/stats 数组，让 Dart 再 parse 一遍
  JSON 是多余往返；落盘场景又需与 CLI 逐字节一致的 JSON，故两者都给。**自洽要求**：闸门
  测试必须断言结构化 `cells` == `pattern_json` 内的 `cells`（防桥层损坏数组却从原始数据
  序列化 JSON 而漏检，见 D-Test）。

### D-CoreDerive — core 零改动，FRB 镜像在 bead-ffi 侧

`BeadPattern` / `ColorStat` **已派生 `Clone`**（核验 `models/mod.rs`），故 core 无需为 FFI
加任何 derive——M8 对 `bead-core` 是零改动。FRB 对这两个类型的镜像在 `bead-ffi` 侧用 FRB
的 `mirror` 机制（或本地包装）完成，core 不被 FFI 注解污染（规则 1）。`GenerateResult`
刻意不 `Clone`：桥接按字段**移动**取出（owned 值析构即可），不给它加 `Clone`。

- **替代方案**：在 `bead-ffi` 内定义一套 DTO 镜像 + `From` 转换。
- **否决理由**：DTO 镜像是「薄桥」里最易腐化的样板（每加字段两处改）。优先 FRB `mirror`；
  仅当 FRB 对某类型确实无法镜像时才退回最小 DTO（filter/shape 已因最小边界不跨界，不存在
  此问题）。**待 apply 时确认**：FRB 能否镜像并从非 `Clone` 的 `GenerateResult` 按字段移动
  取出——若 FRB 的 mirror/opaque 机制对非 `Clone` owner 有要求，在 `bead-ffi` 侧包装解决，
  仍不碰 core。

### D-Test — 决定性闸门 = 同机对比当场跑的 CLI 输出（原始字节 + 自洽）

Dart 单测：以 M7 固定输入（`samples/gradient.png` + `palettes/artkal_s.json`），在**至少
两个尺寸**各跑 FFI，再同机当场跑一次 `bead-cli generate` 同输入。两尺寸用 **16×20 与 30×24**：
- 两者的 width 与 height **都不同**（16≠30、20≠24），且**不落在同一长宽比**——16×20 是
  fixture 的 4:5 线，30×24 是 5:4、**偏离 4:5**。这道关键约束的理由：fixture 是 32×40（4:5），
  若第二尺寸也取 4:5（如 32×40 或 **24×30**——注意 24×30 = 4:5，是个陷阱），一个**忽略
  `height`、按 width×5/4 推导 height** 的桥会同时通过两个尺寸——「证明 width/height 各自被
  转发」就**空洞**了。取一个 off-4:5 尺寸才能让这种比例推导桥露馅。
- **30×24 取非正方形**（非 24×24 那种正方形）是有意的：正方形第二尺寸对 **width↔height 调换**
  bug 是盲的（调换后仍 24×24），swap 检测就**只**压在 16×20 上、且无测试守住这层耦合；30×24
  非正方形，调换后变 24×30 ≠ 30×24，**单尺寸即抓 swap + 比例推导 + 方形坍缩**，消除隐藏耦合。
- 30×24 还**强制一次真实的非恒等 crop+resize**（32×40 先 center-crop 到 32×25、再 Lanczos3
  降到 30×24）；而 32×40 恰等于源尺寸、resize 是恒等透传（0 像素变化），不增 resize 覆盖。

1. 按**原始字节**（二进制读取、无换行规范化）逐一比较**四个具名文件**
   `pattern.json`/`summary.txt`/`preview.png`/`grid.png`——不断言输出目录「为空」
   （`create_dir_all` 不清既有目录）；Dart 侧写 `summary` 落盘时**禁止**追加尾换行（CLI 写
   `result.summary` 原样，尾换行来自 core）。
2. **解析**该次 `pattern_json` **字符串**，断言其 `width`/`height`/`cells`/`stats`/`brand`
   与 Dart 经 FRB 收到的结构化字段逐一相等——不只 `cells`，且 JSON 侧值必须来自**解析字符串**
   而非结构化数组的再序列化（否则断言空洞通过）。`pattern_json` 还含 `total` 键——`total`
   (=cells.len()) 与 `summary` **不**在本自洽检查内（字段表非穷举），它们已由步骤 1 的四文件
   逐字节比较传递覆盖。
3. 错误路径**不**进同机闸门，由 Rust 侧两层测试覆盖（见 D-Errors / tasks 3.3）：①私有内层
   测试断言各失败输入返回对应 `BeadError` 变体；②导出边界测试断言抛给 Dart 的消息 ==
   `err.to_string()`——两者是不同契约，分开测。

- **替代方案**：直接断言 FFI 输出 == `tests/golden/` 里的 byte master。
- **否决理由**：golden master 仅在 canonical（arm64 Linux）平台逐字节有效；开发机
  （macOS arm64）上比 master 会退化成结构不变量。而「对比当场同机 CLI 输出」在**任意**
  host 都成立——FFI 与 CLI 同机同 libm，`Lanczos3` 的 `f32::sin` 一致，逐字节相等与平台
  无关（M7 golden README 已记同设备同 libm 这一前提）。这更贴合「CLI 是契约」，且不要求
  测试机是 canonical。
- **覆盖边界（诚实记录）**：闸门走 happy path，在 **≥2 个尺寸**（16×20 + off-4:5 的 30×24）
  上跑——两尺寸 width/height 各异且跨长宽比，证明 width 与 height 各自被桥转发（非正方形、
  off-4:5 的 30×24 单尺寸即抓 swap + 按比例推导 + 方形坍缩，并强制一次非恒等 crop+resize）。
  因 M8 边界已收成
  width/height（filter/cell_size/shape 不可由调用方覆盖、恒等于 CLI 默认），这三者「被诚实
  采用 vs 被硬编码默认」的歧义已结构性消解，M9 加选项时再补对 Rust oracle 的映射测试。错误
  路径由 D-Errors 的 Rust 侧 `#[test]` 覆盖，不进同机闸门。

## 风险 / 权衡

- **FRB 版本/codegen 漂移** → 在 `bead-ffi` 的 Cargo 钉 `flutter_rust_bridge` 版本，并
  **显式钉 `flutter_rust_bridge_codegen` 工具版本**（lockfile 或 CI 固定安装，非仅运行时
  crate）；codegen 产物（Dart glue）入库，CI 校验「重跑 codegen 无 diff」，且校验前
  **规范化格式**（生成的 Dart 对 formatter 版本敏感，否则会 formatter 版本差异导致假 diff
  或掩盖真 diff）——使该守卫检验「签名一致」而非「formatter 版本一致」。
- **边界默认与 CLI 默认分叉** → M8 边界已不暴露 filter/cell_size/shape，桥接与 CLI 同走
  `..Default::default()`，从结构上消除分叉；闸门测试对比当场 CLI 输出兜底。
- **host-only 把交叉编译风险后移到 M9** → 接受。M9 开头第一件事就是交叉编译 + 真机加载，
  届时有 App 可立即验证；M8 桥接契约与决定性已 host 证明，交叉编译只换工具链不换逻辑。
  tasks 显式标注「交叉编译 = M9 范围」。
- **超大维度无上限** → 引擎对 `width × height` 无上限守卫，超大值可能在产生 PNG 前 OOM；
  更细一层，`total_beads` 的 `as u32` 截断在极端维度下会先于 OOM 产生「字节一致但语义错」
  的 `pattern.json`（CLI/FFI 同样截断，闸门仍过）。M8 薄桥**不**新增上限（属引擎改动、超
  薄桥范围），继承引擎现状；作为已知限制记录，M9/后续若需可在引擎层加守卫。
- **Dart 测试需要能跑 CLI / 加载动态库** → 测试假定 workspace 已 `cargo build` 出
  `bead-cli` 与 `bead-ffi` host 库；CI 在跑 Dart 测试前先 build 二者。host 工具链缺失则
  跳过并 `log`，不静默假绿。

## 开放问题（已在本轮 review 中定）

- **FRB 集成形态与 Dart 测试位置**：M8 产出**最小但真实**的 Dart 包于
  `crates/bead-ffi/dart/`（FRB Dart glue 落在一个稳定路径，M9 的 `infrastructure/
  PatternEngine` 直接 import 扩展它），**不**建 `apps/mobile` 骨架。决定性闸门测试针对**与
  M9 相同的 import 表面**运行，使闸门跨里程碑连续，而非到 M9 重建包结构。闸门测试 import
  的稳定入口符号（generate 函数 + 结果类型）即「M9 复用的同一 import 表面」的被检查契约
  ——后续 regen/refactor 改名会让闸门测试编译失败，故连续性由测试强制而非靠约定。

## M9 交接清单（归档时随变更带入 M9 提案的进入条件）

本变更把若干顾虑**有意延迟且已命名缓解**到 M9；其义务目前只活在本 design 的理由里，
归档时必须显式带入 M9 提案/ARCHITECTURE 的 `bead-ffi` 段，避免在里程碑边界蒸发：

- **非默认选项的 oracle 映射测试义务**：M9 一旦让 filter/cell_size/shape 跨边界，FRB 镜像
  外部 `::image::imageops::FilterType` / `#[non_exhaustive] BeadShape` 的「手写 `From` 静默
  偏离默认」风险即回归；M9 必须对每个新跨界选项补一个**对 Rust 内 oracle**（非 CLI，CLI 无
  对应标志）的映射测试。
- **交叉编译 / 静态库 linkage 风险**：M8 故意把 `crate-type` 收成 `["cdylib","lib"]`；M9
  开头要补回 `staticlib` 并做真机静态归档 linkage（iOS `.a` 符号剥离、cdylib-vs-staticlib
  codegen 差异），这是 M8 无法在 host 取得信号的残留风险，须列入 M9 进入风险。
