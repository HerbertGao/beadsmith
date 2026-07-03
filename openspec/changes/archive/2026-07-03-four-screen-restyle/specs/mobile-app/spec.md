# mobile-app 规范（增量）

## ADDED Requirements

### 需求:生成参数可在设置页调节并透传给引擎

设置页**必须**让用户调节三项引擎选项——减色 `max_colors`(可空,留空=不限)、祛斑 `despeckle`
(可空,留空=不清理)、生成模式 `generator`(`staged` 默认 / `gerstner`)。此处「设置页」即既有
`GeneratePage` / `/generate` 路由屏(加入设置控件后对用户呈现为设置页,**非新增屏**,与「目标尺寸
由用户在生成页指定」是同一屏)。三项选项**必须**以**设置页本地状态**持有,在生成时**原样透传**给 `generate`
(`generate_page._generate` → `GeneratePattern.call` → `PatternEngine.generate` → `ffi.generate`,
沿途各层 Dart 形参加默认 `null/null/staged` 的可选参数并转发)。壳**禁止**实现任何减色/祛斑/生成算法,
也**禁止业务校验**(是否 ≤N 由引擎判);三项只做「取值→转发」。三项**均未设置**时(`max_colors = null`、
`despeckle = null`、`generator = staged`)透传出的 `GenerateOptions` 必须逐字段等价旧的默认路径,使输出
与不带这三项控件时**逐字节相同**(「CLI == FFI 逐字节」闸门不回退)。

#### 场景:选项从设置页转发到 generate
- **当** 用户在设置页设定 `max_colors` / `despeckle` / `generator` 后点击生成
- **那么** App 必须把三者原样作为 `generate` 的对应入参转发,不在壳内改写、不实现减色/祛斑/生成算法

#### 场景:设定的非默认值必须真的抵达桥(防漏接线)
- **当** 用户把某项设为非默认(如 `generator = gerstner` 或 `max_colors = 24`)并生成
- **那么** 该值必须原样出现在**桥函数**的对应入参;为使该断言可测,`PatternEngine` 必须暴露一个**可注入的
  桥函数依赖**(默认 `ffi.generate`),验收测试注入替身桥、覆盖 `_generate → GeneratePattern.call →
  PatternEngine.generate → 桥` 全链(含「去硬编码」跳);**不能只验证「三项未设 ⇒ 默认」路径**——后者在
  控件全死/`_generate` 漏转发时仍会通过,放过「选项形同虚设」这一正是本变更要消灭的缺陷

#### 场景:三项未设时与旧默认路径逐字节一致
- **当** 三项均未设置(`null` / `null` / `staged`)
- **那么** 透传出的 `GenerateOptions` 必须逐字段等价 `{ width, height, ..Default::default() }`,
  输出与引入这三项控件之前逐字节相同

#### 场景:可表示性约束 vs 业务校验(壳只挡编码不合法值,不挡业务)
- **当** 用户在数值控件输入选项值
- **那么** 壳**可**约束输入为**可表示的 `u32`**(非负、在范围内),以免 FRB `putUint32` 在到达引擎前
  编码失败——这是表示性守卫;但壳**不做业务校验**:`max_colors = 0` 必须**抵达引擎**并经既有「桥边界
  扁平化为单一 Dart 异常」报错展示,`despeckle = 0` 是引擎侧合法空操作,壳都不得自行拦截/改写

### 需求:四屏套用设计 tokens 且支持深色模式

四屏(Home / Crop / 设置 / Result)必须套用统一的 *pegboard workshop* 设计 tokens(light 值:accent
`#6C4BF4`、secondary `#12A594`、ink `#1C1830`、ground `#F4F3F7`、line `#E6E3EF`;dark 保留
accent/secondary、翻转中性)取代默认 `ColorScheme.fromSeed(deepPurple)`;豆号/豆数等数据用 mono 字体。
App 必须提供 **light 与 dark** 两套 `ThemeData` 并**跟随系统**(`themeMode: system`)。重塑仅为表现层:
分层(presentation/application/infrastructure)、`bead-core`/`bead-ffi` 零改动、确定性均不受影响;
Result 页的 stats/legend/summary 仍**逐字取自 `GenerateOutput`**,**绝不**从渲染的 preview/grid 图反推
(硬规则)。

#### 场景:深色接线跟随系统(可自动验证)
- **当** 系统在深色与浅色间切换
- **那么** App 必须提供**非空 `darkTheme`**(其 ColorScheme 与 light 不同)且 `themeMode == system`,
  随系统即时切换——此接线可由 widget 测试(覆写 `platformBrightness`)断言

#### 场景:深浅两套均基本可读(人工验收)
- **当** 四屏在深色或浅色下呈现
- **那么** 正文/数据对底色须保持基本可读对比度(目标 ink 对 ground ≥ 4.5:1);此项为**人工验收**,
  非自动化闸门

#### 场景:重塑不动数据来源与分层
- **当** 四屏套用 tokens 重绘
- **那么** Result 的 stats/legend/summary 必须仍取自 `GenerateOutput`(非渲染图),且
  `bead-core`/`bead-ffi` 与确定性不受影响(纯表现层改动)
