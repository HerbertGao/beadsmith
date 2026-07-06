## 1. Infrastructure 层

- [x] 1.1 新增 `apps/mobile/lib/infrastructure/palette_codec.dart`——`PaletteColor {code,name,rgb}` + `parsePalette(String json)`;解析 `#RRGGBB` 与 `#RGB` 简写;**列表下标与 `BeadPattern.cells[i]` 同序**(对齐 `bead_core::palette::load_palette` 保 JSON `colors` 数组序)
- [x] 1.2 新增 `apps/mobile/lib/infrastructure/album_service.dart`——`AlbumService.saveImage(Uint8List)` 封装 `gal`;`hasAccess`/`requestAccess` 处理权限;权限被拒抛 `AlbumAccessDenied`
- [x] 1.3 在 `apps/mobile/lib/application/providers.dart` 新增 `paletteProvider`(由 `paletteJsonProvider` 派生 `List<PaletteColor>`)、`albumServiceProvider`、`saveToAlbumProvider`

## 2. Application 层

- [x] 2.1 新增 `apps/mobile/lib/application/save_to_album.dart`——`SaveToAlbum` use case,薄包 `AlbumService.saveImage`;保持 application 层不直接依赖 `gal`

## 3. Presentation 层 — 格子视图

- [x] 3.1 新增 `apps/mobile/lib/presentation/bead_grid_view.dart`——`CustomPainter` 按 `cells` + palette 画纯色方块,格间留 gap 透出 line 色形成可见网格线(gap 随 cell 缩放,clamp);`shouldRepaint` 仅 `cells`/`highlightedIndex`/`accent`/`lineColor` 变化时重绘;`highlightedIndex` 描边同色格子
- [x] 3.2 缩放/平移用**自管 `GestureDetector.onScaleStart`/`onScaleUpdate` + `Transform`**(非 `InteractiveViewer`——后者在 Android 模拟器捏合下不响应,与 `CropFrame` 同一套已验证机制);围绕 focal point 缩放(1×–20×)、clamp offset 防拖出屏;`Transform` 作用于「居中 grid + 上下留白」整块 → 相册式手感(放大时留白随 grid 放大消失)
- [x] 3.3 点格命中在 `onTapUp` 对当前 transform 求逆(`scene = (point - offset) / scale` 再减 grid 1× top-left)落到格子坐标,回调 `(row,col,paletteIndex)`;缩放/平移下命中仍精确

## 4. Presentation 层 — 结果页布局

- [x] 4.1 重写 `apps/mobile/lib/presentation/result_page.dart`:`Stack(Column(Expanded(BeadGridView) + AnimatedContainer(_LegendSheet)) + 顶部保存提示)`;`_ResultAppBar`(ConsumerWidget + `PreferredSizeWidget`)含 preview 缩略图(点击 `Dialog`+`InteractiveViewer` 放大)、保存图标、复制图标
- [x] 4.2 配色 `_LegendSheet`:收起=薄栏(header「配色·N 色」+ 迷你色块),点击 header 切 `_legendExpanded`;父层 `AnimatedContainer` 高度动画,`expandedH` = grid letterbox 留白高(动态,`(1-gridHF)*bodyH` clamp),展开时 grid 区 `Expanded` 收缩使 grid 上移贴面板顶边;`Material`(非带背景 `Container`,避免 ListTile ink 警告)
- [x] 4.3 点格 → 底部 detail sheet(色块+code+name+count+行列位置+"高亮同色");"高亮同色"对所有同 `paletteIndex` 格子描边,再触发取消
- [x] 4.4 首次保存提示改为**顶部浮动条**(`Positioned` + `AnimatedOpacity`,文案「建议保存到相册」,`Future.delayed(7s)` 自动淡出 + × 关闭 + 保存快捷键);session 级 `static _saveHintShown`
- [x] 4.5 保存内容用 `GenerateOutput.gridPng`(引擎格子图,带网格线/行列号)而非 `previewPng`;AppBar 缩略图/放大仍用 `previewPng`
- [x] 4.6 底部安全区:不套外层 `SafeArea`,配色面板表面铺满至物理底边,列表内容底部 padding 叠加 `MediaQuery.paddingOf().bottom`,`collapsedH`/`expandedH` 加该 inset;移除「汇总」`SelectableText` 文字块

## 5. 生成页默认尺寸

- [x] 5.1 `apps/mobile/lib/presentation/generate_page.dart` 默认宽/高 50→100(初值 + 种子 height 计算);`_clampSide` 上限 1000 内、预设 `[50,80,100]` 已含 100

## 6. 平台配置与依赖

- [x] 6.1 `apps/mobile/pubspec.yaml` 新增 `gal: 2.3.2`(精确 pin,与 pubspec 既有惯例一致);`flutter pub get`
- [x] 6.2 `apps/mobile/ios/Runner/Info.plist` 新增 `NSPhotoLibraryAddUsageDescription`("将生成的拼豆图案保存到相册"),与既有 `NSPhotoLibraryUsageDescription`(读)分开;Android 无需改 manifest(`gal` 在 API 33+ 免权限,旧版由 `gal` manifest 合并处理)

## 7. 测试

- [x] 7.1 新增 `apps/mobile/test/palette_codec_test.dart`——锁住:① `#RRGGBB` 与 `#RGB` 解析;② `parsePalette` 保 JSON `colors` 序(**对账 `bead_core::palette::load_palette` 保序假设**——若未来 engine 改排序,此测试先红);③ `cells[i]` 下标本列表
- [x] 7.2 新增 `apps/mobile/test/bead_grid_view_test.dart`——① 点击命中数学(2×2 grid 三点各报正确 `(row,col,idx)`);② **双指 pinch → `Transform` scale > 1×**(锁住缩放响应,防退回 `InteractiveViewer`);③ 大 grid 构建无异常
- [x] 7.3 `flutter analyze` 干净;`flutter test` 全绿(30 passed + 2 device-only skipped)

## 8. 端到端验收

- [x] 8.1 Android 模拟器(Pixel_10 / API 37)四屏流程:选图 → 裁剪 → 设尺寸(默认 100×100)→ 生成 → `ResultPage`。核对:grid 主角 letterbox、**双指缩放(Ctrl+拖)相册手感**、拖动平移、点格 detail sheet、"高亮同色"描边、AppBar preview 缩略点击放大、保存格子图到相册、复制 summary、配色面板点击展开使 grid 上移、顶部保存提示 7s 自动消失、底部安全区连续无死白
- [x] 8.2 iOS 模拟器(iPhone 17 Pro / iOS 26.5)同 8.1 流程 + **双指缩放(Option+拖)** + `NSPhotoLibraryAddUsageDescription` 权限弹窗 + Dynamic Island/home indicator 安全区正常
- [x] 8.3 对账:格子视图每格颜色 = `cells[i]` 下标 `parsePalette` 结果,**不从 `previewPng`/`gridPng` 反推**(守 ARCHITECTURE 硬规则 3);保存透传引擎 `gridPng` 不在壳内重绘;同 `image+palette+dimensions+options` 引擎输出逐字节不变(本变更纯 UI/呈现层,不碰 `pipeline::generate_pattern`、不碰 FFI 边界、不碰 golden)
