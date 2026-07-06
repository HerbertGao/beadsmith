## 为什么

App 目前把 `artkal_s` 硬编码为唯一调色板（`paletteJsonProvider` 固定读单个 asset），
用户无法选自己手上的拼豆品牌。M10 已在引擎侧铺好 14 个 MIT-clean 色卡（MARD、Artkal
S/A/C/M/R、Hama Midi/Maxi/Mini、Perler/Caps/Mini、Nabbi、Yant），但 App 侧用不上。
让用户按自己的豆选色卡，是「照片 → 可购买图纸」闭环的关键一环，也是上架前「像成品」
的基本能力。

## 变更内容

- **设置页新增「色卡」入口**：既有 `GeneratePage` 增加一行「色卡」，点开**底部弹窗**从
  14 个内置色卡中选一个（每项显示品牌名 + 「N 色」）。
- **默认色卡 `artkal_s` → `MARD`**：App 全中文、面向大陆用户，MARD 为最实用入门牌。
- **打包 14 个 MIT-clean 色卡为 asset**：排除 AGPL 的 `_unlicensed/` 四个国产牌；App
  内 `assets/palettes/` 的每份与顶层 `palettes/*.json` 逐字节一致（保持 CLI==FFI）。
- **设置页配置持久化**：色卡、生成模式、限色开关+值、去斑开关+值、**宽维度豆数
  （`width`，非「长边」——竖构图裁剪下宽是短边）** 跨启动记住；**高不持久化**，进设置页时
  按当次裁剪比例从持久宽经既有锁比例逻辑重推（越界时整体等比缩小，沿用既有越界处理）。
  持久值须在**首帧前同步就绪**（`main()` 预载 `SharedPreferences`），避免生成落到默认或
  播种竞态。为此新增 `shared_preferences`。
- **生成结果钉住「生成当时的色卡」**：结果 = `{output, 生成时的 palette}`；结果页的
  配色面板/格子视图/色号一律读**钉住**的那份，消除「生成后改色卡 → 结果颜色/色号错配」。

## 功能 (Capabilities)

### 新增功能

<!-- 无新增能力：选色卡是对既有 mobile-app MVP 的增强 -->

### 修改功能

- `mobile-app`: 从「单一硬编码调色板 + 设置页本地状态」改为「多色卡可选 + 选择与设置
  跨启动持久化 + 结果钉住生成时色卡」。**修改** 2 条需求(「内置默认调色板随 App 离线分发」
  「生成参数可在设置页调节并透传给引擎」)、**新增** 3 条(设置页可选色卡 / 设置页配置跨启动
  持久化 / 结果绑定生成时的色卡)。另**触及但不改文**的需求:「结果页逐格可交互格子视图」
  (色卡来源由实时改为钉住,经新增需求澄清、原文相容)、「目标尺寸由用户在生成页指定」
  (宽改由持久值播种,锁比例契约不变)、「iOS 上采用平台自适应控件与转场」(新增底部弹窗为
  Material `showModalBottomSheet`,与既有 crop/结果详情弹窗同款,不落入其开关/进度/分段的
  自适应枚举、亦非其对话框条款)、「四屏套用设计 tokens」(新增行/弹窗沿用既有 token)。

## 影响

- **代码（仅 `apps/mobile`）**：`paletteJsonProvider` 改为随选中 id 解析；`GeneratePage`
  新增色卡行、页内配置由局部 `setState` 提升为 `shared_preferences` 支撑的持久化模型；
  `ResultPage` 改读钉住色卡；新增色卡选择弹窗与一个显式色卡注册表；`pubspec.yaml`
  声明 14 份 asset。
- **依赖**：新增 `shared_preferences`（仅 App 侧，**不得**进入任何 crate）。
- **引擎 / CLI / FFI**：**零改动** —— `generate` 早已接收任意 `paletteJson` 字符串，
  `bead-cli` 早已 `--palette <path>`。本变更不触碰 `bead-core` 任何模块。
- **确定性**：引擎输出不变——同 `image + palette + dimensions + options` 仍产出逐字节
  相同的 `BeadPattern`/渲染。本变更只改「App 传哪一份 `paletteJson`」，**不涉任何算法
  Phase**、不动 golden。
- **里程碑**：M10（发布工程 / 品牌化线的一环，App 侧，可独立并行）。

## 非目标

- **不做色块缩略预览**：弹窗每项仅品牌名 + 色数，色块预览为 v1 之后可选增强（YAGNI）。
- **不接入 `_unlicensed/` 的 4 个国产牌**（COCO/漫漫/盼盼/咪小窝）：唯一数字源为 AGPL，
  须用官方实体色卡实测重采后另案处理。
- **不实现 CLI `palette list`**：色卡枚举属引擎/CLI 侧独立事项。
- **不做收藏 / 历史 / 色卡搜索 / 品牌分组导航**：收藏是 M10 另一条工作线。
- **引擎侧不新增或修改色卡数据**：14 份已在 M10 落地并归因。
