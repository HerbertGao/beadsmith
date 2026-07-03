# flutter-ffi 规范（增量）

## MODIFIED Requirements

### 需求:bead-ffi 是 generate_pattern 的零逻辑薄桥

`crates/bead-ffi` 必须是 `bead-core` 到 Dart 的薄桥,禁止包含任何算法或业务逻辑。
它对外只能调用 `bead-core` 的既有公共 API:`load_palette`、`pipeline::generate_pattern`、
`pipeline::pattern_json`；禁止触达 pipeline 的内部阶段(`image_to_grid` / matcher /
统计 / 渲染)或在桥层重新编排生成流程(CLAUDE 规则 4)。`bead-core` 禁止因 FFI 引入
任何 UI / 文件系统 / Flutter / 平台依赖;禁止为 FFI 便利而修改 `bead-core` 的数据模型。
FFI 跨边界的结构化类型必须在 `bead-ffi` 侧完成映射:**出现在响应(返回值)里的结构化
类型必须用 bead-ffi 自有的普通 DTO**(逐字段从 `bead_core` 拷贝),**禁止用 `#[frb(mirror)]`
镜像类型**——镜像类型在**返回值**里会把调用走进 flutter_rust_bridge 的 SSE 编解码路径,
该路径在 **iOS 目标**上以恒定 +6 字节的记账断言失败(`codec/sse.rs:129`)而 panic(与图片
大小/调用参数无关,已由模拟器二分坐实;macOS 对同一字节正常)。镜像(`#[frb(mirror)]`)仍
**允许用于参数**(如 `GeneratorKind`)——参数侧不触发该 panic。`GenerateOutput.pattern` /
`GenerateOutput.stats` 因此以 bead-ffi 的 `BeadPattern` / `ColorStat` DTO(同名同字段:
`width`/`height`/`cells`、`code`/`name`/`count`)跨边界,`generate_inner` 从 `bead_core`
的对应值逐字段拷贝;该 DTO 的字段形状与「结构化数组而非 JSON 跨边界」契约保持不变,对 Dart
消费方透明。

#### 场景:桥只调用既有公共入口
- **当** `bead-ffi` 的桥接函数处理一次生成请求
- **那么** 它必须依次调用 `load_palette` 与 `generate_pattern`(必要时 `pattern_json`),
  禁止自行实现 image→match→stats→render 中的任何一步

#### 场景:core 不被 FFI 污染
- **当** 为支持 FFI 实现桥接
- **那么** `bead-core` 必须保持零改动;跨边界结构化类型的映射在 `bead-ffi` 侧完成(响应类型
  用本地 DTO,见下一场景);禁止在 core 内出现 Flutter / 平台 / 文件系统 / FFI 运行时依赖,
  也禁止把「为何 Clone / 为何 DTO」之类的下游消费者语境写进 core

#### 场景:响应结构类型用本地 DTO,不用镜像
- **当** `bead-ffi` 定义跨边界返回的结构化类型(`GenerateOutput` 内的 `pattern` / `stats`)
- **那么** 它们必须是 `bead-ffi` 自有的普通 FRB struct(DTO),由 `generate_inner` 从 `bead_core`
  值逐字段拷贝;**禁止**对响应中的结构化类型使用 `#[frb(mirror)]`(镜像返回类型在 iOS 触发
  SSE 记账 panic);镜像仅可用于**参数**类型

#### 场景:iOS 上 generate 返回有效结果而非 panic
- **当** 在 iOS(含模拟器)上以合法图像字节与调色板调用桥接 `generate`
- **那么** 调用必须返回结构完整的 `GenerateOutput`(`pattern.width`/`height`/`cells`、`stats`、
  `preview_png` 等非空且形状正确),**不得**因 flutter_rust_bridge SSE 编解码断言而 panic;
  该保证必须由一个**在设备上实际执行**的回归测试守住(测试不得被 `@TestOn` 等选择器静默跳过)
