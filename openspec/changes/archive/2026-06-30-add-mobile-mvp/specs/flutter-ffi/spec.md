## 新增需求

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

## 修改需求
<!-- 无 -->

## 移除需求

### 需求:host-only 范围
**Reason**: M9 正是 M8 显式标注「deferred to M9」的解除点——本里程碑要把桥交叉编译到移动端并让真正的
Flutter App 装载它,host-only 限制在 M9 失效。
**Migration**: 由新增需求「移动端交叉编译与 Flutter 装载(iOS 本轮验证,Android 架构就位)」取代;其中
「依赖隔离」场景承接原需求对 `bead-core` / `bead-cli` 依赖集不变的保证。M8 已交付的 host 动态库与
「CLI == FFI 同机逐字节相等」决定性闸门(本 spec 另一需求)保持有效,不因本次解除而回退。
