## 上下文

M0–M8 完成:Rust 引擎冻结,`bead-ffi` 已证明「CLI == FFI 同机逐字节相等」,但桥是 **host-only**
(macOS/Linux `cdylib` + 纯 Dart 包 `crates/bead-ffi/dart`)。M9 要把这套已验证引擎包成一个真机可跑、
全程离线的 Flutter App,兑现 INIT「Success Criteria」。

当前消费契约(M8 已生成,不改):
```dart
Future<GenerateOutput> generate({
  required List<int> imageBytes,
  required String paletteJson,
  required int width,
  required int height,
});
// GenerateOutput { BeadPattern pattern; List<ColorStat> stats; String summary;
//                  String brand; Uint8List previewPng; Uint8List gridPng; String patternJson; }
```

约束(本机已核验):Flutter/Dart/Xcode 26.6 就绪;**Android SDK/NDK 缺失**;Rust 仅装了 host target,
移动 target 待加。用户已定:范围 = **可运行的离线 App**(商店上架另开变更);平台 = **iOS 优先,Android
收尾**。

## 目标 / 非目标

**目标:**
- `apps/mobile` Flutter App:四屏(`HomePage→CropPage→GeneratePage→ResultPage`)、三层分层(presentation/
  application/infrastructure;ARCHITECTURE「Flutter Architecture」整段〔全 4 层〕标注「Future implementation」,
  MVP 先建前三层、无持久化故 domain 暂不建)、离线优先。
- 解除 `bead-ffi` host-only:iOS 交叉编译 + Flutter 运行时装载,iOS 模拟器跑通 `generate`(真机 best-effort)。
- Android 三 ABI 交叉编译路径在构建配置里就位(本轮不强制验证)。
- 壳即引擎:壳只透传字节 + 原样展示 `GenerateOutput`;**host 端** byte-exact「CLI == FFI」(M8 既有测试),
  **iOS 端**结构不变量一致(不跨目标逐字节比 CLI,见 §风险)。

**非目标:**
- 商店签名/元数据/上传(依赖用户付费账号与私钥,另开变更)。
- 本轮验证 Android(SDK/NDK 未就绪)。
- 暴露 filter/cell_size/shape 等算法选项 UI;不碰配色算法(仍 Phase 1 最近色)。
- 改 `bead-core` 或 `bead-ffi` 的桥 Rust 逻辑/边界类型。

## 决策

### D1 — App 直接复用 M8 的 `bead_ffi` Dart 包,不重写桥
App 的 infrastructure 层 `PatternEngine` 是对 `bead_ffi.generate` 的薄封装。
- **替代方案**:为移动端重写一套 FRB 桥 / 暴露更多入口。**否决**:M8 桥契约已被「CLI == FFI」闸门钉死,
  重写=重新承担决定性风险;且 CLAUDE 规则 4 要求唯一编排入口。壳只调不重排。

### D2 — host 纯 Dart 包 vs Flutter 插件:用 path 依赖复用,装载方式分平台
M8 的 `crates/bead-ffi/dart` 是**纯 Dart** 包,其 `frb_generated.io.dart` 按文件路径 `dlopen` host dylib——
这套在 Flutter 真机上不成立(库要随 App 打包并按平台名装载)。
  这套 glue 的可复用性已从生成代码证实:`frb_generated.dart` 暴露 `BeadFfi.init({ExternalLibrary? externalLibrary})`
  且包只 import `flutter_rust_bridge_for_generated.dart`(不 import Flutter),故 Flutter App 可 path-depend
  该纯 Dart 包并在 init 时注入自己的库——复用 glue **不是** fork。
- **决策（两条独立轴,分开判定）**:
  ① **glue 复用(已决、可靠)**:`apps/mobile` 通过 path 依赖引用既有 `bead_ffi` 包复用生成的 glue
  (api.dart/frb_generated*),装载入口经 `BeadFfi.init(externalLibrary: …)` 注入(FRB 2.12.0 收
  `ExternalLibrary?`,**不是** `DynamicLibrary`):iOS 静态链接用 `ExternalLibrary.process(iKnowHowToUseIt: true)`
  解析进程内符号,Android 用 `ExternalLibrary.open("libbead_ffi.so")`;入口新建在 App 的 infrastructure 层,
  **不改 `@generated` glue**。
  ② **原生构建+链接机制(真正的 open question,见末节)**:把 `bead-ffi` 交叉编译并链进 Runner 的胶水形态。
- **替代方案 A**:把 `bead_ffi` 改造成完整 `flutter_rust_bridge` 插件(plugin template,含 ios/android 平台
  目录 + cargokit 自动构建)。**否决**:最接近 FRB 官方移动路径,但要重排 M8 已交付、已被「CLI==FFI」闸门钉死
  的纯 Dart 包结构——动既有契约面风险大于收益。**选定路径=复用 glue(path 依赖)+ App 侧平台构建胶水**,
  自成一档,不是 A 的子变体。
- **否决**「直接搬 host 的路径 dlopen 到移动端」:移动端无稳定可写绝对路径,必败。

### D3 — iOS 工件形态:`staticlib` 链入 Runner(优先),`cdylib`/framework 次选
`bead-ffi` 当前 crate-type = `["cdylib","lib"]`,注释已标注 `staticlib` 是 M9(真机 archive 链接)。
- **决策**:为 iOS 增 `staticlib`,交叉编译 `aarch64-apple-ios`(真机)+ `aarch64-apple-ios-sim`/
  `x86_64-apple-ios`(模拟器),链进 Runner;符号经 `ExternalLibrary.process(iKnowHowToUseIt: true)` 解析。
- **替代方案**:动态 framework + 嵌入签名。**否决**:静态链最省签名/嵌入麻烦,且离线 App 无热更需求。
- 这是 `bead-ffi/Cargo.toml` 的**打包维度**改动,不动桥 Rust 逻辑(符合「边界零改动」需求)。
- **iOS 静态库已知坑(dead-strip)**:链接器会 strip 仅经运行期 `dlsym`/`process()` 到达的 Rust 符号,
  必须用 `-force_load`(或等价 keep)保留 `libbead_ffi.a` 全部符号,否则 `ExternalLibrary.process()` 运行期
  查不到符号 → 装载失败。故 task 3.1 的「符号存在」是**必要非充分**,真正的硬验收是 task 3.2 在 App 内
  **实际调用 `generate` 成功**。

### D4 — 状态管理与路由:照 ROADMAP 钦定栈,不自造
`riverpod`(状态)+ `go_router`(四屏路由)+ `image_picker`(选图)+ `crop_your_image`(裁剪)。
- **替代方案**:setState/Navigator 1.0 自管。**否决**:INIT「Recommended Technology Stack」(ROADMAP M9 复列)已钦定,
  且四屏 + 异步生成 + 失败态用 riverpod 远比手搓清晰;这些都是 Flutter 生态标准件,无 stdlib/原生替代。
- 依赖只进 `apps/mobile/pubspec.yaml`,不污染任何 crate(flutter-ffi spec「依赖隔离」)。

### D5 — 裁剪在 Dart 侧、调桥之前
`crop_your_image` 产出裁剪后字节 → 作为 `imageBytes` 传桥。core 不含裁剪 UI(只有 `crop_center`)。
- **替代方案**:把裁剪框传给引擎让 core 裁。**否决**:违反「core 不含 UI / 数据进数据出」;且引擎边界只收
  最终字节,M8 契约不含裁剪参数。

### D6 — 调色板作为 asset 打包,无 BOM UTF-8
`palettes/artkal_s.json` 复制为 `apps/mobile` 的 Flutter asset,运行时 `rootBundle` 读成 String 传 `paletteJson`。
- **替代方案**:把 JSON 内联进 Dart 常量 / 运行时下载。**否决**:内联难与源 palette 保持逐字节一致(「CLI==FFI」
  前提);下载违反离线。asset 文件副本须与 `palettes/artkal_s.json` 同步(tasks 留一致性核验)。

## 风险 / 权衡

- **[纯 Dart 包→Flutter 装载不平滑]** M8 的 `bead_ffi` 是纯 Dart 包,移动端装载机制不同 → 缓解:D2 复用 glue
  但重做装载入口;iOS **模拟器实跑 `generate`** 作为硬验收(真机 best-effort),不靠「编译过」糊弄。
- **[iOS 目标 ≠ host libm,跨目标浮点不逐字节相等]** iOS(模拟器/真机)与 host `bead-cli` 是**不同编译
  目标、不同 libm**,`Lanczos3`(`f32::sin`)→ `cells/stats/summary/preview/grid` 都不保证逐字节相等
  (ARCHITECTURE Rule 3 跨目标 caveat)→ 缓解:**byte-exact「CLI==FFI」闸门仅保留在 host**(M8 既有 Dart
  决定性测试,desktop `bead_ffi` vs `bead-cli` 同 macOS libm,不回退);**iOS 端不与 CLI 逐字节比对**,只验证
  结构不变量(生成成功、`pattern.cells` 长度 = width×height〔即总豆数;DTO 无 `total_beads` 字段,由
  `pattern.cells.length` / `Σ stats.count` 派生〕、stats schema、summary 为 INIT 格式)。壳层的「由构造一致」仅指透传字节 + 原样展示 `GenerateOutput`,**不等于**跨目标逐字节等同 CLI。
- **[误用 CLAUDE 规则 5]** iOS≠host 的浮点漂移属上面的 Rule 3 跨目标 caveat,**不是壳 bug**:对账只在 host
  整数/结构路径不一致时才按规则 5 查壳,跨目标浮点差异不触发改壳。
- **[Android 工具链缺失 + 与 iOS 非同模型]** 本机无 SDK/NDK,且 Android=cdylib `.so`+jniLibs+`ExternalLibrary.open`
  与 iOS 的 staticlib+process **不同加载/链接模型**(非「同架构换目录」)→ 缓解:Android 只落**最小声明式**
  脚手架(jniLibs 目录 + 文档 TODO + 装载分支),标注「脚手架、未验证、预期需迭代」,不 ship 声称完成却跑不动的
  Gradle/NDK 胶水;验证显式推迟到用户装 SDK/NDK,不阻塞 iOS 收口。
- **[asset palette 与源漂移]** 复制副本可能与 `palettes/artkal_s.json` 不同步 → 缓解:tasks 留一条逐字节
  一致性核验(可用 `cmp`/构建期校验),漂移即失败。
- **[FRB/工具链版本漂移]** `flutter_rust_bridge` 运行时 pin = 2.12.0(Cargo `=2.12.0`)→ App 侧 pubspec 也须
  pin 同版,移动构建若触发 codegen 必须 no-diff(继承 M8 task 4.2 约束)。

## 待解决问题

- iOS **原生构建+链接机制**(non-glue 轴)的最终形态——三选一:podspec/build-phase 触发 `cargo build`、
  预构建 `.a` 提交、或**把 cargokit 的构建脚本 vendor 进 `apps/mobile/ios` 而不把 `bead_ffi` 重构成插件**
  (第三条路,兼顾 force_load/codesign 自动化与「不动 M8 包结构」)——在 tasks 3 段定。判据:能从干净 checkout
  `flutter run` 出 iOS 包、运行期 `generate` 成功、dead-strip 已用 `-force_load` 解决,且手工步骤最少。
  注意:glue 复用轴已决(见 D2 ①),此 open question **只关原生构建机制**,不回退 glue 决策。
- `GeneratePage` 尺寸输入用预设(40×40/80×100…)还是自由数值——MVP 取其一即可,留 tasks/实现期定,不影响 spec。
