## 1. 设计系统(tokens + 深色)

- [x] 1.1 新增 `apps/mobile/lib/presentation/theme.dart`:pegboard tokens(accent `#6C4BF4` / secondary `#12A594` / ink `#1C1830` / ground `#F4F3F7` / line `#E6E3EF`)定义 **light 与 dark** 两套 `ThemeData`(圆润珠状控件圆角、卡片/输入/按钮样式统一);数据文本用等宽 `fontFamily`(平台 monospace,不新增字体资源)。
- [x] 1.2 `main.dart`:`MaterialApp.router` 套 `theme` / `darkTheme` / `themeMode: ThemeMode.system`,移除 `ColorScheme.fromSeed(deepPurple)`。

## 2. 选项数据流(壳只透传,不含算法)

- [x] 2.1 `generate_page` 以**本地组件状态**持有三项选项 `maxColors: int?` / `despeckle: int?` / `generator`(默认 `null / null / staged`,与既有 `_width`/`_height` 同款,**不引 session provider**——设置与生成在同一 widget、无跨屏需求)。
- [x] 2.2 `pattern_engine.dart` **去掉硬编码** `null/null/staged`,`PatternEngine.generate` 与 `application/generate_pattern.dart` 的 `GeneratePattern.call` **各加三个默认 `null/null/staged` 的可选形参**并原样透传给下一层(默认值使既有 4 参调用点—`integration_test/` 的 e2e 与 `test/` 的单测—不必改)。壳不实现任何减色/祛斑/生成算法(CLAUDE 规则 4)。**并给 `PatternEngine` 加一个可注入的桥函数依赖**(如 `const PatternEngine({GenerateFn gen = ffi.generate})`,默认真实桥),让 5.1 能注入替身桥、把断言真正落在 `ffi.generate` 的入参上——否则唯一 DI 点 `patternEngineProvider` 会整体替换 `PatternEngine`,测不到 `PatternEngine → ffi.generate` 这一「去硬编码」跳。
- [x] 2.3 **关键接线**:`generate_page._generate()`(现调 `.call(imageBytes, paletteJson, width, height)`)**读出 2.1 的本地状态三项并转发**进 `.call(...)`——这是让选项从「死控件」变「活」的那根线(默认可选参数不会强制它,故必须显式加、并由 5.1 测试守住)。

## 3. 设置页(生成页扩展)

- [x] 3.1 `generate_page.dart` 扩为设置页:保留尺寸输入 + **既有比例锁定/越界等比缩小契约不变**;新增三控件——`generator` 分段(staged/Gerstner)、`max_colors`(开关+数值/预设,关=`null`)、`despeckle`(数值,**关 ⇒ `null`,不是 `0`**);数值输入**约束为可表示的 `u32`(非负、在范围内)以免 FRB `putUint32` 编码失败**,但**不做业务校验**(是否 ≤N 由引擎判)。既有 1..=1000 守卫与错误展示不变。

## 4. 四屏套用 tokens 重塑

- [x] 4.1 `home_page.dart`:套 tokens 重绘(选图入口)。
- [x] 4.2 `crop_page.dart` / `crop_frame.dart`:`CropFrame` 全套主题化(取代此前「轻度套主题」),蒙版/网格/角标/工具行取 tokens。
- [x] 4.3 设置页(3.1)套 tokens 呈现。
- [x] 4.4 `result_page.dart`:preview + stats/legend + summary 套 tokens、豆号/豆数用 mono;stats/legend/summary 仍**逐字取自 `GenerateOutput`**,绝不从渲染图反推。

## 5. 验证

- [x] 5.1 **壳级转发测试**:经 2.2 的可注入桥函数注入一个**替身桥**,断言当某项设为**非默认**(如 `generator=gerstner`、`max_colors=24`)时,三者**原样抵达桥函数入参**(即覆盖 `_generate` → `GeneratePattern.call` → `PatternEngine.generate` → 桥 全链,含去硬编码跳);**外加**「三项未设 ⇒ 传 `null/null/staged`」。**只测未设路径不够**——控件全死/漏接线时它仍通过。`flutter analyze` 无新增告警、`flutter test` 通过。
- [x] 5.2 **深色接线自动测试**:widget 测试覆写 `platformBrightness`,断言 `darkTheme != null`、dark ColorScheme ≠ light、`themeMode == system` 且随系统切换生效。深/浅**对比度靠人工核**(untestable acceptance)。
- [x] 5.3 iOS 模拟器端到端:选图→裁剪→设置(设一项非默认选项,如 `generator=gerstner` 或 `max_colors`)→生成→预览成功;`bead-core`/`bead-ffi` 零改动、确定性不变、Result 取自 `GenerateOutput`。
