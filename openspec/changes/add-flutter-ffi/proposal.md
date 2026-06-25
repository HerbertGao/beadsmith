## 为什么

引擎在 M7 已被 golden 测试冻结、可信。但它现在只有 CLI 一个消费者，而项目终点
（ROADMAP「End goal」）是 Flutter 移动 App。M8 要把 `bead-core` 暴露给 Dart：
不复制任何算法，让 Flutter 端调用与 CLI **逐字节相同**的引擎。这是 Phase 1
（CLI 验证）跨到 Phase 2（移动）的第一块、也是唯一的桥。

「CLI == FFI」不是锦上添花，而是 ROADMAP 的核心闸门：只要 FFI 与 CLI 调同一份
`generate_pattern`，二者输出在同机上按构造即逐字节相同；M9 的 App 若与 CLI 不一致，
bug 必在壳里。本次先把这个闸门用一个 Dart 单测钉死。

## 变更内容

- 新建 `crates/bead-ffi`：`bead-core` 到 Dart 的**薄桥**，零业务逻辑（ARCHITECTURE
  「bead-ffi」+ CLAUDE 规则 4）。仅做：解析输入 → 调 `load_palette` + `generate_pattern`
  → 把 `GenerateResult` 各字段交回 Dart → 把 `BeadError` 转成 Dart 异常。
- 桥接技术选 **`flutter_rust_bridge`（FRB）**：codegen 生成 Dart 镜像类，并自动管理
  跨边界变长缓冲区（两块 PNG + `cells: Vec<u16>` + `stats`）的生命周期 —— 这正是
  手写 `dart:ffi` 里最易出 UAF/泄漏的部分。
- 边界契约（一次纯函数调用，无句柄/无状态）：
  - 入：image bytes、palette JSON 字符串（桥内 `load_palette(palette_json.as_bytes())`，
    因 `load_palette` 接受 `&[u8]`）、`{width, height}`。M8 边界**仅** width/height，filter/
    cell_size/shape 桥内恒取引擎 `Default`（与 CLI 的 `..Default::default()` 一致）——CLI 也
    只有这两个标志，暴露其余选项会使「CLI == FFI」对非默认值不可测，且 filter/shape 是 FRB
    无法直接镜像的外部/`#[non_exhaustive]` 类型。
  - 出：`{width, height, cells}`、`stats[{code,name,count}]`、`summary`、`brand`、
    `preview_png`、`grid_png`，外加 `pattern_json` 字符串（供 M9 落盘）。错误在边界扁平化为
    `BeadError` 的 `Display` 字符串再抛 Dart 异常（外部错误负载无法结构化跨 FRB）。
- 范围 = **host-only**（决策见 design D-Scope）：M8 只交付桥接 + host 动态库 + Dart
  单测过「CLI == FFI」闸门。iOS/Android 交叉编译（XCFramework / cargo-ndk / jniLibs）
  推到 M9 开头——那时有真 App 能立刻加载验证 `.so`/`.a`，反馈回路最短。
- 确定性闸门测试：Dart 单测复用 M7 的固定输入（`samples/gradient.png` +
  `palettes/artkal_s.json`），在**规定的两个尺寸 16×20 + 30×24**（跨长宽比、off-4:5 者非
  正方形，证明 width/height 各自被转发）各按**原始字节**断言四个具名产物文件与**当场跑的 CLI
  输出**逐一相等，并**解析**返回的 `pattern_json` 字符串断言其 width/height/cells/stats/brand
  与结构化字段逐一相等。

## 功能 (Capabilities)

### 新增功能
- `flutter-ffi`: `bead-ffi` 薄桥的契约——边界类型（入/出/错误映射）、「无业务逻辑/
  唯一入口 `generate_pattern`」约束、host-only 范围、以及「CLI == FFI 同机逐字节
  相等」的决定性验收。

### 修改功能
<!-- 无。FFI 是 generate_pattern 的薄桥，不改 pipeline 等任何现有 spec 的规范级行为。 -->

## 影响

- **新增 crate**：`crates/bead-ffi`，加入 workspace `members`。
- **bead-core**：**零改动**。`BeadPattern` / `ColorStat` 已派生 `Clone`，无需为 FFI 补
  derive；FRB 镜像在 `bead-ffi` 侧用 `mirror` 完成，core 不被 FFI 注解污染（规则 1）。
  `GenerateResult` 刻意不 `Clone`，桥接按字段移动取出。**机制待 apply 确认**：FRB 能否
  镜像并从非 `Clone` owner 按字段移动取出——若不能，在 `bead-ffi` 侧包装解决，core 仍
  零改动（变的是 bead-ffi 侧机制，不是 core）。
- **新增依赖**：`flutter_rust_bridge`（仅 `bead-ffi`，不污染 core/cli）。stdlib 无
  FFI codegen；现有依赖（thiserror/serde/image）均不覆盖 Dart 互操作与变长缓冲区
  跨边界生命周期管理；手写 `dart:ffi` 能免依赖但要 ~200 行易错 unsafe 边界码——
  FRB 删掉的恰是这部分，符合「写更少且边界正确的代码」。
- **构建**：M8 引入 `flutter_rust_bridge_codegen`（dev 工具）生成 Dart glue；交叉
  编译工具链（cargo-ndk / xcframework）**不**在本次范围。
- **确定性**：不受影响。FFI 与 CLI 共用同一 `Cargo.lock`、同一 `generate_pattern`，
  同机同 target 编译 → 同 image+palette+dimensions+options 仍产生逐字节相同输出
  （M7 已隔离的 `Lanczos3` 跨架构浮点分歧在 same-device 测试下不出现）。

## 非目标

- **不交叉编译到真机**：iOS/Android `.a`/`.so` 构建、XCFramework/jniLibs 打包、签名
  → M9 开头。
- **不写 Flutter App**：无 UI、无屏幕、无 riverpod/go_router → M9。
- **不引入新算法 / 不动配色档位**：本次零算法（仍是算法 Phase 1 最近色），不碰
  quantizer/matcher/renderer 的算法逻辑。
- **不做 palette 句柄缓存**：每次调用传 JSON、桥内重解析（几百色 = 微秒级，YAGNI；
  触发器与可逆性见 design D-Entry）。
- **M8 边界不暴露 filter/cell_size/shape 选项**：M8 边界 = CLI 的 width/height；这三个
  档位待 M9 真有 UI 需要时再加（仍走 `generate_pattern`）。
- **不给引擎加维度上限**：超大 `width × height` 的守卫属引擎改动、超薄桥范围；M8 继承
  引擎现状（已知限制，见 design 风险段）。
- **不暴露内部 pipeline 子阶段**：只过 `generate_pattern` + `load_palette` +
  `pattern_json` 三个既有公共 API，不让 Dart 触达内部阶段（CLAUDE 规则 4）。
- **不做通用 C ABI / WASM 消费者**：当前只有 Flutter 一个消费者 → YAGNI；将来真需要
  再叠 cbindgen 层。
