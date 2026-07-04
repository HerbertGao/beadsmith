# mobile-app 规范

## 目的
定义 `apps/mobile` 的离线 Flutter MVP：四屏导航、调桥生成、裁剪前置、默认调色板内置和结果复制，确保用户在无网络环境下完成 INIT 闭环。
## 需求
### 需求:四屏离线导航流

`apps/mobile` Flutter App 必须实现 `HomePage → CropPage → GeneratePage → ResultPage` 四屏导航流,
用 `go_router` 声明式路由。全流程必须**完全离线**:禁止任何网络请求、图片上传或后端调用;所选图片
仅在设备本地处理(INIT「Local First」)。图片来源经 `image_picker` 从系统相册/文件选取。

#### 场景:走通四屏
- **当** 用户从 `HomePage` 选取一张图片
- **那么** App 必须依次经 `CropPage`(裁剪)、`GeneratePage`(设置尺寸并生成)抵达 `ResultPage`(查看结果),
  导航由 `go_router` 驱动

#### 场景:全程无网络
- **当** 设备处于飞行模式/无网络
- **那么** 选图→裁剪→生成→预览→复制 summary 全流程必须照常完成,App 禁止发起任何网络/上传请求

### 需求:分层(presentation/application/infrastructure)且壳不含算法

App 必须按 presentation / application / infrastructure **三层**组织(MVP)。ARCHITECTURE.md「Flutter
Architecture」整段(全 4 层)标注「Future implementation」;其中 domain 实体为 `Project`/`Palette`/`Pattern`
(`SaveProject` 属 application 用例,非 domain)。本里程碑无持久化/`SaveProject`,domain 会是空透传层,故
**留到持久化落地时再建**(YAGNI),壳不因此承载算法。引擎调用必须经 infrastructure 层的 `PatternEngine` 封装 `bead-ffi` 的 `generate`
入口。壳可做 UI 裁剪(见「裁剪在调桥之前」需求),但**禁止在壳内实现或重新编排** resize / match / 统计 /
渲染 任何一步(CLAUDE 规则 4),也禁止从渲染图反推统计——预览、配色计数、summary 全部直接取自
`GenerateOutput`。壳禁止引入任何随机性。当 App 在 **host 整数/结构路径**上与 `bead-cli` 对**相同输入字节**
不一致时,**视为壳的 bug**(CLAUDE 规则 5);iOS 与 host 间的浮点路径差异属 Rule 3 跨目标 caveat,不算壳 bug。

#### 场景:经 PatternEngine 调桥,不自行编排
- **当** `GeneratePage` 触发一次生成
- **那么** application 层用例必须经 infrastructure 的 `PatternEngine` 调用 `bead-ffi` 的
  `generate({imageBytes, paletteJson, width, height})`,壳内禁止出现任何图像缩放/配色匹配/统计/渲染逻辑

#### 场景:结果只从 GenerateOutput 派生
- **当** `ResultPage` 展示预览、配色计数与 summary
- **那么** 预览必须用 `GenerateOutput.previewPng`、计数必须用 `GenerateOutput.stats`、summary 必须用
  `GenerateOutput.summary`,禁止从渲染图重新统计或在壳内重算

### 需求:裁剪在调桥之前于 Dart 侧完成

手动裁剪必须由 `CropPage` 用**自绘的 `CropFrame` widget** 在**调用桥之前**完成,把裁剪后的图像字节交给引擎;**禁止**使用截图式捕获(`RepaintBoundary.toImage` / `Picture.toImage`)——它在 iOS 模拟器软件渲染器上会失败("Invalid image dimensions"),而本里程碑验收是模拟器硬验收。裁剪屏必须提供:**固定比例取景框**(框不动)+ **拖动/双指缩放图片**;**最小缩放 = 盖满取景框(cover)**,即图片长或宽任一边缩到等于框即到底、只允许放大,平移被夹住使框内不留空;**比例菜单**——默认**正方形**,可选 **2:3 / 3:4 / 4:5 / 9:16** 并有**纵/横切换**(横向 = 3:2 / 4:3 / 5:4 / 16:9);**旋转**与**翻转**。确认时,壳必须**先**对解码后的源图应用已选的 `copyRotate` / `flipHorizontal`(**顺序:先旋转后翻转**,二者不可交换,须与视图预览同序;见 design 决策 2)得到「定向后图像」,**再**按取景框在**定向后图像的坐标系**(即 `copyCrop` 操作的坐标空间)算裁剪矩形并夹取在该图边界内,`copyCrop` 裁出最终字节,编码为 PNG(顺序钉死:旋转/翻转 → 定向后坐标算矩形 → copyCrop;不得在源图坐标算矩形却在旋转后图裁剪)。`bead-core` 不含裁剪 UI;引擎收到的就是**裁剪(及旋转/翻转)后的最终字节**,其内部仍按既有 `crop_center` + resize 处理。**架构澄清(契约放宽)**:此前「壳只消费编辑器产出的 `Uint8List`、不做任何像素级裁剪算法」放宽为——壳**可**用 `image` 包按用户选区做裁剪/旋转/翻转(属**调桥前的输入准备**,确定性、非引擎流程);壳仍**禁止**重新编排引擎的 resize/match/统计/渲染(CLAUDE 规则 4 不变),也禁止从渲染图反推统计。交给引擎的必须是**最终字节**而非裁剪矩形,并写入既有 `croppedImageProvider`(`Uint8List` 契约不变)。**依赖隔离**:新增的 `image` 包只能出现在 `apps/mobile`,禁止污染任何 crate;`crop_your_image` 随本变更从 `apps/mobile` 移除,且**不引入** `pro_image_editor` / `image_cropper`。

#### 场景:交给引擎的是裁剪后最终字节(非截图)
- **当** 用户在 `CropFrame` 拖动/缩放定位(可含旋转/翻转)并确认
- **那么** App 必须先应用旋转/翻转、再按取景框在**定向后图像坐标**算矩形、`copyCrop` 裁出**裁剪后最终图像字节**作为 `imageBytes` 传给 `generate`,不得用 `toImage` 截图,壳内不重新编排引擎流程;字节存入 `croppedImageProvider`

#### 场景:cover 缩放下限(随旋转/比例重算)
- **当** 用户缩小图片到长或宽任一边等于取景框,或旋转(90° 档)/切换比例改变了有效朝向
- **那么** 必须**禁止**继续缩小(只允许放大),且平移被夹住使取景框内**始终被图片盖满、不露空**;**90°/270° 旋转会交换图片有效宽高,故 cover 最小缩放与平移边界必须按旋转后的有效尺寸/比例即时重算**,使旋转后仍盖满、不出现空角

#### 场景:裁剪矩形夹取在定向后图像内(防越界)
- **当** 确认裁剪、把视图变换换算为定向后图像的裁剪矩形
- **那么** 该矩形必须在**应用旋转/翻转后的图像坐标系**(即 `copyCrop` 操作的坐标空间)内计算,并**夹取到该图边界内**(处理浮点→整数取整),使 `copyCrop` 的 `x+w`/`y+h` 绝不越界(上界);同时必须保证 **`w,h ≥ 1`**——设 maxScale 上限,防极端放大把矩形算到 0 尺寸令 `copyCrop` 退化/抛错(下界)

#### 场景:比例菜单(含纵/横)
- **当** 用户点「比例」
- **那么** 弹出菜单必须含 正方形(默认)/ 2:3 / 3:4 / 4:5 / 9:16,并提供纵/横切换(横向对应 3:2 / 4:3 / 5:4 / 16:9);选定后取景框改用该长宽比

#### 场景:旋转与翻转(image 包,非截图)
- **当** 用户点旋转或翻转
- **那么** 变换必须由 `image` 包(`copyRotate` / `flipHorizontal`)在确认时应用到源图字节,壳不自行实现像素旋转/翻转算法、不使用截图

#### 场景:裁剪阶段解码/编码失败展示消息、不崩溃
- **当** picked 图像字节在**裁剪阶段**无法被 `image` 包解码,或裁剪后编码 PNG 失败(注意:解码此前在生成阶段,现前移到裁剪阶段——生成阶段收到的已是重编码后的 PNG)
- **那么** App 必须向用户展示错误消息、**不崩溃、不使用 `toImage`**、不把坏字节传给引擎

### 需求:目标尺寸由用户在生成页指定

`GeneratePage` 必须让用户指定目标拼豆网格尺寸 `width` × `height`(一像素一豆),并把该尺寸作为 `generate` 的 `width` / `height` 入参转发。**目标尺寸的长宽比必须受裁剪比例约束**:`CropPage` 选中的比例经 `cropAspectProvider` 传入,`GeneratePage` 据此**锁定 `width : height` 等于该比例**,用户**不能**再填出与裁剪不符的比例。**这不仅是 UX、更是正确性要求**:引擎对传入字节仍做 `crop_center` 到 `width:height`——若尺寸比例与裁剪不符,引擎会**再次裁剪**、悄悄丢弃用户在裁剪屏框定的构图;锁定使引擎 `crop_center` **至多裁 <1 豆(近似无操作)**、保住用户的取景(整数豆数下比例残差 <1 豆;实现可将豆数吸附到该比例的最简整数倍,使比例**恰好一致、引擎无残差裁剪**)。`cropAspectProvider` 的**默认值必须为正方形**(与裁剪屏默认一致),以便即使 `CropPage` 被绕过/未确认(如深链直达生成页)锁定仍有良定义(当前 40×40 默认在正方形下仍合法)。生成失败时(桥抛出异常)必须向用户展示该异常消息(已在 `bead-ffi` 边界扁平化为可读文案),禁止静默失败或崩溃。**App 必须在调桥前对 `width`/`height` 施加合理上界守卫**(引擎对超大尺寸无上限分配,`w·h·3` 字节的急切分配会触发不可捕获的 alloc abort 崩溃):越界时展示消息、不调用引擎。本里程碑取 `1..=1000`(远超 ROADMAP 最大示例 300×300)。

#### 场景:转发用户尺寸
- **当** 用户在 `GeneratePage` 设定 width × height 并点击生成
- **那么** App 必须以该 width/height 调用 `generate`,不得改写或硬编码尺寸

#### 场景:尺寸长宽比受裁剪比例锁定
- **当** `GeneratePage` 从 `cropAspectProvider` 取到裁剪比例(未设置则默认正方形)
- **那么** width × height 的输入必须被锁定为该长宽比——**用户编辑任一边、另一边按该比例自动算出**;尺寸预设必须**按当前比例筛选/派生**(不得出现违反该比例的预设,如正方形锁定下不出现 80×100);使转发给 `generate` 的尺寸比例贴合裁剪比例(整数豆数下残差 <1 豆),用户无法填出错比例

#### 场景:越界输入整体等比缩小,不夹取单边(否则破坏锁定比例)
- **当** 用户在某一边输入的值,按锁定比例算出的另一边会越过 `1..=1000` 上界(如 9:16 下 width=800 → height=1422)
- **那么** App 必须**整体等比缩小**该 width×height 对使**两边都落在 `1..=1000` 内且锁定比例不变**(残差 <1 豆);**禁止只把越界的一边夹到 1000 而让另一边保持不变**——那会把锁定比例改成别的(如 800×1000=0.8≠0.5625),令引擎 `crop_center` 重新裁剪、丢弃用户取景

#### 场景:生成失败显示消息
- **当** `generate` 抛出异常(如非法尺寸;注意图像解码已前移到裁剪阶段,生成阶段收到的是裁剪已重编码的 PNG,故生成阶段的解码失败基本不再出现,仅深链绕过裁剪等旁路可能触发)
- **那么** App 必须向用户展示该异常的消息文案,且不崩溃

#### 场景:超大尺寸被上界守卫拦截
- **当** 用户输入超出上界(如 `99999`)的 `width` 或 `height` 并点击生成
- **那么** App 必须展示越界消息、**不调用引擎、不崩溃**(引擎无上限分配会先于返回触发 alloc abort)

### 需求:内置默认调色板随 App 离线分发

App 必须把 `palettes/artkal_s.json` 作为 Flutter asset 打包,运行时从 asset 读取调色板 JSON 字符串传给
`generate`;禁止运行时从网络下载调色板。该 asset 必须是无 BOM 的 UTF-8(与 `bead-cli` 消费的同一份逐字节
一致),以维持「CLI == FFI」前提。

#### 场景:从打包 asset 读调色板
- **当** App 需要调色板执行一次生成
- **那么** 必须从打包的 `artkal_s.json` asset 读取 JSON 字符串作为 `paletteJson`,禁止任何网络获取

### 需求:结果页可复制 summary 到剪贴板

`ResultPage` 必须提供把 `GenerateOutput.summary`(INIT「Summary Format」原文)一键复制到系统剪贴板的
能力,复制内容必须是 summary 字符串原文,不在壳内重排或重算。

#### 场景:一键复制 summary
- **当** 用户在 `ResultPage` 点击「复制」
- **那么** 系统剪贴板必须被写入 `GenerateOutput.summary` 的原文字符串

### 需求:满足 INIT 成功标准(端到端离线)

App 必须使用户能完成 INIT.md「Success Criteria」的完整闭环:①选图 ②生成拼豆图案 ③查看预览
④查看配色计数 ⑤复制 summary 文本 ⑥全程无任何后端服务。本需求是 M9「Done when」的离线 App 部分
(商店签名上传不在本变更范围)。

#### 场景:成功标准闭环
- **当** 用户在 **iOS 模拟器**(硬验收;真机 best-effort,需个人开发团队签名)上选图→裁剪→设尺寸→生成→
  预览→查看计数→复制 summary
- **那么** 全部六步必须可完成且全程离线;预览/计数/summary 必须**原样来自该次 `GenerateOutput`**(不重算),
  并满足结构不变量(总豆数〔由 `pattern.cells.length` / `Σ stats.count` 派生〕 = width×height、stats schema、
  summary 为 INIT 格式)。**不要求**与 host `bead-cli`
  逐字节/逐值相等——iOS 与 host 是不同目标/libm(Rule 3 跨目标 caveat);byte-exact「CLI == FFI」由 host 端
  M8 既有决定性测试保证

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

### 需求:iOS 上采用平台自适应控件与转场

App **必须**在 **iOS** 目标上以接近系统的手感呈现少数高信号控件,同时 **Android 保持 Material、无回归**,
且**不改**配色 tokens、分层、确定性与任何交互契约(比例锁定、选项透传、裁剪几何)。**表现层**做控件/转场的
平台分支时**必须**用 `Theme.of(context).platform`(或 `.adaptive` 构造器内建的等价判断),**禁止**用 `dart:io`
`Platform.isIOS`——前者可被 widget 测试覆写、后者取宿主 OS 且测试不可控。**此禁令仅限表现层**:infrastructure
层按真实宿主 OS 选原生库/能力(如 `bead_ffi_loader` 用 `Platform.isIOS` 选原生库)**不受此限**。具体:所有
**开关**与**进度指示器**(设置页两开关、生成 loading、**裁剪屏读尺寸 loading**)**必须**用 `.adaptive`
(iOS→Cupertino、Android→Material);所有**分段控件**(设置页「生成模式」、裁剪比例菜单「纵/横」)在 iOS
**必须**用 `CupertinoSlidingSegmentedControl`(来自 `package:flutter/cupertino.dart`)、Android **必须**保留
`SegmentedButton`(按平台分支,二者取值语义一致);页面转场在 iOS **必须**为横滑 + 边缘滑动返回
(`CupertinoPageTransitionsBuilder`——`MaterialApp` iOS 默认即此,故默认已满足;**若**显式钉
`pageTransitionsTheme` 固化,**必须保留完整 builder map**含 Android 当前默认,**禁止**只写 iOS+Android 两项、
**禁止**用 `platform: TargetPlatform.iOS` 把 iOS 行为强加到 Android)。Cupertino 控件**必须**喂入 pegboard
tokens(激活色/thumb/背景取 `colorScheme`)以保品牌一致。骨架保留 `MaterialApp.router`(不换 `CupertinoApp`)。
未来若引入弹窗,**必须**用 `.adaptive` / `showAdaptiveDialog`(不得裸用 Material `AlertDialog`)。

#### 场景:iOS 呈现自适应/Cupertino 控件
- **当** 在 iOS(`Theme.of(context).platform == TargetPlatform.iOS`)呈现各屏
- **那么** 开关必须经 `SwitchListTile.adaptive` 呈现 iOS 自适应外观(`Switch.adaptive` 只保证 iOS 呈现,**不
  保证**具体 `CupertinoSwitch` widget 类型——故契约锁「自适应呈现 + 取值转发」的行为,不锁 widget 类型)、
  生成与裁剪读尺寸的 loading 必须是 iOS 菊花(经
  `.adaptive`)、两处分段(生成模式、纵/横)必须是 `CupertinoSlidingSegmentedControl`;页面切换为横滑且支持
  边缘滑动返回

#### 场景:Android 保持 Material 无回归
- **当** 在 Android(`TargetPlatform.android`)呈现同样的屏
- **那么** 开关/进度必须是 Material、两处分段必须是 `SegmentedButton`、转场保持 SDK 默认——与本变更前一致,无回归

#### 场景:iOS 分段的选值必须真的抵达引擎(不止渲染)
- **当** 在 iOS 分支点选 `CupertinoSlidingSegmentedControl`(如生成模式选「照片」)并生成
- **那么** 该选值(`generator`)**必须**原样抵达桥(经既有替身桥断言),验收**不得只测「iOS 渲染出 Cupertino
  分段」的存在性**——iOS 分段是一条新代码路径(`onValueChanged(T?)` → 回写),只测存在会漏掉转发接错,重蹈
  「死控件」覆辙

#### 场景:控件皮肤不影响交互契约
- **当** 用户在任一平台的自适应控件上操作(切生成模式/纵横、开关限色与去斑、调尺寸)
- **那么** 选项透传(设定值抵达桥)、尺寸比例锁定、裁剪比例、错误展示等既有契约**必须**保持不变——换的只是
  控件外观,取值语义与数据流不变

#### 场景:纯表现层,引擎与确定性不受影响
- **当** 实施本变更
- **那么** `bead-core`/`bead-ffi` 必须零改动,「CLI == FFI 逐字节」与确定性不受影响,配色 tokens 不变

