## 为什么

现有裁剪屏用 `crop_your_image`,**不支持旋转/翻转**,也无比例快捷。先前尝试的成熟包 `pro_image_editor` 经真机/模拟器实测:它用 `RepaintBoundary.toImage`(截图)捕获结果,而 `toImage` 在 **iOS 模拟器**(软件渲染器)上失败——抛 "Invalid image dimensions"、返回空字节,导致生成崩溃。而本里程碑的验收恰是**iOS 模拟器硬验收**,故 `pro_image_editor` 不满足验收门。

改为**自绘裁剪器**:固定比例取景框 + 拖动/双指缩放图片(cover),用 `image` 包按选区在**定向后(旋转/翻转后)图像坐标**裁字节——**纯 CPU、无截图**,模拟器与真机一致,彻底消除这一类捕获崩溃。它还能主题化(吃四屏 tokens),并与拼豆网格天然对齐:裁剪选的比例传给生成页,锁定豆数长宽比,避免用户填出与裁剪不符的错比例。

属 ROADMAP「Post-M9 — Mobile UI Refinement」三工作流中的裁剪器升级一项。

## 变更内容

- **BREAKING**(App 内部依赖 + 裁剪屏实现):`apps/mobile` **移除** `crop_your_image`,**不引入** `pro_image_editor`,改为自绘 `CropFrame` widget;**新增依赖 `image`**(纯 Dart 图像处理,做 `copyCrop`/`copyRotate`/`flipHorizontal`)。
- **裁剪交互**:固定比例取景框(框不动)+ 拖动/双指缩放图片到位;**最小缩放 = 盖满取景框(cover)**,只允许放大、平移被夹住(框内不留空);框外压暗 + 九宫格 + 四角高亮辅助构图。
- **比例菜单**:默认**正方形**;「比例」按钮弹出菜单选 **2:3 / 3:4 / 4:5 / 9:16**,加**纵/横切换**(横向 = 3:2 / 4:3 / 5:4 / 16:9)。工具行为 **比例 · 旋转 · 翻转 · 重置** 并列。
- **旋转 / 翻转(v1)**:`image` 包 `copyRotate` / `flipHorizontal`,同样非截图。
- **出图**:确认时先应用旋转/翻转、再按取景框在**定向后图像坐标**算裁剪矩形(夹取在图内)→ `image` `copyCrop` → 编码 PNG 字节 → 存既有 `croppedImageProvider`(`Uint8List` 契约不变)→ `/generate`。
- **比例 → 生成页**:选中比例随裁剪经 `cropAspectProvider`(默认正方形)传给生成页,生成页**两输入互算锁定宽×高长宽比**、预设按比例筛选,用户不能再填错比例。**这是正确性要求而非仅 UX**:引擎对传入字节仍 `crop_center` 到 `width:height`——比例不符会被引擎**再裁一次、丢掉用户构图**;锁定使引擎 crop_center **至多裁 <1 豆(近似无操作)**、保住取景。

### 非目标

- 不做四屏整体重写 / 深色模式 / 完整 design tokens(属另一工作流,仅裁剪屏 + 生成页比例约束在本变更)。
- 不改 `bead-core` 引擎、不改 `bead-ffi` 边界、不加 FFI 选项。
- 不做自由裁剪(所有比例均为固定档,正是为了与拼豆网格对齐)。
- 不引入 `pro_image_editor` / `image_cropper`。

## 功能 (Capabilities)

### 新增功能
<!-- 无新增功能:本变更修改既有 mobile-app 契约。 -->

### 修改功能
- `mobile-app`:①「裁剪在调桥之前于 Dart 侧完成」——实现从 `crop_your_image` 改为自绘 `CropFrame` + `image` 包裁剪,新增固定框/拖动缩放(cover)/比例菜单(含横纵)/旋转/翻转;核心契约(裁剪后字节而非矩形、引擎仍 crop_center+resize)保留,但「壳只消费编辑器字节、不做像素级算法」放宽为「壳可用 `image` 包按用户选区裁剪」。②「目标尺寸由用户在生成页指定」——生成页尺寸受裁剪比例**约束**(锁长宽比),不再自由填任意比例。

## 影响

- **代码**(仅 `apps/mobile`):
  - 新增 `lib/presentation/crop_frame.dart`(或在 `crop_page.dart` 内)——自绘取景框 + 手势(`InteractiveViewer`/`Matrix4`)+ cover 夹取 + 蒙版/网格/角标;比例菜单;工具行。
  - `lib/presentation/crop_page.dart` 重写为承载 `CropFrame`;确认时先旋转/翻转、再在定向后图像坐标算矩形 → `image` `copyCrop` → `croppedImageProvider`。
  - `lib/presentation/session_providers.dart` 新增 `cropAspectProvider`(存选中比例);`croppedImageProvider` 契约不变。
  - `lib/presentation/generate_page.dart` 读 `cropAspectProvider`,锁定宽×高长宽比 / 调整尺寸预设。
  - `pubspec.yaml` 移除 `crop_your_image`,新增 `image`(固定版本)。
- **bead-core / bead-ffi**:零改动。**里程碑**:Post-M9(裁剪器升级)。
- **确定性**:不涉及引擎、不改 FFI。裁剪(含旋转/翻转)是**用户输入**;`image` 包对**同一源字节 + 同一矩形/变换**产出确定字节;引擎对同一裁剪字节仍确定性输出。「CLI == FFI」闸门不受影响。
- **依赖**:**替换**——移除 `crop_your_image`,加入 `image`(纯 Dart,pubspec.lock 现为 **4.9.1**)。`image` 目前**仅由 `crop_your_image` 传递引入**(`image_picker` 引入的是平台包、非 `image`),故这是一次**干净替换**:去掉 `crop_your_image` 会使 `image` 失去引入者,本变更把它提升为 `apps/mobile` 直接依赖并保留——**净体积反而下降**(移除 crop_your_image 及其它传递依赖,保留 image)。精确钉版本在实现首步按 lockfile 定。理由:`crop_your_image` 无旋转/翻转;`pro_image_editor` 的 `toImage` 捕获在 iOS 模拟器上失败、不满足模拟器验收;`image_cropper` 是原生 UI、吃不到主题、翻转不保证。自绘 + `image` 包裁剪是唯一同时满足「可主题 + 模拟器可用 + 契合拼豆比例约束」的方案,且 stdlib / 平台原生均无法一次满足。
