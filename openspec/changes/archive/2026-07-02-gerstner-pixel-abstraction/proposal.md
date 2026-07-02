# 提案：Gerstner 像素化抽象（可选生成模式）

## 为什么

现分段管线（`缩放 → 贴板 → 珠色减色`）对**平涂卡通**够用，但对**照片输入**在中低珠数下会崩三处：面积平均缩放（Triangle）糊掉边缘、逐像素独立贴板丢失空间连贯、减色只在结果珠色上**事后**合并——特征/边缘保留差，照片转珠像「糊掉的马赛克」。Gerstner 2012《Pixelated Image Abstraction》(NPAR 2012) 把**降采样与调色板分配联合优化**（超像素 + 感知空间聚类），在极低分辨率下经用户研究证实显著优于朴素 downsample+quantize。

用户已确认**要支持照片输入**，且珠数/分辨率由用户经 `--width/--height` 控制、不锁死——照片 × 中低珠数正是 Gerstner 的价值场景（对平涂卡通边际收益小、对照片收益大）。现在做正合时：`palette-aware-reduction` 刚落地，Oklab 度量/转换（`srgb_to_oklab`/`linearize`）已 `pub(crate)` 可复用，trait 缝已就位。

## 变更内容

- 新增 **Gerstner 风格可选生成模式**（一个 image→pattern 的整体生成策略）：SLIC 类**超像素**（5D：Oklab 三色 + 二位置的 k-means，**规则网格播种**）+ **palette-constrained 分配**——超像素代表色**对珠板 Oklab 快照做 argmin** 贴到**固定珠板**（同 `OklabMatcher` 规则），`max_colors` **复用珠色减色（`GreedyReducer` 贪心合并）到 ≤N**，**永不发明中间色**（延续 `palette-aware-reduction` 原则）。
- 与现分段管线（`image_to_grid → match_pattern → GreedyReducer → 统计/渲染`）**并存、opt-in**：默认仍走现路径；经显式选项启用 Gerstner。落地为「生成策略缝」（`PatternGenerator` trait，现分段流程为默认实现、Gerstner 为可选实现）**或** pipeline 分支——二选一在 design 定，务必**算法走缝、不重构现管线主流程形态**。
- **排除抖动**：不含 Gerstner 的 dithering（Floyd–Steinberg 对实体摆珠是负分——制造散点、更难摆珠；属独立算法 Phase 4、默认关，另案）。
- **确定性（硬门）**：规则网格播种（非随机）、固定迭代次数、固定平局规则、无 `rayon`（Phase 1 单线程）；f32 → 同机 canonical（与 `OklabMatcher`/`Triangle` 同档、非跨架构位精确），**禁** `mul_add`/FMA。
- 照片格式（PNG/JPG/JPEG/WEBP）**已支持**，无需新输入管道。

**定位与排序**：Gerstner 有意**重排/抽象特征**（非忠实几何采样），故必须 opt-in、非默认——想忠实转网格走 Staged。两模式是**工作流分工、非优劣**：`Staged` 平涂干净、要**直接出图**（默认）；`Gerstner` 把颜色过渡带如实 surface 成位置提示，作**人工精修底稿**（照片 / 头像）信息量更高——`Staged` 压成单一最近珠色会丢过渡、边缘发虚。且它**押后**于两个更便宜、两路通用的项：**去孤立点/连通域清理**（`despeckle`，另案先行）与可选 **Area+线性光降采样**（④，部分重叠本提案降采样收益）。Gerstner 定位为**后续高质量模式**（照片/头像/极低珠数），非「不像马赛克」的首选杠杆。

## 功能 (Capabilities)

### 新增功能
- `gerstner-superpixel`: **Gerstner 超像素生成算法**的契约——确定性 SLIC 变体（实数 per-axis 步长、原始网格锚定候选、明确 round-0 质心、快照式更新、固定累加序）、上采样守卫、Oklab-argmin palette-constrained 贴板、`max_colors` 复用 `GreedyReducer`、确定性口径。产出同形状全板 `BeadPattern`（`cells` 合法珠板下标）。（**生成模式选择** `GeneratorKind` 落在 `pipeline`，非本能力。）

### 修改功能
- `pipeline`: `generate_pattern` 增加**生成模式选择**（默认=现分段路径逐字节不变；opt-in=Gerstner）；`GenerateOptions` 增一个模式字段（如 `generator`），`Default` 仍为现分段路径；`generate_pattern` 仍是**唯一编排入口**，不重构主流程形态、不内联算法。
- `cli`: `generate` 新增一个可选 flag 选生成模式（默认沿用现路径；非法值退出码 2）。
- `golden-tests`: 新增一份 **Gerstner 合成小夹具 golden**（`8×8`、无二进制照片）守其确定性机制；默认 `Staged` golden 不变。

## 影响

- **bead-core**：新增超像素 / 生成策略模块（走 trait 缝或 pipeline 分支，不碰现管线内联算法）；`pipeline` 增模式分支；`GenerateOptions` 增字段。复用 `matcher` 的 `pub(crate)` Oklab 转换/度量。
- **CLI**：`generate` 增一个模式 flag（`clap::ValueEnum`，映射 core 枚举）。
- **门**：Gerstner 新路径同样须满足确定性（同机逐字节）+ golden（canonical arm64 字节 / 非 canonical 结构不变量）；FFI 边界**默认仍走现分段路径**、不扩移动端入参（新模式是 CLI/core 能力）。
- **性能**：Phase 1 单线程 × 固定迭代 `T` × **源像素数**（非网格）——`O(源像素×T)`，大照片慢；**v1 不 clamp**（诚实记为已知性能特征，缓解=调用方预缩输入），**源像素 clamp + rayon 留 Phase 2**（配套：clamp 需钉降采样滤镜、rayon 需保质心累加确定序）。bench 覆盖新路径。
- **依赖**：无新增（超像素自实现，复用 `image`/`matcher`）；不引入 dithering、不引入并行。
