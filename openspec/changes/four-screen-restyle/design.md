## 上下文

当前 `apps/mobile` 主题只是 `ThemeData(colorScheme: ColorScheme.fromSeed(seedColor:
Colors.deepPurple))`,无 tokens、无深色模式;四屏是默认 Material 皮。`widen-ffi` 已在 FFI 边界
开放 `max_colors` / `despeckle` / `generator`,但 `pattern_engine.generate` 仍**硬编码**
`maxColors: null, despeckle: null, generator: staged`——UI 无从调节(ROADMAP 记为「Settings
控件在 FFI 落地前是死的」)。本变更是 Post-M9 三工作流的收尾。

## 目标 / 非目标

**目标:** 四屏套 pegboard tokens + light/dark 跟随系统;豆号/豆数 mono;把三项引擎选项作为控件
落到设置页(生成页扩成)并透传;`bead-core`/`bead-ffi` 零改动、确定性不变、Result 仍从
`GenerateOutput` 派生。

**非目标:** 改引擎/FFI/`generate` 签名;新引擎算法;新增导航屏;多语言;无障碍全面改造;
裁剪器几何改动。

## 决策

**决策 1:tokens 集中在 `lib/presentation/theme.dart`,不散落;light/dark 两套值。**
一处定义 light + dark 两套 `ThemeData`,`main.dart` 用 `theme` / `darkTheme` / `themeMode: system`;
圆润珠状控件靠 `ThemeData` 的 shape/卡片圆角统一,不逐 widget 手调。
- **light**:accent `#6C4BF4` / secondary `#12A594` / ink `#1C1830` / ground `#F4F3F7` / line `#E6E3EF`。
- **dark**:保留 accent `#6C4BF4` / secondary `#12A594`,**翻转中性**——ground→深(如 `#141019`)、
  surface 略高于 ground(如 `#1E1830`)、ink→浅(如 `#ECEAF2`)、line→低对比深灰(如 `#2A2436`)。
  两套均以正文/数据(ink)对 ground **≥ 4.5:1** 为目标;具体十六进制实现时微调。

**决策 2:设置页 = 扩展的生成页,不新增屏。**
保持四屏流 Home→Crop→设置→Result。生成页在原有尺寸(比例锁定不变)之上加三控件,避免多一层导航。

**决策 3:选项数据流——本地组件状态(不用 session provider),配「设定值抵达桥」测试。**
三项选项(`maxColors: int?` / `despeckle: int?` / `generator`)作为**设置页(= `GeneratePage`)的
本地组件状态**(与既有 `_width`/`_height` 同款——设置与生成在同一 widget,无跨屏需求,故不引 session
provider)。`PatternEngine.generate` 与 `GeneratePattern.call` **各加三个默认 `null/null/staged` 的
可选形参**(默认让既有 4 参调用点不改),`generate_page._generate()` 把本地状态三项**读出并转发**进
`.call(...)`;`pattern_engine` **去掉硬编码**。壳只「取值→转发」,不实现任何算法(CLAUDE 规则 4)。
**因可选默认不会强制转发,必须加壳级测试**:断言某项设为非默认时它**原样抵达桥**(不能只测「未设=默认」——
那条在控件全死/漏接线时仍通过,正是本变更要消灭的旧 bug)。**测试可行性需一个注入缝**:`PatternEngine` 暴露
可注入的桥函数依赖(如 `const PatternEngine({GenerateFn gen = ffi.generate})`),替身注入其处——否则唯一 DI 点
`patternEngineProvider` 整体替换 `PatternEngine`、测不到 `PatternEngine → ffi.generate` 这一去硬编码跳。

**决策 4:选项控件 UX + 可表示性(非业务校验)。**
- `generator`:分段控件 `逐级(staged)` / `Gerstner`(默认 staged)。
- `max_colors`:「限制颜色数」开关 + 数值/预设片(如 24/36/48/72);关 ⇒ `null`(不限)。
- `despeckle`:「去斑」数值;**关 ⇒ `null`(不是 `0`)**——ADDED 规范的「逐字段等价 `Default`」要求
  `despeckle = None`,`Some(0)` 虽是合法空操作但**非** field-identical(会破坏该字面契约)。
- **可表示性约束(≠ 业务校验)**:数值输入约束为可表示的 `u32`(非负、在范围内),避免 FRB `putUint32`
  编码在到达引擎前失败;但**不做业务校验**——是否 ≤N 由引擎判,`max_colors = 0` 照样抵达引擎报错、
  `despeckle = 0` 是引擎侧合法空操作。
默认全关/staged,使**未设路径逐字节等价旧默认**(验证见风险的确定性 + 决策 3 的抵达测试)。

**决策 5:mono 字体走平台等宽,不新增字体资源。**
数据类文本(豆号/豆数)用等宽 `fontFamily`(平台 monospace 回退),避免打包字体资源、保持轻。
若将来要品牌等宽字体再议。

**决策 6:深色仅跟随系统,v1 不做手动切换。**
`themeMode: ThemeMode.system`,最小面。手动切换/持久化偏好另议。

## 风险 / 权衡

- **确定性回退风险**:三项未设时必须与旧默认**逐字节**一致。缓解:壳级测试断言「全未设的透传 ==
  `null/null/staged`」**并**「非默认值原样抵达桥」(决策 3);`widen-ffi` 的边界测试已覆盖引擎侧字节。
- **漏接线 = 选项仍死(前向脆弱)**:可选默认参数下,若 `_generate` 忘了转发,控件写了没人读、选项
  仍是死的(正是本变更要消灭的旧 bug),且只测「未设=默认」的测试会误绿。缓解:决策 3 的「设定值
  抵达桥」测试就是防这一条(任务 2.3 + 5.1)。
- **Gerstner 上采样悬崖**:开放 `generator = gerstner` 引入一个新失败面——豆数网格 > 裁剪源尺寸时
  命中引擎 `target ≤ source` 守卫(`BeadError::InvalidImage`),`staged` 则成功。**非崩溃**,经既有
  边界扁平化 → 生成页错误展示;本轮**只走既有错误展示、不加预警功能**(预警属新功能、超范围)。
- **深色对比度**:自动化仅能验接线(`darkTheme` 非空 / `themeMode: system` / dark≠light);对比度本身
  靠人工核 + tokens 选值(目标 ink 对 ground ≥ 4.5:1)。
- **重塑面广**:限定在 tokens + 四屏 + 设置页三控件,**不**新增导航/功能;裁剪器仅套主题、不动几何。
- **mono 平台差异**:平台 monospace 字形不一,仅影响观感,不影响数据正确性。

## Open Questions

- 选项控件的具体档位(`max_colors` 预设值集、`despeckle` 上限)实现时定,可与 CLI 档位对齐。
- 是否需要手动深色切换 / 偏好持久化——本轮 system-only,后续按需。
