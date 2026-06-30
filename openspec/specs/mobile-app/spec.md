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

手动裁剪必须由 `CropPage` 用 `crop_your_image` 在**调用桥之前**完成,把裁剪后的图像字节交给引擎;
`bead-core` 不含裁剪 UI(中心裁剪之外的交互裁剪属前端职责,INIT/ARCHITECTURE)。引擎收到的就是已裁剪
的最终字节,其内部仍按既有 `crop_center` + resize 处理。

#### 场景:交给引擎的是裁剪后字节
- **当** 用户在 `CropPage` 选定裁剪区域并确认
- **那么** App 必须把**裁剪后**的图像字节作为 `imageBytes` 传给 `generate`,壳内不实现任何像素级裁剪
  算法(仅用 `crop_your_image` 的 UI 结果)

### 需求:目标尺寸由用户在生成页指定

`GeneratePage` 必须让用户指定目标拼豆网格尺寸 `width` × `height`(一像素一豆),并把该尺寸作为
`generate` 的 `width` / `height` 入参转发。生成失败时(桥抛出异常)必须向用户展示该异常消息(已在
`bead-ffi` 边界扁平化为可读文案),禁止静默失败或崩溃。**App 必须在调桥前对 `width`/`height` 施加合理
上界守卫**(引擎对超大尺寸无上限分配,`w·h·3` 字节的急切分配会触发不可捕获的 alloc abort 崩溃):越界
时展示消息、不调用引擎。本里程碑取 `1..=1000`(远超 ROADMAP 最大示例 300×300)。

#### 场景:转发用户尺寸
- **当** 用户在 `GeneratePage` 设定 width × height 并点击生成
- **那么** App 必须以该 width/height 调用 `generate`,不得改写或硬编码尺寸

#### 场景:生成失败显示消息
- **当** `generate` 抛出异常(如无法解码的图像或非法尺寸)
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
