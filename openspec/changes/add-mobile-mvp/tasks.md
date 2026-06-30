## 1. bead-ffi iOS 交叉编译(打包维度,桥逻辑零改动)

- [x] 1.1 `crates/bead-ffi/Cargo.toml`:`[lib] crate-type` 增加 `"staticlib"`(保留 `cdylib`/`lib`),
  仅打包维度,不动 `src/api.rs` 桥逻辑。验收:`cargo build -p bead-ffi` 仍通过,host 决定性测试不回退。
- [x] 1.2 安装并交叉编译 iOS target:`rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios`,
  `cargo build -p bead-ffi --release --target aarch64-apple-ios`(及两个模拟器 target)。
  验收:三个 target 各产出 `libbead_ffi.a`。
- [x] 1.3 在 `crates/bead-ffi/` 或 `apps/mobile/ios/` 下放一段构建脚本(`scripts/build-ios.sh` 或 Xcode
  build phase)封装 1.2,使 `apps/mobile` iOS 构建能自动产/取静态库。验收:干净环境跑脚本得到可链接 `.a`。

## 2. Flutter App 脚手架与依赖

- [x] 2.1 `flutter create apps/mobile`(org/bundle id 占位即可,签名留空——非本变更范围)。验收:`flutter run`
  能起空白 App(iOS 模拟器)。
- [x] 2.2 `apps/mobile/pubspec.yaml` 加依赖:`image_picker` / `crop_your_image` / `flutter_riverpod` /
  `go_router`,**pin 到已知良好版本**(尤其 `crop_your_image`——其裁剪字节返回形状是 load-bearing 假设,
  pin 防版本漂移悄改 API);并以 path 依赖引用 `crates/bead-ffi/dart`(复用 M8 生成的 glue);`flutter_rust_bridge`
  运行时 pin 与 Cargo `=2.12.0` 一致。验收:`flutter pub get` 通过,依赖只在 `apps/mobile`(crate 依赖集不变)。
- [x] 2.3 建**三层**目录骨架:`apps/mobile/lib/{presentation,application,infrastructure}/`(domain 层 MVP 暂不建,
  见 4.2 YAGNI 说明)。验收:目录骨架就位(脚手架,真正的行为验证在 4.x;此处仅 `flutter analyze` 无错)。

## 3. Flutter 运行时装载 bead-ffi(iOS)

- [x] 3.1 iOS 平台胶水:把 1.x 的静态库链入 Runner(podspec / build phase 触发构建或链接预构建 `.a`)
  **+ `-force_load`(或等价 keep)防链接器 dead-strip 仅经 `process()` 运行期到达的 Rust 符号**,影响
  `apps/mobile/ios/`。验收:iOS 构建产物内含 `bead_ffi` 符号(**必要非充分**——真正的装载硬验收在 3.2)。
- [x] 3.2 infrastructure 层装载入口:`apps/mobile/lib/infrastructure/bead_ffi_loader.dart`——以
  `BeadFfi.init(externalLibrary: ExternalLibrary.process(iKnowHowToUseIt: true))` 初始化(FRB 2.12.0 收
  `ExternalLibrary?`,**非** `DynamicLibrary`;替代 host 的路径 dlopen)。验收(硬):App 内**实际调用
  `generate` 成功返回 `GenerateOutput`**,不抛装载/符号查找错误。

## 4. 引擎封装与四屏(presentation/application/infrastructure)

- [x] 4.1 infrastructure `PatternEngine`(`apps/mobile/lib/infrastructure/pattern_engine.dart`):薄封装
  `bead_ffi.generate({imageBytes, paletteJson, width, height}) → GenerateOutput`,壳内无任何
  缩放/匹配/统计/渲染逻辑(CLAUDE 规则 4)。验收:单测以固定 fixture 调用返回非空 `GenerateOutput`。
- [x] 4.2 application 用例:`GeneratePattern`(经 `PatternEngine`)、`CopySummary`(经 infrastructure
  `ClipboardService`)。**MVP 不建 domain→`GenerateOutput` 映射层**(无 `SaveProject`/持久化,YAGNI;
  presentation 直接消费用例返回的 `GenerateOutput`,避免 domain 反向依赖 infra 的 FRB DTO)。验收:
  `GeneratePattern` 单测返回非空 `GenerateOutput`;`flutter analyze` 无错。
- [x] 4.3 `HomePage`(presentation):`image_picker` 选图 → 路由到 `CropPage`(`go_router`)。验收:模拟器选图进下一屏。
- [x] 4.4 `CropPage`:`crop_your_image` 交互裁剪,确认后把**裁剪后字节**经 riverpod 生成会话 provider 带入
  `GeneratePage`(裁剪在调桥前,D5;字节走会话状态而非 `go_router` extra,避免重建/深链丢失)。
  验收:裁剪结果字节传递成功,壳内无像素级裁剪算法。
- [x] 4.5 `GeneratePage`:用户设 width×height(预设或数值),经 `GeneratePattern` 用例调 `PatternEngine`;
  失败时展示桥抛出的扁平化异常消息(不崩溃)。验收:正常生成进 `ResultPage`;传非法尺寸显示错误消息。
- [x] 4.6 `ResultPage`:展示 `previewPng`、`stats` 计数列表、`summary`;「复制」经 application `CopySummary`
  → infrastructure `ClipboardService` 写 `summary` 原文到剪贴板(不在 presentation 直接调平台剪贴板,守分层)。
  预览/计数/summary 全取自 `GenerateOutput`,不反推(spec「结果只从 GenerateOutput 派生」)。验收:三者正确显示,复制后剪贴板=summary。

## 5. 调色板 asset 与离线

- [x] 5.1 复制 `palettes/artkal_s.json` 为 `apps/mobile/assets/palettes/artkal_s.json` 并在 pubspec 声明 asset;
  运行时 `rootBundle` 读成 String 传 `paletteJson`。验收:`cmp` 副本与源逐字节相等(无 BOM UTF-8),漂移即失败。
- [x] 5.2 离线核验:模拟器开飞行模式走完选图→生成→复制全流程。验收:飞行模式下全流程完成(证离线**可用**);
  「禁止网络请求」由构造保证——`pubspec.yaml` 不引入任何 http/网络依赖(核验依赖清单)。

## 6. 端到端验收(iOS 模拟器 + 分层决定性对账)

- [x] 6.1 iOS **模拟器**(硬验收;真机 best-effort、需个人开发团队签名)跑通 INIT 成功标准六步(选图→裁剪→
  设尺寸→生成→预览/计数→复制)。验收:六步全过、全程离线。
- [x] 6.2 决定性**分层**验收(避免跨目标浮点误判):
  - **(host 闸门)** 既有 M8 Dart 决定性测试(desktop `bead_ffi` vs 同机 `bead-cli`,同 macOS libm)保持通过
    = byte-exact「CLI == FFI」,不回退。
  - **(iOS)** App 端**不**与 CLI 逐字节比对(iOS≠host libm,Rule 3 跨目标 caveat),只断言结构不变量:
    `GenerateOutput.pattern.cells` 长度 = width×height(即总豆数,DTO 无 `total_beads` 字段——由
    `pattern.cells.length` 或 `Σ stats.count` 派生)、stats schema、summary 为 INIT 格式。
  - 若确需任何 host 级 App-流对账,必须把 App **交给桥的裁剪后字节**(非原始选图)写临时文件喂
    `bead-cli generate --input`——否则 CLI 内部 `crop_center` 与 Dart 手动裁剪不同 → 假阳性。

  验收:host 闸门绿 + iOS 结构不变量成立;仅当 host 整数/结构路径不一致才按 CLAUDE 规则 5 视为壳 bug。

## 7. Android 架构就位(本轮不验证)

> Android 与 iOS 是**不同加载/链接模型**(iOS=staticlib 链入+`process()` 符号;Android=cdylib `.so`+jniLibs
> +`ExternalLibrary.open`),非「同架构换目录」。本组只落**最小声明式脚手架,未验证、预期需迭代**,不 ship
> 声称完成却跑不动的 Gradle/NDK 胶水。

- [x] 7.1 Android 交叉编译路径落到构建配置:Gradle/NDK 触发 `cargo build --target aarch64-linux-android`
  等三 ABI(`cdylib`→`.so`),产物按 `apps/mobile/android/app/src/main/jniLibs/{arm64-v8a,armeabi-v7a,x86_64}/`
  结构就位。验收:**最小**配置 + jniLibs 目录 + 文档 TODO 就位(脚手架,未验证);**实际编译/真机验证标注
  「需用户装 Android SDK+NDK 后执行」**,不阻塞 iOS 收口。
- [x] 7.2 infrastructure 装载入口补 Android 分支(`ExternalLibrary.open("libbead_ffi.so")`)。验收:代码就位,
  Android 验证随 7.1 一并推迟。

## 8. 文档与收尾

- [x] 8.1 更新 `CLAUDE.md`(「currently at M0」→ M9)、`ROADMAP.md` M9 标注离线 App 已达成、商店上架另开变更,
  **并把 M9 的分层从 4 层更新为「MVP 三层(presentation/application/infrastructure),domain 待持久化/`SaveProject`
  落地」**(否则 ROADMAP 与本变更三层契约相矛盾);
  `ARCHITECTURE.md` bead-ffi 段把「host-only」更新为「iOS 已交叉编译验证,Android 架构就位」;**并更新
  `openspec/specs/flutter-ffi/spec.md` 的「目的」段**(去掉 host-only 表述,改为 iOS 交叉编译/装载范围——
  归档的需求 delta 不会重写非需求的「目的」前言)。验收:文档与现状一致。
- [x] 8.2 `apps/mobile/README.md`:本地构建运行步骤(含 iOS 静态库构建、Android 待办前置)。验收:照 README 从干净
  checkout 能 `flutter run` 出 iOS App。
- [x] 8.3 收尾自检:`cargo test`(引擎/桥不回退)+ `flutter analyze`(App 无错)+ host 决定性测试仍过。验收:三者全绿。
