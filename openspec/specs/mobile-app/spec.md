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

App **必须**把全部内置色卡(顶层 `palettes/` 下 14 个 MIT-clean 色卡:`mard`、`artkal_s`、
`artkal_a`、`artkal_c`、`artkal_m`、`artkal_r`、`hama`、`hama_maxi`、`hama_mini`、`perler`、
`perler_caps`、`perler_mini`、`nabbi`、`yant`)作为 Flutter asset 打包,运行时从**当前选中
色卡**对应的 asset 读取调色板 JSON 字符串传给 `generate`;**禁止**运行时从网络下载调色板。
每个 asset **必须**是无 BOM 的 UTF-8,且与顶层 `palettes/<name>.json` 逐字节一致(与
`bead-cli` 消费同一份),以维持「CLI == FFI」前提。**禁止**打包 `palettes/_unlicensed/` 下的
任何色卡(AGPL 授权,不可随可上架 App 分发)。`pubspec.yaml` **必须逐个显式列举**这 14 份 asset,
**禁止**用 `assets/palettes/` 目录通配——目录通配会把误拷入该目录的 `_unlicensed` 等**任何**文件一并
打包,而只做「14 份存在且字节一致」的正向校验**抓不到多余文件**,AGPL 数据会静默进店包。**必须**有一个
纳入 `flutter test` 的校验,断言打包色卡集合**恰等于**这 14 个 id(**双向**:无缺、无多余、无
`_unlicensed` 路径)且每份与顶层逐字节相同。用户未选过时,默认选中色卡**必须**为 `MARD`。

#### 场景:从选中色卡的打包 asset 读调色板

- **当** App 需要调色板执行一次生成
- **那么** **必须**从**当前选中色卡**对应的打包 asset 读取 JSON 字符串作为 `paletteJson`,
  **禁止**任何网络获取

#### 场景:默认色卡为 MARD

- **当** 用户从未选过色卡(首次使用)
- **那么** 生成**必须**使用 `MARD` 色卡的打包 asset

#### 场景:_unlicensed 及任何多余色卡被校验挡下

- **当** `flutter test` 运行 asset 集合校验
- **那么** `apps/mobile/assets/palettes/` 的文件集合**必须**恰等于 14 个 clean 注册表 id;出现任何多余
  文件(尤其误拷入的 `_unlicensed/` COCO/漫漫/盼盼/咪小窝)或任何缺失**必须**使校验失败

### 需求:结果页可复制 summary 到剪贴板

`ResultPage` 必须提供把 `GenerateOutput.summary`(INIT「Summary Format」原文)一键复制到系统剪贴板的
能力,复制内容必须是 summary 字符串原文,不在壳内重排或重算。复制入口**必须**位于 `ResultPage` 顶部
AppBar(与"保存到相册"并列),`ResultPage` **禁止**再以 `SelectableText` 形式平铺展示 `summary` 文字块
(该文字块与配色计数信息冗余,属噪声)。

#### 场景:一键复制 summary
- **当** 用户在 `ResultPage` 顶部 AppBar 点击「复制」
- **那么** 系统剪贴板必须被写入 `GenerateOutput.summary` 的原文字符串

#### 场景:summary 文字块已移除
- **当** `ResultPage` 渲染
- **那么** 页面**禁止**出现展示 `summary` 全文的 `SelectableText` 卡片;summary 仅通过 AppBar 复制出口可达

### 需求:结果页逐格可交互格子视图

`ResultPage` **必须**以可交互格子视图作为主视图,使用户能逐格辨认"第 N 行第 M 格该用哪颗豆"。
格子视图**必须**从 `BeadPattern.cells`(u16 调色板索引,row-major)+ 已解析的 palette(`List<PaletteColor>`,
下标与 `bead_core::palette::load_palette` 保序)渲染每格纯色方块——**禁止**从 `GenerateOutput.previewPng`/
`gridPng` 位图反推任何格子信息(守 ARCHITECTURE 硬规则 3)。每格之间**必须**留可见网格线(格间 gap 透出
line 色),使用户能区分单个格子边界。格子视图**必须**按**渲染画布**的宽高比 letterbox:含刻度 margin 或边框圈
`k` 时该画布 = 刻度 margin + 板面 `(W+2k)×(H+2k)`,其 `canvasAspect` 由共享布局函数给出;**当刻度 margin=0（两轴均
`<10`)且板面 `(W+2k):(H+2k)=W:H`** 时退化为 pattern 宽高比。**每个格子仍必须为正方形、禁止拉伸变形**（容器按 `canvasAspect`
letterbox 即保证内容格正方)。并支持双指缩放与拖动平移,使大尺寸 grid(如 100×100)可放大到单格可点。缩放**必须**
呈相册式手感:1× 时 grid 居中、其余为留白;放大时留白随 grid 一同放大而逐渐消失(即缩放作用于含留白
的整个视口内容,而非只在一个已裁到 grid 边界的视口内)。点格**必须**回调 `(row, col, paletteIndex)` 并
弹出底部 detail sheet 展示该豆的色块、code、name、count 与行列位置;点格命中**必须**在任意缩放/平移
下精确(通过逆变换/`globalToLocal` 映射)。detail sheet **必须**提供"高亮同色"动作,触发后所有同
`paletteIndex` 的格子**必须**被描边标识,再次触发取消。

配色图例**必须**呈现为底部可展开面板:收起态为薄栏(展示"配色 · N 色"与迷你色块预览),点击**必须**
在收起/展开间切换;展开态**必须**展示可滚动的完整色表(每行色块 + name + code + count),且展开时
grid 区域**必须**相应收缩、使 grid 上移贴合配色面板顶边(配色面板顶边**禁止**遮挡或覆盖 grid)。

#### 场景:相册式双指缩放
- **当** 用户在 `ResultPage` 的格子视图上双指捏合放大
- **那么** grid **必须**跟随缩放,且 1× 时存在的上下留白**必须**随 grid 放大而逐渐减小直至消失;缩放后仍可拖动平移查看任意区域

#### 场景:格子不拉伸
- **当** 一个方形(宽=高)的 pattern 在细长的结果区渲染
- **那么** 每个格子**必须**为正方形(水平/垂直边长比≈1),**禁止**纵向或横向拉伸

#### 场景:点格查豆详情
- **当** 用户点击(单指、无明显拖动)格子视图中某格
- **那么** **必须**弹出底部 sheet,显示该格对应豆的色块、code、name、count 与"第 R 行 · 第 C 列"位置;双指手势**禁止**被误判为点格

#### 场景:展开配色面板使 grid 上移
- **当** 用户点击底部"配色 · N 色"薄栏
- **那么** 配色面板**必须**展开为可滚动完整色表,grid 区域**必须**收缩使 grid 上移贴合面板顶边,面板顶边**禁止**覆盖 grid
- **当** 用户再次点击面板 header
- **那么** 面板**必须**收回薄栏,grid **必须**回到居中(上下留白)状态

#### 场景:高亮同色格子
- **当** 用户在 detail sheet(或色表行)点击"高亮同色"
- **那么** grid 上所有与所选格同 `paletteIndex` 的格子**必须**被描边,再次触发**必须**清除

#### 场景:格子色块来源守硬规则
- **当** 引擎对同 `image+palette+dimensions+options` 产出 `BeadPattern`
- **那么** 格子视图的每格颜色**必须**由 `cells[i]` 下标 palette 得出,**禁止**采样 `previewPng`/`gridPng` 像素

### 需求:结果页可保存到相册
`ResultPage` **必须**提供把「照着拼」参考图纸保存到设备系统相册的能力,使用户在切其它 App 导致后台被杀前能离物化结果。保存内容**必须**是 **App 从 `BeadPattern.cells` + 已解析 palette 用 CPU 光栅化（`image` 包,`pubspec.yaml` 已依赖）渲染的图纸**（含每 10 格加粗分割线、1-based 行列刻度、当前边框圈 `k`;守硬规则 3——从 `cells` 渲染,**禁止**从 `previewPng`/`gridPng` 像素反推),**禁止**用 `Picture.toImage`/`RepaintBoundary.toImage`（撞既有「裁剪在调桥之前」需求禁令 + iOS 模拟器软渲染 + GPU 纹理上限),**不再保存引擎 `GenerateOutput.gridPng` 原文、也禁止改用平滑 `previewPng`**（交互预览与保存图共用几何、有防漂移测试保证一致）。保存**必须**走平台相册 API(iOS `NSPhotoLibraryAddUsageDescription` / Android MediaStore),**禁止**写入 App 私有目录后让用户手动找。相册权限被拒或渲染/保存失败时**必须**给用户可读的提示而非崩溃。首次进入 `ResultPage` **必须**温和提示一次保存,提示**必须**为**顶部浮动条**(**禁止**用底部持久 SnackBar),文案仅提示保存(如「建议保存到相册」),**必须**在 5–10 秒内自动消失并提供手动关闭与"保存"快捷动作;提示为 session 级,不跨启动 nag。

结果页底部安全区**必须**处理自然:配色面板表面色**必须**延伸至物理底边(home indicator 之下),但色表末行**必须**清出 home indicator(内容底部 padding 叠加底部安全区 inset),**禁止**在面板下方留出与面板不连续的死白条。

#### 场景:一键保存格子图到相册
- **当** 用户在 `ResultPage`（AppBar 保存键或首保顶部浮条,**两个入口共享同一当前 `k`**)点保存
- **那么** **App CPU 光栅化的图纸**（含每 10 格分割 / 1-based 刻度 / 当前 `k` 圈边框,而非引擎 `gridPng`、非 `previewPng`、非 `Picture.toImage`）**必须**被写入设备相册,成功后提示"已保存到相册"

#### 场景:相册权限被拒
- **当** 用户拒绝相册写入权限后点击保存
- **那么** **必须**提示"相册权限被拒绝,请在系统设置中允许访问",且 App 不崩溃

#### 场景:首次保存提示为顶部浮动条
- **当** 用户本次会话首次进入有结果的 `ResultPage`
- **那么** **必须**在**顶部**浮现一个含"保存"动作的提示条,**必须**在 5–10 秒内自动消失(或用户手动关闭)
- **当** 用户在同一会话再次进入 `ResultPage`
- **那么** 该提示**禁止**再次出现(session 级 flag)

#### 场景:配色面板底部安全区连续
- **当** `ResultPage` 在带 home indicator 的设备渲染且配色面板展开
- **那么** 面板表面色**必须**铺满至物理底边,色表末行**必须**在 home indicator 之上可完整阅读,面板下方**禁止**出现死白条

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
由用户在生成页指定」是同一屏)。三项选项**必须**以**跨启动持久化的设置页状态**持有(见新增需求
「设置页配置跨启动持久化」),在生成时**原样透传**给 `generate`
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

### 需求:设置页可选色卡

设置页(即既有 `GeneratePage` / `/generate` 屏,**非新增屏**)**必须**提供一行「色卡」入口,
点击**必须**弹出**底部弹窗**列出全部内置色卡(每项**必须**显示品牌名与「N 色」),当前选中项
**必须**有可见标记。用户选定后弹窗关闭,该行**必须**显示当前色卡品牌名,后续生成**必须**使用
该色卡。可选色卡集合**必须**由一个**显式注册表**(`{id, brand 展示名, asset 路径}`,固定顺序:
MARD → Artkal S/A/C/M/R → Hama Midi/Maxi/Mini → Perler/Caps/Mini → Nabbi → Yant)定义,以控制
展示顺序。色卡行与弹窗项显示的品牌名**必须**取自**注册表**(首帧同步可得),**禁止**依赖异步解析
JSON(`palette_codec.parsePalette` 不产出 `brand`,异步解析前行文案将未定义);仅「N 色」可惰性解析。
若某内置色卡 JSON 解析失败,弹窗项「N 色」**必须**回落为非崩溃占位(如「—」),弹窗**禁止**崩溃。
底部弹窗**必须**为普通 Material `showModalBottomSheet`——Flutter 无 adaptive bottom-sheet 构造器,
且底部弹窗不属既有「iOS 上采用平台自适应控件与转场」需求的自适应枚举(开关/进度/分段)、亦非其
对话框条款(`showAdaptiveDialog`)所指,故两端用 Material sheet 合规、不违反该需求。当持久化的色卡
id 不再存在(如某色卡被移除)时,**必须**回落默认 `MARD`,**禁止**崩溃或空选。

#### 场景:点色卡行弹底部弹窗并选择

- **当** 用户在设置页点击「色卡」行,并在底部弹窗中选择某个色卡
- **那么** 弹窗**必须**关闭、该行**必须**显示所选色卡品牌名,且随后点击生成时**必须**以该色卡的
  JSON 作为 `paletteJson` 传入 `generate`

#### 场景:持久化的色卡 id 失效时回落默认

- **当** 持久化保存的色卡 id 在当前内置集合中不存在
- **那么** App **必须**回落到默认 `MARD`,**禁止**崩溃或让色卡处于空选状态

### 需求:设置页配置跨启动持久化
设置页配置**必须**跨 App 启动记住,持久化**必须**经 `shared_preferences`(仅 App 侧依赖,**禁止**进入任何 crate)。**必须**持久化:选中色卡 id、生成模式 `generator`、限色开关与 `max_colors` 值、去斑开关与 `despeckle` 阈值、**边框圈数 `borderRings` 默认值**（整数,默认 0,有规范硬上限;读取时对超限旧值 clamp）、目标**宽维度**(`width` 水平豆数,**非**「长边」——竖构图裁剪下宽是短边;**禁止**持久化 `max(w,h)` 之类的长边值)。**高不持久化**:每次进入设置页**必须**按当次裁剪比例从持久化的宽经既有锁比例逻辑重推(以免记住的 `width:height` pair 与新图比例冲突);持久宽在更瘦长比例下**越界时按既有逻辑整体等比缩小**(宽不再恰等持久值)。**写回规则**:用户在设置页**显式编辑**宽时,持久化写入**经锁比例 / 越界处理后落定的合法宽**(用户编辑即便触发越界缩小,落定值仍写入——它是用户主动设定);而 `initState` 播种 / **进入页面时的重推**(含越界等比缩小)产生的宽**禁止回写**,以免在更瘦长比例下一进设置页就悄悄改写持久的宽偏好。持久值**必须**在**首帧前同步就绪**(`main()` 中预载 `SharedPreferences` 并经 provider override 注入),使设置页首帧即读到持久值;生成**禁止**在持久值就绪前发起(否则会落到默认色卡/尺寸),晚到的持久值亦**禁止**冲掉用户已改的编辑。用户改动上述任一配置(含 `borderRings` 默认)**必须**即时写入持久化。首次启动(无任何持久值)**必须**回落既有默认:色卡 `MARD`、`generator = staged`、限色关、去斑关、**边框圈 0**、宽 100。本需求为纯 App 侧行为,不涉 `bead-core`/`bead-ffi`,确定性不受影响。

#### 场景:配置跨启动保留
- **当** 用户在设置页改动色卡 / 生成模式 / 限色开关或值 / 去斑开关或值 / **边框圈默认** / 宽,随后杀掉 App 并重开
- **那么** 色卡 / 生成模式 / 限色 / 去斑 / **边框圈默认**必须恢复为改动后的值(而非默认);宽**必须**以持久化值为准、在当次裁剪比例下经锁比例逻辑落定(见下一场景),无需用户重设

#### 场景:高不持久化,按当次裁剪比例重推(含越界等比缩小)
- **当** 用户重开 App 进入设置页,且当次裁剪比例与上次不同
- **那么** 宽**必须**以持久化值为基准、高**必须**由其按当次裁剪比例经既有锁比例逻辑重推(**禁止**沿用上次的高);**若**持久宽在该比例下使配对越界,则**必须**按既有越界逻辑整体等比缩小——此时宽不再恰等持久值,**禁止**规定「宽必恰等持久值」

#### 场景:首次启动回落默认
- **当** App 首次安装后启动、无任何持久化配置
- **那么** 设置页各项**必须**为默认:色卡 `MARD`、`generator = staged`、限色关、去斑关、**边框圈 0**、宽 100

### 需求:结果绑定生成时的色卡

一次生成的结果**必须**携带「生成当刻传给 `generate` 的那份色卡」——即 `_generate` 实际转发的
`paletteJson` 本身(**禁止**重读 provider 或重新取值,以免与引擎实际使用的色卡漂移)。
`ResultPage` 的配色面板、格子视图、点格详情与色号**必须**读取该**钉住**的色卡,**禁止**读取设置页
当前选中的实时色卡。由此:用户生成后在设置页改选其它色卡而未重新生成时,已有结果的颜色 / 色号 /
计数**必须**与生成时逐项一致、不被改写。

#### 场景:生成后改色卡不影响已有结果

- **当** 用户以 `MARD` 生成得到结果,返回设置页改选 `Hama Midi`,但**不**重新生成,再回到结果页
- **那么** 结果页格子颜色、配色面板、点格详情的 code/name 与计数**必须**仍为 `MARD` 的对应值,
  **禁止**显示 `Hama Midi` 的颜色或色号

#### 场景:结果页用的色卡恰为生成时传入的那份

- **当** `ResultPage` 渲染格子与配色面板
- **那么** 用于把 `cells[i]` 映射为颜色 / code / name 的 palette **必须**是本次 `generate` 调用
  传入的 `paletteJson` 解析所得,而非 `paletteJsonProvider` 的当前实时值

### 需求:App 显示名为中文品牌名「拼豆匠」且随系统语言本地化
App 面向用户的**显示名** MUST 为中文「拼豆匠」/ 英文「Beadsmith」,**随系统语言切换**,覆盖两端所有可见落点:Android 启动器名（`android:label="@string/app_name"` + `res/values/strings.xml`=拼豆匠(默认桶)、`res/values-en/strings.xml`=Beadsmith）、iOS 显示名（`en.lproj`/`zh-Hans.lproj` 的 `InfoPlist.strings` 覆写 `CFBundleDisplayName`/`CFBundleName`,`Info.plist` 声明 `CFBundleLocalizations = [zh-Hans, en]`,且**无交集回落语言 = `zh-Hans`**（`Info.plist` 的 `CFBundleDevelopmentRegion` 直写 `zh-Hans` 去宏歧义 + pbxproj `developmentRegion=zh-Hans`）,并把 `InfoPlist.strings` 注册进 `Runner.xcodeproj`(`PBXVariantGroup` + `knownRegions` 补 `zh-Hans`)否则不进 `.app`）、以及 `MaterialApp` 应用标题（`onGenerateTitle`,见 Android 任务切换器）。**回落语义**（跨两端一致):偏好语言列表含 en → 英文;**既无 zh 又无 en** → 回落**中文**（Android 默认桶 = 中文、iOS `developmentRegion` = zh-Hans 对称保证）。**MUST NOT** 改动内部包名 `beadsmith`（`pubspec` `name:`）、Android `applicationId`（`com.beadsmith.beadsmith`）、iOS bundle identifier——那些是永久标识,改动等于换一个 app。

#### 场景:显示名随系统语言切换、两端一致
- **当** 系统语言为中文,查看 Android/iOS 桌面图标名、任务切换器
- **那么** 显示名为「拼豆匠」；系统语言为英文时为「Beadsmith」
- **且** 偏好语言列表既无 zh 又无 en(如 `[fr, de]`)时,两端均回落「拼豆匠」（Android 默认桶 + iOS developmentRegion=zh-Hans）
- **且** `applicationId` / bundle identifier / 内部包名保持不变

### 需求:界面文案经 gen-l10n 国际化（中英双语、默认中文），OS 对话框经 InfoPlist.strings 本地化
所有**用户可见**界面文案 MUST 支持**中文 zh 与英文 en**。**回落语义**:偏好语言列表含 en → 英文;**既无 zh 又无 en** → 回落**中文**（`preferred-supported-locales: [zh, en]` 把 `supportedLocales.first` 钉为 zh,`basicLocaleListResolution` 在「无任何匹配」时取首项;不能依赖 gen-l10n 默认的 ARB 文件名序,那会使无匹配时落 en）。注:偏好列表 `[fr, en]` 命中 en → 英文（尊重用户列出的英语,跨 Android/iOS/Flutter 一致）,**非**中文。**App 内**文案经 Flutter 官方 `gen-l10n`（ARB → `AppLocalizations`）提供,**MUST NOT** 在 Widget 里硬编码,改为 `AppLocalizations.of(context)` 键；带数量/尺寸的文案用 ARB `placeholders`/`plural`（不用字符串拼接）；`localizationsDelegates`/`supportedLocales` **必须用生成的聚合 getter**（禁手列,否则丢 `GlobalCupertinoLocalizations`）。**OS 对话框**文案（iOS 相册权限 `NSPhotoLibrary*UsageDescription`）不经 gen-l10n,MUST 经 `en.lproj`/`zh-Hans.lproj` 的 `InfoPlist.strings` 本地化。**例外**:`main()` 中在 `AppLocalizations` 建立之前运行的启动失败兜底屏（引擎/设置加载失败）**允许**保持硬编码中文；品牌名（调色板 `brand` 字段）**不翻译**。

#### 场景:切换系统语言，App 内文案与 OS 权限弹窗均随之变化
- **当** 系统语言为中文时打开各屏并触发相册权限
- **那么** App 内文案与相册权限弹窗均为中文；系统语言为英文时均为英文
- **且** 偏好语言列表既无 zh 又无 en 时回落中文；含 en 时为英文（跨端一致）

#### 场景:界面无残留硬编码中文（可测机制明确）
- **当** 以覆盖自检测试检查 `lib/presentation` 用户可见文案
- **那么** 无残留硬编码中文字面量——自检 MUST 用**可测机制**:关键屏 Widget 文案取自 `AppLocalizations`,或扫描时**先剥注释、只匹配字符串字面量**并 allowlist（品牌名/非 UI 串/已声明例外的兜底屏）；**禁**裸行级 CJK 扫描（会被中文注释假阳)

### 需求:App 具备自有启动图标（占位，自适应双层 + iOS 无 alpha）
App MUST 具备**自有启动图标**（不再用 Flutter 默认图标）,经 `flutter_launcher_icons` 生成:Android **自适应图标**（`adaptive_icon_foreground` 主体留安全区 + `adaptive_icon_background` 满幅底 → `mipmap-anydpi-v26/ic_launcher.xml`)与 iOS（`AppIcon.appiconset`,**无 alpha 通道**,`remove_alpha_ios`)。本阶段图标为**占位**（拼豆网格 motif,可用但非终稿）,上架前 MAY 用正式设计稿替换源图重生成,不改本需求。

#### 场景:两端有非默认自适应图标、iOS 无 alpha、构建通过
- **当** 生成图标并构建 Android / iOS 产物
- **那么** `mipmap-anydpi-v26/ic_launcher.xml` **存在**（证自适应已生成,非仅「mipmap 非空」——库中本有默认图标使「非空」恒真）、iOS `AppIcon.appiconset` 图标**无 alpha 通道**、构建通过
- **且** legacy `mipmap-hdpi/ic_launcher.png`（API<26 旧机）**≠ 库内 Flutter 默认图标**（经 base `image_path` 覆盖）——使「不再用 Flutter 默认图标」在全 API 段成立

### 需求:预览与保存图对齐实体拼豆板（每 10 格分割 + 1-based 刻度 + 可配边框圈）
在既有「结果页逐格可交互格子视图」之上,`ResultPage` 的交互格子视图与「保存到相册」的图 MUST **由 App 从
`BeadPattern.cells` + 已解析 palette 自渲染**（守硬规则 3——从 `cells` 渲染,**禁止**从 `previewPng`/`gridPng`
像素反推),并满足:

- **每 10 格加粗分割线 + 1-based 刻度**:内容格间在细网格线之外,**每 10 格** MUST 画加粗/高亮分割线（间隔
  固定 10,同引擎 `render_grid` 的 `STEP`);内容格坐标 MUST 为 **1-based（水平 1..W、垂直 1..H）**;轴（顶/左）
  MUST 在每 10 格边界标号（10/20/…）。**某轴内容数 <10 时该轴 MUST 无加粗线、无刻度**（与引擎一致）。点格
  detail sheet 的行列 MUST 为 1-based;**`k>0` 时点格换算 MUST 先落到内容相对坐标（减刻度 margin 与 k 偏移）
  再 +1**,不得把边框偏移算进编号。
- **两层几何(单一坐标系)**:被 `Transform` 缩放的**整个画布** MUST = **刻度 margin（顶/左)+ 板面逻辑区
  `(W+2k)×(H+2k)`**（margin 在缩放子树**内**,否则数字与网格脱钩);刻度数字画在网格**外**的 margin（不得压首
  行/首列内容或被裁）。命中测试 MUST：逆变换 → 扣刻度 margin → 板面区 `/cellSize` → 扣 k 偏移 → 内容 `(row,col)`。
- **aspect / 命中分母读共享布局(含 margin),非重推 `(W+2k)`**:内容可非方形;因画布含**顶/左不对称刻度 margin**,
  letterbox/容器 aspect **MUST NOT** 定为 `(W+2k)/(H+2k)`、命中分母 **MUST NOT** 为 `gridRect.width/(W+2k)`。三处
  （`BeadGridView.gridAspect`、命中分母、`ResultPage` 独立 letterbox/留白/legend-flush)MUST 读**同一共享布局函数**
  产出的 **`canvasAspect`（含 margin）**——即被 letterbox/`Transform` 的**整个画布** aspect,**非**板面 `(W+2k)/(H+2k)`;
  仅当刻度 margin=0（**两轴均 <10**、无标号)时二者恰等(此比较**与 `k` 无关**——`k` 改板面尺寸、不产标号 margin)。
  命中分母 MUST 用**该适配器自身尺度下**的板面内 `cellSize`（预览由 fit 后的 `gridRect` 反推、保存由像素预算 `cellPx`
  得出),**非**跨适配器共享一个绝对 `cellSize`。**本需求下容器 aspect 恒为 `canvasAspect`（含 margin)**——刻度 margin
  在**任一轴 ≥10（k=0 亦然)或 k>0** 时**一般**使 `canvasAspect ≠ W:H`(`W=H` 时 `(W+2k):(H+2k)` 仍 =1:1 属巧合,但
  **恒读 `canvasAspect`** 即得正确值),故取代既有「结果页逐格可交互格子视图」需求「保持 pattern 宽高比」的字面
  (**已 MODIFIED,见下「修改需求」**),格子本身仍正方、不拉伸。
- **可配边框圈 k**:MUST 支持用户配置 `k`(整数,**默认 0**,**规范硬上限 `0..8`**,持久化旧值读时 MUST clamp 到
  `0..8`);边框格 MUST 为
  **空留白（浅中性色）、无珠子数据、不计入统计、不进 `BeadPattern`、不编号**;**点边框区 MUST no-op**(不弹 detail
  sheet)。`k` 的**默认值**跨启动持久化(见「设置页配置跨启动持久化」需求);`ResultPage` MUST 提供实时调整 `k`
  的控件,调整后 MUST **仅重渲染预览**(不重新调桥生成),`k` 为本次结果展示态、**不回写默认**。
- **保存图 CPU 光栅化 + 像素预算**:保存图 MUST 用 **CPU 光栅化(`image` 包)** 出 PNG,**禁止** `Picture.toImage`
  /`RepaintBoundary.toImage`（撞既有裁剪需求禁令 + iOS 模拟器软渲染)。保存图分辨率 MUST 有**自足的总像素封顶**:
  每格 `cellPx` 随 `(max(W,H)+2k)` 自适应下降(有下限 clamp),**且最终画布最长边 MUST hard-clamp 到 `maxEdgePx`**
  （**不依赖** App 长边 ≤1000 等外部上界),使内存有界(`maxEdgePx≈4096` → RGBA ~64MB)。
- **交互预览与保存图共用几何、防漂移(锚在布局层 + 保存侧 golden)**:布局函数 MUST 产出**与像素尺度无关**的几何
  （`canvasAspect`、板面豆数、刻度标号**值**、内容格/网格线/刻度的 **cell-unit 或归一化位置**、`k`);两适配器 MUST
  各自传入自身尺度(预览 fit 后 cellPx、保存像素预算 cellPx)由**同一布局函数**换算,**不得各自重推算术**。防漂移测试
  MUST 断言**两适配器的尺度无关产出一致**（`canvasAspect`/标号值/归一化位置/`k`)+ 对 **CPU 光栅 PNG 做 golden**;
  **MUST NOT** 断言 `cellSize`/`marginTopLeft` 等**像素绝对量**逐值相等(预览 fit 与保存预算尺度天生不同),亦 **MUST
  NOT** 断言「预览侧像素与保存图逐像素一致」——取预览像素只能用被禁的 `Picture.toImage`(iOS 模拟器崩),且 `TextPainter`
  与 `image.drawString` 字形天生不同像素。`bead-core`/`bead-cli`/`gridPng`/golden/`BeadPattern`/统计 MUST **不受影响**。

#### 场景:预览含每 10 格加粗分割线与 1-based 刻度
- **当** 用户在 `ResultPage`（`W,H≥10`)查看格子视图
- **那么** 细网格线之外 MUST 每 10 格出现加粗分割线,轴上每 10 格边界 MUST 标 1-based 刻度（10/20/…）
- **且** `W` 或 `H` `<10` 时,该轴 MUST 无加粗线/无刻度（与引擎一致）

#### 场景:调整边框圈数实时重渲染、统计不变、点边框 no-op
- **当** 用户在 `ResultPage` 把边框圈数从 `k` 改为 `k'`
- **那么** 预览 MUST 仅重渲染(不重新生成),在内容外围呈现 `k'` 圈空留白格、内容坐标仍 1..W/1..H、边框不编号
- **且** 配色统计（「配色 · N 色」/色表数量）MUST 不变（边框不计入)；点边框区 MUST 不弹 detail sheet

#### 场景:非方形内容加边框不拉伸、命中精确
- **当** 内容为**非方形** `W×H`（如竖构图）、边框 `k>0`
- **那么** 格子 MUST 仍为正方形(不拉伸)、边框不溢出画布;在任意缩放/平移下点内容格 MUST 精确映射到正确
  `(row,col)`(先扣刻度 margin 与 k 偏移)

#### 场景:保存图 CPU 光栅、含边框/刻度、有像素上限
- **当** 用户在 `ResultPage`（AppBar 或顶部浮条任一入口)点保存
- **那么** MUST 用 CPU（`image` 包)光栅化出含每 10 分割/1-based 刻度/当前 `k` 圈边框的 PNG（**非** `Picture.toImage`、
  **非**引擎 `gridPng`、**非** `previewPng`),写入相册;超大图 MUST 按像素预算自适应降每格像素而非无保护出图/崩溃

