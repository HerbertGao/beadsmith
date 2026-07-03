## 上下文

`crop_your_image` 无旋转/翻转。`pro_image_editor` 经实测在 iOS 模拟器上崩:它用 `RepaintBoundary.toImage` 截图捕获,`toImage` 在模拟器软件渲染器上抛 "Invalid image dimensions" → 空字节 → 引擎解码崩溃(`PanicException`)。本里程碑验收是**模拟器硬验收**,故截图式方案不可行。

自绘裁剪器把裁剪做成「固定框 + 拖动缩放图片」,确认时先应用旋转/翻转、再按框在**定向后图像坐标**用 `image` 包 `copyCrop` 裁字节(见决策 2)——**纯 CPU、无渲染器依赖**,模拟器/真机一致。并顺势解决拼豆比例一致性:裁剪比例传给生成页锁定豆数长宽比。

架构约束:交给引擎的仍是裁剪后**字节**(设计 D5);引擎照旧 `crop_center` + resize;壳不重编排引擎流程(CLAUDE 规则 4);裁剪依赖只在 `apps/mobile`。

## 目标 / 非目标

**目标:** 固定框 + 拖动/缩放(cover 下限)+ 比例菜单(含纵横)+ 旋转/翻转,`image` 包裁字节;裁剪比例锁定生成页尺寸;可主题化;模拟器可跑通。

**非目标:** 四屏整体重写;引擎/FFI 改动;自由裁剪;`pro_image_editor`/`image_cropper`。

## 决策

**决策 1:自绘裁剪器,不用现成包。**
- 替代 A:`pro_image_editor`。否决:`toImage` 截图在 iOS 模拟器失败,不满足模拟器验收(本变更的直接动因)。
- 替代 B:`image_cropper`(原生 uCrop/TOCropViewController)。否决:原生 UI 吃不到主题、翻转不保证、文件路径数据流。
- 选自绘 + `image` 包:唯一同时满足 可主题 + 模拟器可用 + 契合拼豆比例约束。

**决策 2:出图用 `image` 包 `copyCrop`,绝不用 `toImage`;坐标空间钉死。**
**变换顺序(钉死)**:确认时先把旋转(90° 档)/翻转应用到解码后的源图——**顺序钉为先 `copyRotate` 后 `flipHorizontal`**(90° 旋转 + 水平翻转两种先后结果不同,须与视图预览用同一顺序)——得到「定向后图像」,**再在定向后图像的像素坐标系**里把取景框可视区逆映射为矩形 `Rect(x,y,w,h)`,`copyCrop` 于该图。即**矩形与 copyCrop 处于同一坐标空间(旋转/翻转之后)**,避免「矩形在原图坐标、裁剪在旋转后图」的错位。矩形必须**夹取到定向后图边界内**:上界 `x+w`/`y+h` 不越界,**下界 `w,h ≥ 1`**(浮点→整数取整;配合决策 3 的 maxScale 上限,防极端放大算出 0 尺寸令 `copyCrop` 退化/抛错)。
- 替代:截图 `RepaintBoundary.toImage`。否决:渲染器依赖 → 模拟器崩;且截图是屏幕像素而非源图像素,精度受设备缩放影响。
- `copyCrop` 确定、跨平台一致,天然契合「字节进引擎」。

**决策 3:固定框 + 拖动/缩放图片,cover 下限(随旋转/比例重算)。**
框固定(比例=预设),用 `InteractiveViewer`(或手写 `Matrix4` 手势)平移+缩放图片。**minScale = max(框宽/图有效宽, 框高/图有效高)** 使图片始终盖满框;**另设 maxScale 上限**(防极端放大使裁剪矩形算到 0 尺寸,见决策 2 的 `w,h≥1` 下界);平移用 `boundaryMargin`/夹取约束到框内不露空;只允许放大。**旋转 90°/270° 会交换图片有效宽高**,故每次旋转/切比例后必须用新的有效尺寸**重算 minScale 与平移边界并 re-clamp**,否则旋转后可能露空角。
- 替代:拖动裁剪框、图片不动。否决:用户明确要「固定框 + 拖图缩放」范式(头像式),且 cover 语义更简单、无露空态。

**决策 4:比例菜单 + 纵/横切换。**
默认正方形;「比例」按钮弹 sheet:正方形 / 2:3 / 3:4 / 4:5 / 9:16 + 纵/横段控(横向交换长短边)。工具行:比例 · 旋转 · 翻转 · 重置。全固定档、无自由裁剪——每个裁剪都有确定 W:H 供生成页锁定。

**决策 5:旋转/翻转 v1,`image` 包应用。**
UI 上转/翻的是视图;确认时按决策 2 的顺序把旋转(90° 档)/翻转应用到源图(`copyRotate`/`flipHorizontal`)得定向后图像,再在其坐标系 `copyCrop`。90° 档避免非直角旋转的画布扩边复杂度。

**决策 6:比例 → 生成页,`cropAspectProvider` 承载;两输入互算锁定。**
`CropPage` 确认时把选中比例(枚举/分数,如 3:4)写入 `cropAspectProvider`(**默认正方形**,深链绕过裁剪时仍良定义);`GeneratePage` 读它,采用**两输入互算**:用户编辑任一边,另一边按比例自动算出(取整并夹到 1..=1000);尺寸预设按当前比例筛选/派生(正方形锁定下不出现 80×100)。
- 替代:单「长边豆数」输入。否决理由(弱):两输入互算改动更小、更贴近现有 UI,用户可从任一边入手;两者都满足「转发比例=裁剪比例」的正确性契约,取更省改动的。

**决策 7:壳做裁剪属输入准备,不违反 CLAUDE 规则 4。**
规则 4 禁的是在壳内**重编排引擎流程**(resize/match/统计/渲染)。按用户选区裁剪源图是**调桥前的输入准备**(和 image_picker 给字节同层),引擎仍独立 crop_center+resize+match+…。spec 已把「壳只消费编辑器字节」放宽为「壳可用 image 包裁剪」并明记此点。

## 风险 / 权衡

- **坐标换算(载荷逻辑)**:视图变换(pan/zoom/rotate/flip)→ **定向后图像**像素矩形的映射是最易错处 → 缓解:把映射抽成纯函数,**单测**几组已知变换(含缩放、平移、90°旋转、翻转、各比例、极端放大)断言矩形正确、在图内且 `w,h≥1`;它无渲染依赖、可 host 上跑。
- **内存**:`copyCrop` 在**全分辨率**解码图上操作 → 相机图可能较大 → 缓解:在 `image_picker` 取图时用 `maxWidth/maxHeight` 限制源尺寸(引擎最终 resize 到 ≤1000 豆,源无需超高分辨率)。**不在壳内做额外下采样**——分层需求禁止壳做图像缩放;控制源大小只能在 picker 入口,不在裁剪阶段缩放。
- **`image` 包解码**:需解码 picked 字节再 copyCrop → 缓解:`image` 支持常见格式;失败走既有错误路径(展示消息)。
- **旋转档**:仅 90° 档(避免任意角的扩边/黑边)——若将来要任意角,再议。

## Migration Plan

1. `pubspec.yaml`:移除 `crop_your_image`,加 `image`(精确版本),`flutter pub get`。
2. 写 `CropFrame`:固定框 + 手势(pan/zoom,cover 夹取)+ 蒙版/网格/角标;比例菜单(含纵横);工具行(旋转/翻转/重置)。
3. 出图:`image` 解码 → 先 `copyRotate` 后 `flipHorizontal` 得定向后图 → 纯函数在定向后坐标算矩形(夹取、`w,h≥1`)→ `copyCrop` → PNG 字节 → `croppedImageProvider`;写 `cropAspectProvider`。
4. `GeneratePage`:读 `cropAspectProvider`,锁定尺寸长宽比。
5. 单测坐标换算纯函数;`flutter analyze` / `flutter test`;**iOS 模拟器端到端**跑通(现可跑,无 toImage)。
6. 回滚:还原 `crop_page.dart` / `generate_page.dart` / `session_providers.dart` / `pubspec.yaml`;引擎/桥未动。

## Open Questions

- `image` 包的精确版本与 `image_picker` 的 `maxWidth/maxHeight` 取值,实现首步定。
- (生成页锁定形态已定为「两输入互算 + 按比例筛选预设」,见决策 6——待 owner 复核可调,但不改「锁长宽比」正确性契约。)
