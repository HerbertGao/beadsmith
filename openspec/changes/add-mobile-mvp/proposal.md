## 为什么

Phase 1（M0–M7）的 Rust 引擎已冻结可信，M8 的 `bead-ffi` 已证明「CLI == FFI 同机逐字节相等」——
但桥目前是 **host-only**（macOS/Linux cdylib，纯 Dart 包），还没有任何真正的 App 消费它。
M9 是路线图最后一个里程碑，也是项目 Phase 2 的收尾：把已验证的引擎包成一个**可在 iOS 设备/模拟器运行、
全程离线**的 Flutter App，兑现 INIT.md 的成功标准（选图→生成→预览→配色计数→复制 summary）。

引擎不变；本里程碑全部工作在 `bead-ffi` 的移动端打包与新的 `apps/mobile` 壳上。

## 变更内容

- **新增 `apps/mobile` Flutter App**：四屏 `HomePage → CropPage → GeneratePage → ResultPage`，
  presentation / application / infrastructure **三层**（ARCHITECTURE.md「Flutter Architecture」整段（全 4 层）
  标注「Future implementation」；本 MVP 先建前三层，无持久化/`SaveProject` 故 domain 留到持久化落地再建——YAGNI）。
- **解除 `bead-ffi` 的 host-only 限制**：把 M8 标注「deferred to M9」的**交叉编译**补上——
  iOS（arm64 真机 + 模拟器）交叉编译 `bead-core`→`bead-ffi` 为静态库链入 Runner，经
  `ExternalLibrary.process` 注入 FRB 装载。**本轮硬验收 iOS 模拟器**；Android（arm64-v8a /
  armeabi-v7a / x86_64 jniLibs）按**各自加载模型**（iOS=staticlib 链入+进程符号；Android=cdylib
  `.so`+jniLibs+`ExternalLibrary.open`，非「同架构换目录」）落地为收尾脚手架，待
  Android SDK+NDK 就绪再验证。
- **App 复用 M8 桥的既有契约**：`generate({imageBytes, paletteJson, width, height}) → GenerateOutput`，
  壳不重新编排任何 image→match→stats→render 流程（CLAUDE 规则 4），裁剪由 Dart 侧
  `crop_your_image` 在调桥**之前**完成（中心裁剪之外的手动裁剪属前端职责，core 不含裁剪 UI）。
- **内置默认调色板**：把 `palettes/artkal_s.json` 作为 Flutter asset 打包随 App 离线分发。
- **复制 summary 到剪贴板**：`ResultPage` 直接复制 `GenerateOutput.summary`（INIT「Summary Format」原文）。

## 功能 (Capabilities)

### 新增功能
- `mobile-app`: `apps/mobile` Flutter 壳的契约——四屏导航流、三层分层（domain 待持久化）、离线优先（无网络/无上传）、
  内置调色板 asset、通过 infrastructure 层的 `PatternEngine` 调 `bead-ffi`、复制 summary，
  以及「壳不含算法、与 bead-cli 一致即正确」的从属约束。

### 修改功能
- `flutter-ffi`: 解除「host-only」范围——新增 iOS（及架构上的 Android）交叉编译与 Flutter
  运行时动态库装载需求。**边界类型不变**（仍只 width/height，`GenerateOutput` 字段不增减），
  filter / cell_size / shape 选项档位**继续推后**（本里程碑 UI 不需要，避免破坏「CLI == FFI」可测性）。

## 影响

- **新增** `apps/mobile/`（Flutter 工程）、Flutter asset 内置 `palettes/artkal_s.json` 副本。
- **修改** `crates/bead-ffi`：仅 `Cargo.toml` 增 `staticlib` crate-type + iOS 交叉编译构建脚本/配置
  （Android jniLibs `.so` 架构就位）。**`bead_ffi` Dart 包（`@generated` glue）不改**——Flutter 下的
  库装载入口**新建**在 `apps/mobile/lib/infrastructure/`（经 `BeadFfi.init(externalLibrary: …)` 注入），
  不触碰生成代码。**`bead-core` 零改动**，`bead-ffi` 的 Rust 桥逻辑零改动（仅打包维度）。
- **新增依赖**（均为 INIT.md「Recommended Technology Stack → Flutter Frontend」钦定、ROADMAP M9 复列，stdlib/原生不覆盖）：
  `image_picker`（系统相册/文件选择——平台权限+选择器，Flutter 无内置）、
  `crop_your_image`（交互式裁剪 UI——core 明确不含裁剪 UI）、
  `flutter_riverpod`（riverpod 的 Flutter 绑定，状态管理）、`go_router`（声明式四屏路由）。
- **确定性**：App 消费的就是 `bead-ffi`/`bead-core` 引擎本身（同一 `generate_pattern` + 默认 options），
  壳只把字节透传给桥、把 `GenerateOutput` 字段原样展示，不重算、不引入随机性——这是**壳层的「由构造一致」**。
  但 **iOS 目标（模拟器/真机）与 host `bead-cli` 是不同编译目标、不同 libm**，浮点 `Lanczos3` 路径不保证
  逐字节相等（ARCHITECTURE Rule 3 跨目标 caveat）；故 **byte-exact「CLI==FFI」闸门仅在 host 同 libm 成立
  （M8 既有 Dart 决定性测试，不回退）**，iOS 端只验证结构不变量（生成成功、计数 = width×height、stats
  schema、summary 格式），**不与 CLI 逐字节/逐值比对**。
- **算法 Phase**：不触碰配色算法，仍为 Phase 1（最近色 RGB）；不提前引入降色/CIELAB/抖动。

## 非目标

- **不做商店上架**：签名证书、商店元数据/截图、App Store Connect / Google Play 上传依赖用户的付费
  开发者账号与私钥，留作 M9 之后的独立变更（届时仅「差一步签名」）。**iOS 硬验收以模拟器为准**（无需
  签名）；iOS 真机运行需用户的免费/付费个人开发团队签名，属 best-effort，不作硬验收。
- **不在本轮验证 Android**：Android 架构按 ROADMAP 落地但本机 Android SDK/NDK 未就绪，验证收尾。
- **不暴露算法选项 UI**：filter/cell_size/shape/降色/抖动均不进 MVP UI。
- **不做** 用户账号、云同步、在线生成、AI 生图、库存模式、多调色板替换、价格计算（沿用 INIT.md Non Goals）。
- **不改 `bead-core` 与 `bead-ffi` 桥逻辑**：只动 `bead-ffi` 的交叉编译/装载与新建 `apps/mobile`。
