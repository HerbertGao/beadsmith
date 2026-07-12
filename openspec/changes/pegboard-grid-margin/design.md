## 决策

> 术语:`W×H` = 内容豆数（水平 `width` × 垂直 `height`,**可非方形**——竖构图裁剪下 W 是短边）。
> `k` = 边框圈数（每圈 = 每边 1 格）。**板面逻辑区** = `(W+2k)×(H+2k)`。**最终画布** = 刻度 margin
> + 板面逻辑区。

### D1：App 接管用户可见图纸渲染，引擎 / CLI 不动
交互预览与「保存到相册」的图都由 App 从 `BeadPattern.cells` + palette 自渲染;引擎 `render_grid` 的
`gridPng` 降为**开发辅助**、保持现状。

- **替代方案:给引擎 `render_grid` 加边框/1-based 参数,App 继续用 `gridPng`**。**否决理由**:CLI `gridPng`
  用户无感知、本是开发辅助;把纯终端展示需求下沉引擎会动 golden。用户明确倾向 App 自渲染。
- **不破硬规则**:`BeadPattern` 仍是唯一真相源;统计取 `output.stats`(`result_page.dart:313`)非从渲染图
  反推;App 从 `cells` 正向绘制。「CLI 是契约」约束 pattern/统计,非渲染图样式。
- **尾巴（非目标）**:引擎每次 generate 仍产 `gridPng`(cell_size=10),App 不再用属算力浪费——本次刻意
  「引擎零改」保留,后续可单独 change 让 FFI 跳过 `gridPng`。

### D2：内容坐标 1-based（对齐板辅助网格）+ 每 10 格加粗分割线
内容格 **1-based（1..W / 1..H）**,对齐实体刻度板;细网格线 + **每 10 格加粗分割线**;轴上（顶/左）在每 10
格边界标号（10/20/…）。

- **N<10 退化**:某轴内容数 <10 时该轴**无加粗线、无刻度**(与引擎 `renderer/mod.rs:354` `has_col=width>=STEP`
  一致);仅当存在 10/20/… 内容边界才标号。
- **detail sheet 已是 1-based**:`result_page.dart:219` 现已 `cellDetailPosition(row+1, col+1)`、arb 已「第 R
  行」——**这不是新工作**。真正的新工作是:`k>0` 时点格换算须先落到**内容相对 0-based `(row,col)`**（减掉
  k 圈偏移与刻度 margin）再 `+1` 显示,**勿把边框偏移算进编号**。
- **替代方案:0-based / 边界标号（如引擎 `gridPng` 现在的每 10 边界数字）**。**否决理由**:实体刻度板按
  1-based 内容格编号,让屏幕号对上板号是本变更全部目的。

### D3：可配边框圈 k（板对齐留白），默认 k=0 opt-in，硬上限 + 像素预算
`k`（内容外围空留白圈数,每圈每边 1 格）:内容 `W×H` → 板面逻辑区 `(W+2k)×(H+2k)`。边框格**空留白、无珠、
不入统计、不进 `BeadPattern`、不编号**（内容 1..W/1..H 是唯一坐标锚);**点边框区 no-op**（无 bead 可查,
不弹 detail sheet）。

- **默认 k=0**(行为与现状一致,opt-in);**硬上限**（规范值 `0..8`),持久化旧值超限须 clamp。
- **像素预算(防 OOM)**:保存图**每格像素随尺寸自适应降**——`cellPx = clamp(floor(maxEdgePx / (max(W,H)+2k)), 4, 10)`
  （**注意括号**:分母是 `(max(W,H)+2k)`),`maxEdgePx ≈ 4096`。CPU 光栅存图是内存 buffer→PNG、**不经 GPU 纹理**
  （8192 上限只是否决 `toImage` 的理由,非本约束);本约束是内存(`4096²` RGBA ~64MB)。**最终画布最长边 MUST
  hard-clamp 到 `maxEdgePx`**(画布 = 刻度 margin[仅数十 px] + 板面·cellPx),**不依赖** App 长边 ≤1000 等外部上界。
  极端超大图(如 `max(W,H)+2k > ~1020`,含 margin 后板面·4 会略破 4096)下,**hard-clamp 压过 `cellPx` 下限 4**
  (有效 `cellPx` 可 <4;此时 1000 豆图早已不可读,降 <4 无实际损失)——最终边长 ≤ `maxEdgePx` 恒成立。
- k 的**默认值**持久化(见 D5 持久化需求);**结果页 k 是本次结果的展示态、不回写默认**（类比高不持久化）。
- **替代方案:板子产品库 / 默认 k=1**。**否决理由**:各家板不同、用户判断不需要;k=1 会改所有用户板对齐前提。
- **边框不用负数/跳 0 编号**:实体板边框钉本无印号,内容 1..N 对齐辅助网格即足;负数/跳 0 增歧义（调研确认
  业界无此范式）。

### D4：保存图走 CPU 光栅化（`image` 包），**不用 `Picture.toImage`**
`ResultPage` 保存动作把 App 用 **`image: 4.9.1`（已在 `pubspec.yaml:57`,纯 Dart CPU）** 光栅化的图纸转 PNG
存相册,**禁止用 `RepaintBoundary.toImage` / `Picture.toImage`**。

- **为什么禁 toImage**:本仓既有「裁剪在调桥之前」需求 + `crop_page.dart:17` 注释**明文禁** `toImage`——iOS
  模拟器软渲染会 `Invalid image dimensions`,而本里程碑 **iOS 模拟器是硬验收**;且大图（1000 宽 → 10000px）
  超 GPU 单轴纹理上限 ~8192,`toImage` 报错/截断。而 CPU `image` 包仅受内存限（同 crop 绕过手段）。
- 保存动作有**两个调用点**(`result_page.dart:91` AppBar + `:336` 顶部浮条 onSave),两处**都**改为「先
  光栅化 → 存」;CPU 光栅化大图较慢,须在 compute isolate 或加进度、并处理失败走既有 catch 提示。
- **替代方案:保留存 `gridPng` / 用 `Picture.toImage`**。**否决理由**:前者对不上板;后者撞 iOS 模拟器禁令 +
  纹理上限。

### D5：交互预览与保存图共用**几何/布局**（非同一 draw API），加防漂移 golden
交互预览是屏上 `CustomPainter(ui.Canvas)`(矢量、可缩放清晰)、保存图是 `image` 包 CPU 光栅——**两套 draw
API 无法共用同一 `paint(Canvas)`**。故抽一个**纯几何/布局函数**:入 `W/H/k` + 该适配器的**尺度 `cellPx`**
（**不入 `cells`/`palette`**——那是 draw 适配器的事,布局只算几何) → 出 {刻度 margin、内容格 rect、细/粗网格线
位置、刻度标号位置与值、边框区、`canvasAspect`}。两个薄 draw 适配器（`ui.Canvas` 预览 + `image` 包保存）**各自
传入自身 `cellPx`** 调用同一函数:预览传 fit 后的 cellPx(由视口 contain 反推)、保存传像素预算 cellPx。

- **防漂移(唯一几何来源)**:两适配器 MUST **消费同一布局函数**(**不得各自重推算术**,那正是 B3 根因),但断言点是
  **尺度无关产出**:`canvasAspect`、刻度标号**值**、内容格/网格线/刻度的 **cell-unit 或归一化位置**、`k`、板面豆数
  ——**这些两适配器逐值相等**。`cellSize`/`marginTopLeft` 等**像素绝对量随各自 `cellPx` 不同,MUST NOT 断言逐值相
  等**(预览 fit vs 保存预算尺度天生不同)。测试断言**尺度无关产出一致** + 对 CPU 光栅 PNG 做 golden。**预览侧不做
  像素 golden**:取 `ui.Canvas` 像素只能用 `Picture.toImage`/`RepaintBoundary.toImage`,正是 D4/B1 禁的、iOS 模拟器
  会崩的 API;且预览 `TextPainter` 与保存 `image.drawString` 位图字体天生不能逐像素相同——故防漂移锚在**布局层(尺度
  无关) + 保存侧 golden**。
- **margin 用 `cellSize` 的实数比例**（勿学引擎 renderer 的整数截断 `scale=cell/5`),否则 `canvasAspect` 不严格
  cell-无关、预览 letterbox(用 cell-free aspect)与实绘画布会差零点几格。
- **替代方案:笼统「抽一个 painter 共用」/「断言两后端像素一致」**。**否决理由**:B1 使二者必为两套 API,「像素
  一致」不可实现(且撞禁令);只能共用几何、断言布局一致。

### D6：两层几何——被缩放的画布 = 刻度 margin + 板面逻辑区（单一坐标系）
刻度数字需在网格**外**留 margin（同引擎 `renderer/mod.rs:362-368` 的 `margin_left/margin_top`,随刻度位数/
cell 尺寸成比例),不能塞进格子区。**被 `Transform` 缩放的整个画布** = `刻度 margin(top/left) + 板面逻辑区
(W+2k)×(H+2k)`（margin 在缩放子树**内**,否则缩放/平移时数字与网格脱钩)。命中测试:屏坐标经逆变换 → **扣刻度
margin** → 板面区 → `/cellSize` → **扣 k 圈** → 内容 `(row,col)`,越界 no-op。

### D7：全程 `width×height`（非 `N×N`）；aspect/命中分母**读共享布局**（含 margin），非重推 `(W+2k)`
内容**可非方形**（竖构图裁剪 2:3/3:4 等,规范允许)。**关键修正(B3)**:letterbox aspect **不是** `(W+2k)/(H+2k)`,
命中分母 **不是** `gridRect.width/(W+2k)`——因画布含**顶/左不对称的刻度 margin**。三触点 MUST 改为**读 D5 共享布
局函数**:

1. `bead_grid_view.dart:161` letterbox `gridAspect` → **`layout.canvasAspect`（含 margin）**(仅当 margin=0,即该轴<10 时才恰等 `(W+2k)/(H+2k)`)
2. `bead_grid_view.dart:145-146` 命中:`gridRect` 语义定为**含 margin 的整画布**;`content = ((screen−gridRect.topLeft)−layout.marginTopLeft)/layout.cellSize`,再 `−k`,越界 no-op——此处 `layout` 由**预览自身 fit 后的 cellPx** 调用(其 `marginTopLeft`/`cellSize` 与屏上 `gridRect` 同尺),分母 = 板面内 `cellSize`(**非**跨适配器共享的绝对值、**非** `gridRect.width/(W+2k)`)
3. `result_page.dart:265-266` `ResultPage` **独立**的 `gridAspect` + whitespace/legend-flush → 同样用 `layout.canvasAspect`（`ResultPage` 持有 k 状态)

否则非方图/margin>0 会:格子拉伸(破既有「格子不拉伸」场景)、边框溢出、远端点格 off-by-margin 串位。**根治 = 单一
几何来源(D5 布局函数),三触点消费之,不各自重推。**

### D8：`k` 默认值持久化并入既有「设置页配置跨启动持久化」需求
`borderRings`（默认值）纳入 `GenerateSettings` + `shared_preferences`(须同步改 `generate_settings.dart` 的
ctor/defaults/copyWith/==/hashCode/build/_persist 六处镜像);且既有「设置页配置跨启动持久化」需求的**必须持久
化字段清单**须 MODIFIED 纳入 `borderRings`,否则合并后主规范自相矛盾(「必须持久化 k」vs「清单不含 k」)。首启
回落 `k=0`。

## UI 细节（本次默认，可评审调）

- **预览刻度数字**:轴上每 10 标号(顶/左),非每格;点格出 1-based 行列。
- **边框格样式**:浅中性留白色(区别内容格与奶白底);点边框 no-op。
- **默认 k=0**;硬上限 `0..8`(规范值)。
- **改 k 交互**:结果页步进器(0..上限),实时重渲染预览;保存按当前 k 出图。

## 确定性 / 引擎影响

零。纯 `apps/mobile` 展示层:改 `bead_grid_view` 渲染+命中(padded)、新增 `image` 包 CPU 存图 + 共享几何、
结果页 k 状态+步进器、设置页 k 默认+持久化、i18n 新键。`bead-core`/`bead-cli`/`gridPng`/golden/`BeadPattern`/
统计不受影响,CLI==FFI 的 pattern 契约不变。
