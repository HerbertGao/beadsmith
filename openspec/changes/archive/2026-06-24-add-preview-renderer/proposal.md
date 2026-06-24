## 为什么

里程碑 **M5 — Preview Renderer**。M3 产出了 `BeadPattern { width, height, cells: Vec<u16> }`——配色后的管线真理源
（CLAUDE 规则 3），M4 已把它派生成统计/summary。M5 把同一真理源**渲染成图**：`preview.png`（无坐标，代表成品
拼豆外观）与 `grid.png`（带行号/列号/网格线，拼装时照着摆豆）。这是 color-matching 规范早已写下的**前向约束**
（`下游渲染（M5）必须从 cells[i] 查 palette.colors[idx].rgb 上色，禁止从 PixelGrid 原始 RGB 或渲染图反推`）的落地，
也是 INIT/ROADMAP 里 golden 测试清单（`preview.png`）的兑现。M6 pipeline 串联、M7 golden 冻结都依赖这里产出的
**逐字节确定性** PNG。

## 变更内容

- `bead-core` 新增 `renderer` 模块（两个公开函数，均**从 `BeadPattern` + `&Palette` 派生**、不碰文件系统、返回内存
  PNG 字节）：
  - `pub fn render_preview(grid: &BeadPattern, palette: &Palette, opts: &RenderOptions) -> Result<Vec<u8>, BeadError>`
    ——**无坐标**，每颗珠子放大成 `cell_size × cell_size` 的实心方块（默认 `cell_size==10`），颜色取
    `palette.colors[cells[i]].rgb`，行优先摆放。
  - `pub fn render_grid(grid: &BeadPattern, palette: &Palette, opts: &RenderOptions) -> Result<Vec<u8>, BeadError>`
    ——同样的珠子格，外加**每格细网格线 + 每 10 格加粗分隔线 + 1-indexed 行号/列号**（坐标数字用**内置位图数字字体**
    绘制，无字体渲染依赖）。
- 新增 `RenderOptions { cell_size: u32, shape: BeadShape }`（仿 `image` 模块 `ResizeOptions` 先例；`Default` →
  `cell_size: 10, shape: Square`）与 `BeadShape` 枚举（**当前仅 `Square` 一个变体**，标 `#[non_exhaustive]` 作为
  「后续扩展多种珠形」的 seam——用户明确要求保留，但 M5 **不**实现 Circle/Ring，见 design D2）。
- `BeadError` 新增 **一个** 变体 `ImageEncode`（PNG 编码失败；`ImageDecode` 已占用 `#[from] ImageError`，故新变体用
  具名字段而非 `#[from]`）；`BeadError` 已 `#[non_exhaustive]`，加变体**非破坏**（design D8）。维度类可达失败
  （`cell_size==0`、空网格、输出尺寸溢出 `u32`）**复用** M2 既有的 `InvalidImage { reason }`。
- **`BeadPattern` 不动**：渲染是**按需派生**，绝不从 `BeadPattern` 存图、绝不从渲染图反推数据（CLAUDE 规则 3 / D4）。

## 功能 (Capabilities)

### 新增功能
- `renderer`: 从 `BeadPattern` + `Palette` 渲染两张**逐字节确定性**的 PNG——`render_preview`（无坐标的成品外观）与
  `render_grid`（行/列号 + 网格线的拼装图），珠形与放大倍率由 `RenderOptions` 控制；越界调色板下标以哨兵色容错
  （不 panic），坐标数字用内置位图字体（零字体依赖）。

### 修改功能
- 无。M5 **不修改任何已生效规范的需求**：color-matching 规范「BeadPattern 是配色后的真理源」里关于渲染的文字
  （`下游渲染（M5）必须从 cells[i] 查 palette.colors[idx].rgb`，line 94–106）是**前向约束**，由本 change 的 renderer
  规范 + 测试**落地兑现**，其规范性文字**不变**（同 M4 落地 statistics 前向约束的方式，无需 delta）。statistics 规范
  对 M5 的提及（line 66）同理。

## 非目标（Non Goals）

按 YAGNI 推迟 / 不做：

- **Circle / Ring 等其它珠形** → `BeadShape` 只留 seam（`#[non_exhaustive]` + 单 `Square` 变体），实现留后续 change
  （D2）。本 change 不写任何非 `Square` 的渲染分支。
- **`Renderer` trait** → ARCHITECTURE「Future Plugin Architecture」的未来扩展点；M5 先用普通函数，待真有第二个渲染器
  再引 trait（与 M3 给*算法档位*引 `ColorMatcher` trait 不同——珠形差异由 `shape` 字段承担，不需要换整个渲染器，D1）。
- **写文件 / 拼接 pipeline** → M6：core 只产 `Vec<u8>` 字节（规则 1，不碰文件系统），由 `bead-cli` 写 `preview.png`/
  `grid.png`，由 `pipeline::generate_pattern` 串联 load→resize→match→stats→render。
- **可配置线宽 / 线色 / 字色 / 背景色 / 标号间隔** → M5 钉为代码内**固定常量**（确定性 + 简单）；`RenderOptions`
  可后续长字段（**技术上是破坏变更**——`RenderOptions` 非 `#[non_exhaustive]`，同 `ResizeOptions` 取舍；靠 `Default` +
  `..Default::default()` 缓解、且 M5 阶段无外部字面构造者），现在不投机加（D2/D5）。
- **抗锯齿 / 透明通道 / 半透明 / 自定义 TrueType 字体 / 图内色卡图例** → 非 MVP。
- **严格的 PNG 字节硬编码 golden** → M7（本 change 的确定性测试比较**解码后像素** + **同跑两次字节相等**，跨架构稳健；
  逐字节硬编码 PNG golden 与「依赖升级须重生成」的取舍归 M7，见 D3）。
- **CSV / 其它导出** → 非 MVP。`rayon` 并行 → Phase 2（M5 单线程）。

## 影响

- **代码**：
  - `crates/bead-core/src/renderer/mod.rs`（新）——`render_preview` / `render_grid` / `RenderOptions` / `BeadShape`
    + 私有绘制原语（`paint_*`、位图数字字体常量、PNG 编码）。
  - `crates/bead-core/src/lib.rs`（改）——`pub mod renderer;` + 重导出 `render_preview / render_grid / RenderOptions /
    BeadShape`；`BeadError` **新增 `ImageEncode` 变体**（非破坏，`#[non_exhaustive]`）。
- **依赖**：**无新增**。复用既有 `image`（0.25，已开 `png` feature——解码已用，编码同 crate）；数字字体是手写
  `const` 位图，**不**引 `ab_glyph` / `rusttype` / `fontdue` 等字体渲染 crate（D9）。
- **确定性**（引擎输出，硬性门）：同 `(BeadPattern, Palette, RenderOptions)` →
  ① **像素缓冲跨架构逐位相同**（纯整数绘制：无 `f32`、无随机、无 `HashMap`/`HashSet`、无 `rayon`）；
  ② **PNG 字节同跑两次相同**（编码参数显式钉死，不靠可能随版本漂移的 crate 默认）。PNG 字节在锁定依赖版本
  （`Cargo.lock`）下稳定；故意升级 `image`/`png` 可能改变字节——这正是 M7 让 PNG golden「响亮失败」的意义（D3）。
- **里程碑 / Phase**：里程碑 M5；Phase 1（单线程；算法档位不涉及——渲染不属配色档位）。
- **文档**：**无真理源校正**。ARCHITECTURE（`render_preview(...)` / `render_grid(...)` + `Renderer` 列在 Future Plugin
  Architecture）、ROADMAP（M5 done-when：两图产出且跨跑字节一致）、INIT（preview 无坐标 / grid 带行列号+格线）均与本
  设计一致，无需改动。
