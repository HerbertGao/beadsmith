## 为什么

`generate` 在 **iOS 目标**上返回时必崩:flutter_rust_bridge 2.12.0 的 SSE 编解码在
反序列化该调用的消息时,`codec/sse.rs:129` 的 `assert_eq!(data_len, cursor.position())`
以**恒定 +6 字节**失败(`SseDeserializer::end()`)。表现为 `PanicException(assertion left == right)`,
被生成页 try/catch 接住显示成错误文案。

经模拟器上 headless 二分坐实:与图片大小无关(9KB 与 13MB 同样 +6)、与调用**参数**无关
(带 widen-ffi 新增的 `max_colors`/`despeckle`/`generator` 全 7 参 + 大图的 `probe_full` 正常),
**触发点是响应类型 `GenerateOutput` 里的 `#[frb(mirror)]` 镜像类型**(`BeadPattern`、`Vec<ColorStat>`):
镜像类型在**返回值**里会把调用走进 iOS 上会崩的 SSE 路径;返回普通结构体(`Vec<u8>`/`Vec<u16>`/`String`
字段)则一律正常。macOS(CLI 直调 + FRB 桥)对**同一字节**均正常,故此前从未暴露。

这个坑自 **M8**(`GenerateOutput` / 镜像类型引入)就潜伏,只是 M9 的 iOS 验收从未真正跑过
`generate`——`engine_on_ios_test.dart` 的 `@TestOn('ios')` 选择器使它**静默从未执行**,bug 才随
`upgrade-crop-editor` 第一次把真实照片喂过 `generate` 而暴露。属独立于裁剪器的 FFI 边界修复。

## 变更内容

- `GenerateOutput` 的 `pattern` / `stats` 从 `#[frb(mirror(BeadPattern))]` / `#[frb(mirror(ColorStat))]`
  改为 **bead-ffi 自有的普通 FRB DTO**(`BeadPattern` / `ColorStat`,同名同字段,对 Dart 透明);
  `generate_inner` 逐字段从 `bead_core` 值拷贝进 DTO。**`GeneratorKind` 保持镜像**——它只作**参数**
  跨桥,二分已证明参数侧不受影响。
- 重新生成 FRB 胶水(`api.dart` / `frb_generated.rs`);`bead-core` **零改动**,契约「结构化数组而非
  JSON 跨边界」「CLI == FFI 逐字节相等」均保持。
- 新增**真正会运行**的 iOS 回归测试(在 booted 设备上实调 `generate` 断言返回有效 pattern),
  **不带 `@TestOn('ios')`**——该选择器正是让老引擎测试从未运行、bug 漏网的原因。

### 非目标

- 不升级 / 不更换 flutter_rust_bridge(保留钉死的 `=2.12.0`),不切 DCO 编解码。
- 不改 `bead-core`、不改 `generate` 的对外签名与边界契约、不动裁剪器(`upgrade-crop-editor`)。
- 不给 `image_picker` 加尺寸上限(与本 bug 无关——恒定 +6 与图片大小无关)。

## 功能 (Capabilities)

### 新增功能
<!-- 无:本变更修改既有 flutter-ffi 契约。 -->

### 修改功能
- `flutter-ffi`:①「bead-ffi 是 generate_pattern 的零逻辑薄桥」——把「跨边界镜像用 FRB `mirror`
  **或**本地包装」中出现在**响应**里的结构化类型**收窄为必须用本地 DTO(不得用 `#[frb(mirror)]`)**,
  因镜像返回类型在 iOS 触发 SSE 记账 panic;镜像仍可用于**参数**(`GeneratorKind`)。②「移动端交叉编译
  与 Flutter 装载」——iOS 验收必须**实际执行** `generate` 并断言返回有效 `GenerateOutput`(回归测试
  不得被 `@TestOn` 等选择器静默跳过),守住「iOS 上 generate 返回不 panic」。

## 影响

- **代码(仅 `crates/bead-ffi` + `apps/mobile` 测试)**:
  - `crates/bead-ffi/src/api.rs`:`BeadPattern` / `ColorStat` 由镜像改 DTO + `generate_inner` 拷贝;
    `pub use bead_core::{...}` 去掉两类型(保留 `GeneratorKind`);两 DTO 加 `#[derive(Debug, PartialEq)]`。
  - `crates/bead-ffi/src/frb_generated.rs`、`crates/bead-ffi/dart/lib/src/api.dart`:重新生成。
  - `apps/mobile/integration_test/generate_ios_regression_test.dart`:新增 on-device 回归测试。
- **`bead-core`**:零改动。**FRB 版本**:不变(`=2.12.0`)。**Dart 侧 App 代码**:不变(DTO 同名同字段)。
- **确定性**:纯 marshalling 改动,不触引擎;「CLI == FFI 逐字节相等」闸门与 golden 不受影响。
- **验证(已完成)**:iOS 模拟器实跑 9.7MB `generate` 成功不崩;决定性闸门 4/4 逐字节过;
  bead-ffi 库测试 15/15;`flutter analyze` 无问题。
