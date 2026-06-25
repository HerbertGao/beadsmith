## 1. crate 脚手架与依赖

- [x] 1.1 新建 `crates/bead-ffi`（`Cargo.toml` + `src/lib.rs`），加入根 `Cargo.toml`
  的 workspace `members`；`bead-ffi` 依赖 `bead-core`（path）。验收：`cargo build`
  全 workspace 通过。
- [x] 1.2 在 `crates/bead-ffi/Cargo.toml` 钉 `flutter_rust_bridge` 精确版本（非 `^`）；
  `crate-type` 设为 `["cdylib", "lib"]`（host 实际构建的两种；**不**预声明 `staticlib`——
  真机静态库属 M9，本里程碑不构建不验证它，YAGNI）。验收：`cargo build -p bead-ffi`
  产出 host 动态库。
- [x] 1.3 确认 `bead-core` / `bead-cli` 依赖集**未**变化（FRB 只进 bead-ffi）。验收：
  `cargo tree -p bead-core` 与 `-p bead-cli` 不含 `flutter_rust_bridge`。

## 2. core 跨边界类型适配（确认零改动）

- [x] 2.1 确认 `BeadPattern` / `ColorStat` **已派生 `Clone`**（`crates/bead-core/src/
  models/mod.rs`），M8 对 `bead-core` 零改动；真正待确认的是 FRB 能否镜像这两个类型并从
  **非 `Clone`** 的 `GenerateResult` 按字段移动取出——若 FRB 的 mirror/opaque 机制对非
  `Clone` owner 有要求，在 `bead-ffi` 侧包装解决，仍不碰 core。验收：`git diff` 对
  `crates/bead-core/` 为空；`cargo test -p bead-core` 全绿。
- [x] 2.2 用 FRB `mirror` 在 `bead-ffi` 侧镜像 `BeadPattern` / `ColorStat`，core 不加任何
  `#[frb(...)]` 注解（规则 1）。验收：`cargo build -p bead-ffi` 编译通过、core 无 FFI 注解
  （codegen 配置与「codegen 成功」在 4.1 验收——本任务不依赖 codegen 工具就位）。

## 3. bead-ffi 桥接 API（零业务逻辑，最小边界）

- [x] 3.1 在 `crates/bead-ffi/src/api.rs` 写单一桥接函数：入 `image_bytes: Vec<u8>`、
  `palette_json: String`、`width: u32`、`height: u32`；内部
  `load_palette(palette_json.as_bytes())?`（`load_palette` 接受 `&[u8]`）→ 组
  `GenerateOptions { width, height, ..Default::default() }`（与 CLI `main.rs` 完全相同的
  构造，filter/cell_size/shape 取引擎默认 Lanczos3/10/Square）→ `generate_pattern`。
  **不**暴露 filter/cell_size/shape 参数。验收：函数体只调既有公共 API、无重编排；边界仅
  width/height（对应 spec「桥只调用既有公共入口」「选项构造与 CLI 一致」）。
- [x] 3.2 桥接返回结构：pattern(width/height/cells:Vec<u16>)、stats(Vec<{code,name,
  count}>)、summary、brand、preview_png:Vec<u8>、grid_png:Vec<u8>，并暴露
  `pattern_json` 字符串。`cells`/`stats` 为结构化数组而非 JSON 串。验收：对应 spec
  「一次调用返回全部产物」「结构化数组而非 JSON 化的 cells」。
- [x] 3.3 错误路径：桥接在边界把 `BeadError` **扁平化为 `Display` 字符串**（`to_string()`）
  再交 FRB 抛 Dart 异常（禁止结构化返回——`PaletteParse`/`ImageDecode` 元组变体与
  `ImageEncode { source }` 具名变体包裹的 `serde_json::Error`/`::image::ImageError` 无法跨
  FRB；`to_string()` 不依赖变体形状）。验收**分两层**（不同契约，分开测）：① 私有内层
  `#[test]` 断言以下失败输入各返回对应 `BeadError` 变体——无法解码图像、空图像字节
  （均 `ImageDecode`）、JSON 语法错调色板（`PaletteParse`）、语义非法调色板（零颜色/重复
  code/非法 hex，均 `InvalidPalette`）、零维度（`InvalidImage`）；② 导出边界测试断言抛给
  Dart 的消息 == 该 `BeadError` 的 `to_string()`（对应 spec「非法输入抛出带消息的异常」
  「外部错误负载不跨边界」）。注：`ImageEncode` 经 M8 黑盒输入**不可达**（维度守卫后的有效
  buffer 编码不失败），由通用 `err.to_string()` 映射隐式覆盖，不单测。

## 4. FRB codegen 与最小但真实的 Dart 包

- [x] 4.1 配置 `flutter_rust_bridge_codegen`，生成 Dart glue 到 `crates/bead-ffi/dart/`
  ——一个**最小但真实**的 Dart 包（稳定 import 路径，M9 的 `infrastructure/PatternEngine`
  直接扩展它），**不**建 `apps/mobile` 骨架。验收：codegen 成功、产物入库、闸门测试针对此
  包的 import 表面运行（design 开放问题已定）。
- [x] 4.2 校验「重跑 codegen 无 diff」：**显式钉 `flutter_rust_bridge_codegen` 工具版本**
  （lockfile / CI 固定安装，非仅运行时 crate），且校验前**规范化 Dart 格式**，使守卫检验
  「签名一致」而非「formatter 版本一致」；再跑一次 codegen 后 `git diff` 为空。验收：固定
  工具版本下生成代码与 Rust 签名一致（design 风险「FRB codegen 漂移」）。

## 5. 决定性闸门测试（CLI == FFI 同机逐字节）

- [x] 5.1 写 Dart 测试：以 M7 固定输入（`samples/gradient.png` +
  `palettes/artkal_s.json`）在**至少两个尺寸**各调 FFI——两尺寸 width/height 各异且跨长宽比
  （用 **16×20 与 30×24**：16×20 是 fixture 的 4:5、30×24 是 5:4 偏离 4:5 且**非正方形**，
  故能让按比例推导 height 的桥露馅、并单尺寸即抓 width↔height 调换；24×30 是 4:5 陷阱勿用；
  30×24 强制一次非恒等 crop+resize），取四样产物字节 + 结构化
  width/height/cells/stats/brand + pattern_json。测试 import 的稳定入口
  符号（generate 函数 + 结果类型）即「M9 复用的同一 import 表面」，由本测试强制（regen 改名
  会编译失败）。验收：测试可运行、能从 FFI 拿到这些。
- [x] 5.2 同测试内**每个尺寸**同机运行 `bead-cli generate` 同输入，按**原始字节**（二进制
  读取、无换行规范化）逐一断言**四个具名文件** `pattern.json`/`summary.txt`/`preview.png`/
  `grid.png` 与 FFI 输出相等——**不**断言输出目录「为空」（`create_dir_all` 不清既有目录）；
  Dart 写 `summary` 落盘时**禁止**追加尾换行（CLI 写 `result.summary` 原样）。**跨长宽比的
  多尺寸**证明 `width`/`height` 各自被桥转发。验收：对应 ROADMAP M8「Dart 单测拿到与 CLI
  相同结果」+ spec「Dart 测试证明 CLI == FFI（多尺寸、跨长宽比）」。
- [x] 5.3 **解析**该次返回的 `pattern_json` **字符串**，断言其 `width`/`height`/`cells`/
  `stats`/`brand` 与 Dart 经 FRB 收到的结构化字段逐一相等（不只 `cells`；JSON 侧值来自
  解析字符串而非结构化数组再序列化，否则空洞通过）。`pattern_json` 的 `total` 键与 `summary`
  **不**在此自洽检查内——二者已由 5.2 的四文件逐字节比较传递覆盖；**自洽字段表固定为这 5 项、
  勿增 `total`/`summary`**（塞进来会重新引入 round-3 关掉的空洞风险）。验收：对应 spec「结构化
  字段与 pattern_json 自洽（解析而非再序列化）」。
- [x] 5.4 测试对比的是**当场 CLI 输出**而非 `tests/golden/` byte master（非 canonical 机
  也成立）；host 工具链缺失时跳过并 log，不静默假绿。验收：在 macOS arm64 开发机上测试
  通过（对应 spec「对比当场 CLI 输出而非 canonical golden master」）。

## 6. 收尾与边界校验

- [x] 6.1 确认 host-only 范围：仓库内**无** iOS/Android 交叉编译、XCFramework、
  jniLibs、签名相关产物或脚本，且 `crate-type` 不含 `staticlib`（design 非目标 / spec
  「不交叉编译到移动端」）。验收：`git status` 与目录树无移动端构建文件。
- [x] 6.2 文档：在 `ARCHITECTURE.md` 的 `bead-ffi` 段落补一句「M8 host-only、边界仅
  width/height、core 零改动；交叉编译与选项档位属 M9」。验收：文档与范围一致。
- [x] 6.3 全量回归：`cargo build && cargo test` 全 workspace 绿；`bead-core` golden
  测试（M7）不受影响仍通过。验收：CI 全绿。
