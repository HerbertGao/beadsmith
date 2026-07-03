## 1. 依赖切换(apps/mobile)

- [x] 1.1 `apps/mobile/pubspec.yaml`:移除 `crop_your_image`(依赖行 + 其 crop 专属钉版注释,保留通用「精确钉版本」说明),新增 `image`(纯 Dart,**精确钉版本**);`image_picker` 不变。`flutter pub get` 成功。

## 2. 坐标换算(纯函数 + 单测,先做载荷逻辑)

- [x] 2.1 写纯函数:输入(源图宽高、取景框比例、pan 偏移、zoom 缩放、旋转档、翻转标志)→ 输出裁剪矩形 `Rect(x,y,w,h)`,该矩形表达在**应用旋转/翻转后的图像坐标系**(即 `copyCrop` 操作的坐标空间,见 design 决策 2),并**夹取到该图边界内**(上界 `x+w`/`y+h` 不越界;下界 **`w,h ≥ 1`**,防极端放大算出 0 尺寸令 `copyCrop` 退化)。放 `apps/mobile/lib/presentation/crop_geometry.dart`(或同类)。另出一个 `coverMinScale(有效宽,有效高,框比例)` 纯函数供 3.1 复用。
- [x] 2.2 `apps/mobile/test/crop_geometry_test.dart`:单测——恒等、放大 2×居中、平移到角、90° 旋转、水平翻转、各比例(正方形/3:4/9:16 及横向),**外加 zoom+pan+90°旋转+翻转组合用例、以及极端放大用例**;断言矩形正确、始终在图内(不越界)**且 `w,h≥1`**(极端放大不退化)。另断言 `coverMinScale` 在 90° 旋转后按交换后的有效宽高得出、使框仍被盖满。host 上可跑(无渲染依赖)。

## 3. CropFrame widget(交互)

- [x] 3.1 `apps/mobile/lib/presentation/crop_frame.dart`:固定比例取景框(框不动)+ `InteractiveViewer`/手势平移缩放图片;**minScale = 盖满框(cover,用 2.1 的 `coverMinScale`)**、**另设 maxScale 上限**(防极端放大使裁剪矩形退化到 0 尺寸)、`boundaryMargin`/夹取使框内不留空、只允许放大;框外压暗 + 九宫格 + 四角高亮。
- [x] 3.2 「比例」按钮 → 弹出菜单:正方形(默认)/ 2:3 / 3:4 / 4:5 / 9:16 + 纵/横段控(横向交换为 3:2 / 4:3 / 5:4 / 16:9);选定改框比例。工具行:比例 · 旋转 · 翻转 · 重置(轻度套四屏主题)。
- [x] 3.3 旋转(90° 档)/ 翻转 作用于视图状态;**每次旋转/切比例后按交换后的有效宽高重算 minScale 与平移边界并 re-clamp**(否则旋转后露空角);重置回默认。

## 4. 出图 + 比例传递(apps/mobile)

- [x] 4.1 `crop_page.dart` 重写为承载 `CropFrame`;确认时:`image` 解码 picked 字节 → 按 design 决策 2 顺序应用 `copyRotate`/`flipHorizontal` → 用 2.1 在**定向后图像坐标**算矩形 → `copyCrop` → 编码 PNG → `croppedImageProvider` → `context.push('/generate')`。空图守卫保留;**裁剪阶段解码/编码失败必须展示消息、不崩溃、不 `toImage`、不把坏字节传引擎**(注意解码已从生成阶段前移到此)。
- [x] 4.2 `session_providers.dart` 新增 `cropAspectProvider`(存选中比例,**默认正方形**,深链绕过裁剪时仍良定义);确认时写入。`croppedImageProvider` 契约不变。
- [x] 4.3 `generate_page.dart` 读 `cropAspectProvider`,**两输入互算锁定长宽比**:编辑任一边、另一边按比例自动算出(取整并夹 1..=1000);尺寸预设按当前比例筛选/派生(正方形锁定下不出现 80×100)。使转发尺寸比例贴合裁剪比例(整数豆数残差 <1 豆;可吸附到该比例最简整数倍使恰好一致);既有 1..=1000 上界守卫与错误展示不变。

## 5. 清理与测试

- [x] 5.1 移除 `crop_your_image` 所有 import 与残留(`crop_page.dart` 等),确认无编译残留;**改写 `ROADMAP.md` Post-M9「Cropper upgrade」整条**(现写 `pro_image_editor` + `image_cropper` 回退,已作废)→ 自绘裁剪器方案(去掉 image_cropper 回退叙述)。`INIT.md:331` / `ROADMAP:163` 的 M9 栈提及属历史快照,不改(必要时标注为 M9 当时)。
- [x] 5.2 `flutter analyze` 无新增告警;`flutter test`(含 2.2 坐标单测)通过;既有 `@Skip` 原生库依赖测试维持现状。

## 6. 验收(iOS 模拟器现可跑通)

- [x] 6.1 端到端(**iOS 模拟器硬验收**,现无 `toImage` 阻碍):选图 → 裁剪(拖动/缩放 cover + 比例菜单 + 旋转 + 翻转)→ 生成页尺寸受比例锁定 → 生成 → 预览成功;交给引擎的是裁剪后最终字节、壳未截图、未重编排引擎流程。(注:6.1 的生成步骤此前被 FFI 的 iOS SSE panic 阻塞,已由独立变更 `fix-ffi-ios-mirror-response` 修复;二者需一并落地。)
- [x] 6.2 依赖与不变量核对:`image` 只在 `apps/mobile`;`bead-core`/`bead-cli`/`bead-ffi` 依赖集不变;`crop_your_image` 已移除,未引入 `pro_image_editor`/`image_cropper`。确定性:引擎/FFI 未改;同一裁剪后字节的生成产出与相同输入下一致。
