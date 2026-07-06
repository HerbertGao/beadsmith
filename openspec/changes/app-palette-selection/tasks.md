## 1. 打包色卡与依赖

- [x] 1.1 把 13 份 clean 色卡逐字节拷入 `apps/mobile/assets/palettes/`(`mard`、`artkal_a`、`artkal_c`、`artkal_m`、`artkal_r`、`hama`、`hama_maxi`、`hama_mini`、`perler`、`perler_caps`、`perler_mini`、`nabbi`、`yant`;`artkal_s` 已在);**不拷** `palettes/_unlicensed/` 任何文件
- [x] 1.2 `apps/mobile/pubspec.yaml`:assets 段**逐个显式列举** 14 份 `assets/palettes/<name>.json`(**禁止**用 `assets/palettes/` 目录通配——目录通配会把误拷入的 `_unlicensed` 文件随目录一并打包);dependencies 加 `shared_preferences`(精确 pin,与仓库既有 pin 风格一致)
- [x] 1.3 加一个纳入 `flutter test` 的 asset 校验(`apps/mobile/test/palette_assets_test.dart`):① 用 `dart:io` `Directory('assets/palettes').listSync()` 枚举**磁盘真实文件**(**禁止**用 `AssetManifest`/`rootBundle`——它只见已声明 asset,看不到误拷入的未声明文件),断言集合**恰等于** 14 个 clean 注册表 id(**双向**:无缺、无多余、无 `_unlicensed` 路径);② 每份与顶层(相对 `apps/mobile` cwd 的 `../../palettes/<name>.json`)**逐字节相同**(守 CLI==FFI);③ 加「测试该测试」诱饵自检——**两个独立扰动各断言校验失败**:(a) 多放一个文件 ⇒ 集合不等、失败(证「无多余」方向有效);(b) 抽掉一个必需 id ⇒ 集合不等、失败(证「无缺」方向有效)。**必须**是运行于 `flutter test` 的测试,不得是无人调用的脚本

## 2. 色卡注册表与可选调色板 provider

- [x] 2.1 新增显式注册表 `apps/mobile/lib/infrastructure/palette_registry.dart`:每项 `{id, brand 展示名, asset 路径}`,固定顺序 `MARD → Artkal S/A/C/M/R → Hama Midi/Maxi/Mini → Perler/Caps/Mini → Nabbi → Yant`;默认 id = `mard`。`brand` 入注册表以便色卡行/弹窗**首帧同步**显示(`parsePalette` 不产出 brand);加测试断言每项 `brand` 与对应 JSON 的 `brand` 字段一致
- [x] 2.2 `apps/mobile/lib/application/providers.dart`:`paletteJsonProvider` 由固定读 `artkal_s` 改为读**选中 id** 对应 asset(**保持普通 `FutureProvider<String>`,不要改成 `.family`**——否则既有 `option_forwarding_test` 的 `.overrideWith` 会失效);选中 id 不在注册表时回落 `mard`
- [x] 2.3 弹窗「N 色」数据 provider:惰性加载注册表各 asset,产出每项色数(`colors.length`)供列表展示(色数不硬编码,防漂移;`brand` 已由注册表同步提供,无需解析)

## 3. 持久化设置模型

- [x] 3.1 定义 `GenerateSettings` 值对象(`paletteId` / `generator` / `limitColors`+`maxColors` / `despeckleOn`+`despeckle` / `width` 宽维度豆数)+ 默认常量(`mard`/`staged`/关/关/`100`)
- [x] 3.2 `shared_preferences` 支撑的 Riverpod Notifier:**`main()` 中 `await SharedPreferences.getInstance()` 并经 provider override 注入,使 Notifier 在 `ProviderScope` 构建前即就绪、首帧同步读持久值**(故**无未就绪窗口**,不需另建异步 gate;不用「先默认后覆盖」);任一字段改动即写;无持久值时用默认
- [x] 3.3 高不持久化:`GeneratePage` 进入时按当次 `cropAspectProvider` + 既有 `lockedGridPair` 从持久化的 `width` 同步重推 `height`(越界时沿用既有等比缩小,宽不再恰等持久值)。**重推 / 越界产生的宽不回写持久化**——「任一字段改动即写」仅指用户**显式编辑**,播种/重推不算(见 spec 持久化需求)

## 4. 设置页:色卡行 + 底部弹窗 + 接线持久化

- [x] 4.1 `apps/mobile/lib/presentation/generate_page.dart`:三项引擎选项 + 宽由局部 `setState` 字段改为读写组 3 的持久化模型(交互契约不变);`initState` 用持久宽单次播种 + 单次推高,不做二次异步改写
- [x] 4.2 新增「色卡」行(显示当前色卡 `brand`,取自注册表);点击经**普通 Material `showModalBottomSheet`**(两端一致——Flutter 无 adaptive bottom-sheet 构造器,底部弹窗不属既有 iOS 自适应枚举〔开关/进度/分段〕、亦非其对话框条款,与 crop/结果详情弹窗同款)弹出列表,列全部内置色卡(`brand` + 「N 色」),选中项打勾;选定后即写持久化并关窗、行文案更新
- [x] 4.3 `_generate` 生成时取选中色卡的 `paletteJson`(经 `paletteJsonProvider.future`)传 `generate`(透传链与既有一致)

## 5. 结果钉住生成时色卡

- [x] 5.1 结果承载新增 `paletteJson`:`_generate` 把**实际传给 `generate` 的那份** `paletteJson`(不重读)连同 `GenerateOutput` 一起存入 `generateResultProvider`(Dart 侧包装 `{output, paletteJson}`,**不改** FFI `GenerateOutput` 类型);`generateResultProvider` 的 Notifier 类型由 `GenerateOutput?` 改为包装体 `{output, paletteJson}?`
- [x] 5.2 `apps/mobile/lib/presentation/result_page.dart`:配色面板/格子/点格详情改**同步**解析钉住的 `paletteJson`(去 `.when` loading/error 分支),**不再** `ref.watch(paletteProvider)` 实时值
- [x] 5.3 **删除孤儿** `paletteProvider`(`providers.dart:42`)——钉住后其唯一消费者(`result_page.dart:30`)已移除;`grep` 确认无其它引用后删除

## 6. 验证与对账

- [x] 6.1 单元/widget 测试:选某色卡→生成时该色卡 `paletteJson` 抵达**桥**(复用既有可注入替身桥断言链;**用两个不同的真实色卡断言、禁止把 `paletteJsonProvider` override 成常量**,否则空转);默认色卡 = MARD;持久 id 失效回落 MARD。**所有 pump GeneratePage 的 widget 测试须先 `SharedPreferences.setMockInitialValues({...})` + prefs provider override**,否则 prefs-backed Notifier 抛 `MissingPluginException`——**含回改既有** `apps/mobile/test/option_forwarding_test.dart`(它 pump GeneratePage 4 次)
- [x] 6.2 持久化测试:改配置→**传播第一个 container 的写入**到重建的第二个 container(而非直接给第二个 container 播种断言值,否则空转)→各项保留;就绪后 `height == lockedGridPair(持久width, 当次aspect)`;高按当次裁剪比例重推(含越界等比缩小);**断言在越界比例下进入设置页再离开,持久的宽不变**(越界重推产生的宽不回写);首次启动(无持久值)回落默认
- [x] 6.3 钉色卡测试:以色卡 A 生成→改选 B 且**不**重新生成→`result_page` 解析仍用 A(颜色/code/name/count 不变)。**A、B 须为两个颜色不同的真实色卡**(如 MARD/Hama),**禁止**常量 override 或相同 A/B(否则断言空转)
- [x] 6.4 `flutter analyze` + `flutter test` 全绿(含组 1.3 asset 集合+字节校验);`grep` 确认 `paletteProvider` 已无引用
- [x] 6.5 iOS 模拟器手验(对应 ROADMAP M10「像成品」验收):选 MARD 生成→结果页色号为 MARD;生成后改色卡不污染已有结果;杀 App 重开后设置页配置(色卡/模式/限色/去斑/宽)保留
