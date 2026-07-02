# 设计：Gerstner 像素化抽象（可选生成模式）

## 上下文

**排序（与专家共识对齐）**：本提案**押后**于两个更便宜、ROI 更高的项——**去孤立点 / 小连通域清理**（`despeckle`，pattern→pattern 后处理，Staged/Gerstner 两路通用、对实体摆珠直接减「碎/脏」，另案先行）与可选的 **Area + 线性光降采样**（④，比现 Triangle-in-sRGB 平均更准、部分**重叠** Gerstner 的降采样收益）。Gerstner 定位为**后续高质量模式**（照片 / 头像 / 极低珠数），不是「让拼豆不像马赛克」的首选杠杆（那是分辨率 + despeckle）。

现管线分段（`image_to_grid`：`crop_center` + `Triangle` 缩放 → `match_pattern` 贴固定珠板 → `GreedyReducer` 珠色减色 → 统计/渲染），可替换算法各走 trait 缝（`ColorMatcher`/`BeadReducer`/`Renderer`；`Quantizer` 已删）。对**照片**输入在中低珠数下：面积平均缩放糊边、逐像素独立贴板丢空间连贯、珠色事后合并保留特征差。Gerstner 2012 把降采样与分配**联合优化**（超像素 + 感知聚类），低分辨率下优于朴素。

约束：`bead-core` 无 UI/FS、确定性（同机逐字节 + CLI==FFI）、Phase 1 单线程、算法走缝、`generate_pattern` 唯一编排入口。可复用：`matcher` 的 `pub(crate)` `srgb_to_oklab`/`linearize` 与珠板 Oklab 快照 + 最近色平局规则（Gerstner 前段做 **Oklab-坐标 argmin**——`find_best_match` 只吃 RGB `[u8;3]`、不能收 Oklab 质心，故 Gerstner 另写一个同规则的 Oklab-argmin）；珠板固定 199 色。

## 目标 / 非目标

**目标：**
- 为**照片**在中低珠数产更好图案（边缘/特征保留优于分段路径）。
- opt-in 与现路径**并存**、可 A/B、可回退；默认不变。
- 确定性（同机逐字节）；固定板 **palette-constrained**、**永不发明中间色**；`max_colors` 仍 ≤N。

**非目标：**
- 不改默认生成路径；不引入抖动（Phase 4 另案）；不做自由调色板优化（板固定）；不并行（Phase 1）；不扩 FFI/移动端入参；不新输入格式；不引入 gamma-correct 线性光缩放（④ 另案）。
- **不追求几何忠实**：Gerstner **有意重排 / 抽象特征**（论文自陈五官呈 caricature 式变形），非忠实照片采样——这正是它必须 **opt-in、非默认**的根本原因（想「忠实转网格」的用户走 Staged）。人像 / 头像 / 宠物 / 角色这类「远看要认得出主体」的场景受益最大；忠实构图需求走 Staged。

## 决策

**D1：opt-in 可选模式，不替换默认。**
`GenerateOptions` 增 `generator` 字段，默认现分段路径。理由：可与刚落地的分段路径 A/B、可回退、不动默认 golden。
- *替代*：Gerstner 替换主路径。否决——不可 A/B、回退难、与刚落地的分段架构冲突、默认 golden 全变。

**D2：pipeline 内分支，不新增 `PatternGenerator` trait。**
`generate_pattern` 内 `match opts.generator` 调两条前段：`Staged` 走现 `image_to_grid → match_pattern`；`Gerstner` 走 `crop_center → gerstner::superpixel_assign`。两者都产出**全板 `BeadPattern`**，之后**共用同一后段**（可选 `GreedyReducer` 减色 → 统计 → 渲染）。Gerstner 算法本体在独立 `gerstner` 模块。
- *替代*：新 object-safe `PatternGenerator` trait（image→pattern）。否决——只两种实现、且两者入口形态不同（Staged 从 grid、Gerstner 从裁剪原图），统一 trait 签名别扭，是投机抽象（YAGNI）；`generate_pattern` 仍是唯一编排入口（规则 4），分支不违背。第三种生成策略出现时再抽 trait（见 Open Questions）。

**D3：Gerstner 只替换「缩放 + 贴板」前段，`max_colors` 复用 `GreedyReducer`。**
Gerstner 前段产出**全板 `BeadPattern`**（不同珠色数可能 > N）；`max_colors==Some(n)` 时**复用现 `GreedyReducer`** 后处理到 ≤N。
- 理由：DRY——与分段路径**同一减色语义**、同一 ≤N 上限口径；Gerstner 只负责「更好地降采样+贴板」这一它擅长的部分。
- *替代*：在超像素/分配阶段联合限制 ≤N（Gerstner 原文的调色板缩减）。否决——板固定使「挑色」退化；与现减色重复；复杂度高、收益不明。

**D4：超像素 = 确定性 SLIC 变体（5D k-means，规则播种，固定迭代）。**
5D 空间 = Oklab `(L,a,b)` + 归一化位置。**确定性要点（都 MUST，权威见 gerstner-superpixel 规范；review 补强后定稿）**：
① **实数 per-axis 步长** `S_x=W/w`、`S_y=H/h`（不取整）；距离位置项 `m²·((Δx/S_x)²+(Δy/S_y)²)`、窗口/归一按 per-axis（不混用单一 `S`）。
② **明确 round-0 质心** = 种子中心最近整数像素（`round` = Rust `f32::round`、ties-away）的 Oklab + **原始源像素位置**（质心存原始坐标、非归一化；距离计算时才按 `S` 归一 Δ，避免二次归一）（不留未定义初值）。
③ **候选集按原始网格锚定**（像素候选 = 其原始网格 cell 及 8-邻的种子、**按下标非漂移位置**）→ 覆盖**不随种子漂移失效**，`T>1` 仍每像素必分配、无漏（这是 SLIC 标准做法，也是本 review 抓到的最深洞）。
④ **上采样守卫**：`W<w || H<h`（`S<1`）→ 返回 `InvalidImage`、不进退化路径（Staged 不受限）。
⑤ 每轮**快照式**（分配只读上轮质心、途中不 mutate）；质心 f32 累加**行优先固定序**（f32 加法非结合）；**空簇保留上轮质心**；无随机/rayon/HashMap 顺序/mul_add。
⑥ **贴板 = Oklab-argmin**（非 `find_best_match`，见 Context）。
f32 → 同机 canonical（同 Oklab 档）。`m`、`T` 为**固定编译期常量**（数值实现期目视调、非运行时输入）；`S_x=W/w`、`S_y=H/h` 是**运行期导出的确定值**（公式固定、非常量）。
- *替代*：Gerstner 原文的 MCDA/模拟退火调色板联合优化。否决——含随机性、难确定性化、重、Phase 1 单线程慢。
- *替代*：无空间项的纯 k-means / 直接 median-cut。否决——丢空间连贯正是 Gerstner 要解决的。

**D5：palette-constrained 贴板复用 `OklabMatcher`。**
每个 cell 的簇质心（Oklab 坐标）→ **对珠板 Oklab 快照做 ΔEok² argmin**（`(ΔL)²+(Δa)²+(Δb)²`、不开方、严格 `<`、平局取最低下标——**与 `OklabMatcher` 同规则、同 `pub(crate) srgb_to_oklab` 快照，但入参是 Oklab 坐标**；`find_best_match` 只吃 RGB，故这是一个独立的 Oklab-argmin）。迭代中**不**改珠板色。得全板 `BeadPattern`（`cells` 均合法珠板下标）。

**D6：确定性口径。**
规则播种（非随机）、固定 `T`、分配平局取**最小种子下标**、簇均值/距离 f32 **禁 `mul_add`**、无 `rayon`。全程 f32 → **同机 canonical**（与 `OklabMatcher`/`Triangle` 同档，非跨架构位精确）；复用 `pub(crate)` `srgb_to_oklab` 保证与配色口径一致。

**D7：v1 不含 saliency。**
Gerstner 原文用显著性给重要区更多超像素/保真。v1 用**均匀**超像素（每 cell 一种子），简单、确定、够 MVP。
- *替代*：v1 引入 saliency。否决——加复杂度与调参面，MVP 不需；留后续（Open Questions）。

**D8：选项与 CLI。**
`GenerateOptions { .., generator: GeneratorKind }`，`GeneratorKind { Staged, Gerstner }`，`Default == Staged`。CLI `generate --generator staged|gerstner`（`clap::ValueEnum` 手映射 core 枚举，默认 `staged`，非法值退出码 2）。FFI 边界**不暴露** `generator`（默认 Staged，随现路径）。

## 风险 / 权衡

- [Phase 1 单线程 × `T` 迭代 × **源像素数**（非网格）→ 大图慢：12MP 照片 × T=10 ≈ 1e9 次] → **v1 明确不 clamp、不做工作分辨率降采样**（避免引入一个未钉滤镜的新 f32 确定性面 + 与上采样守卫的 pre/post 交互）——诚实记为**已知 v1 性能特征**：`O(源像素 × T)`，大照片慢，缓解 = 调用方**先自行缩小输入**；**源像素 clamp + `rayon` 并行**留 **Phase 2**（两者配套：clamp 需钉降采样滤镜、rayon 需保质心累加确定序）。bench 覆盖并标注「大源图慢、建议预缩」。
- [`rayon`-Phase 2 与确定性张力] → 分配阶段读快照、可并行；**质心累加必须保持确定序**（有序/固定树归约，不能裸并行 sum 打乱 f32 加序）——design 显式记，防 Phase 2 天真并行破坏确定性门。
- [`GreedyReducer` 最少用量合并可能**重新打散** Gerstner 刚建立的空间连贯] → 可接受、由目视验收；DRY 复用减色仍是对的取舍（D3）。
- [Gerstner 对平涂卡通边际收益小（已实证）] → 定位为**照片路径**、opt-in、默认不变；文档写清适用场景，避免用户对卡通误用。
- [f32 迭代 → 非跨架构位精确] → 与 Oklab 同档，canonical-only；golden 走 canonical arm64 / 非 canonical 结构不变量。
- [compactness `m`、迭代 `T` 需调] → 作内部常量、目视调优，不进 CLI（维持极简边界）。
- [Gerstner 是本项目最重算法、复杂度大] → 用户已确认照片需求、接受；走缝隔离、默认关、可回退。

## 验收发现（§7.1 端到端目视，2026-07-02）

实现正确 + **确定性验证通过**（UncleGao 80×80 `--generator gerstner` 同机跑两次 `pattern.json` 逐字节相同；输出 palette-constrained、44 种珠色 < staged 61、背景 S73 Steel Blue 更连贯）。**但 UncleGao（平涂卡通）上 Gerstner 目视比 Staged 差**——超像素聚类打散了眼镜/文字/轮廓等**有意的细线**，边缘断裂、GAO 文字出现红斑、背景有散点。这**证实**了非目标「不追求几何忠实（caricature 变形）」与风险「对平涂卡通边际收益小」：**作为直接成品，Gerstner 是照片工具，不适合平涂卡通**（同 despeckle 的教训）。「照片边缘/特征保留优于 staged」的卖点**需真实照片**验证——本机无照片夹具，留待用户上传真实照片时确认。默认 `Staged` 保护卡通用户（默认输出不变、golden 不动）。

**用途二·人工精修底稿（2026-07-02 用户复盘）**：换「人工优化底稿」而非「直接成品」的标准看,结论反转——Gerstner 那些「散点」是**信息不是噪声**:超像素把颜色过渡带如实 surface 出来,正是「此处该换色 / 该留渐变」的位置提示,给人工摆珠一个信息量更高的 draft;而 `Staged`+Oklab 把每块压成单一最近珠色、平涂干净但**丢过渡信息**,边缘软、层次塌(用户评「发虚」),作为再加工底稿反而信息少。故两模式定位是**工作流分工、非优劣**:`Staged`=要直接出图(平涂卡通,默认);`Gerstner`=要人工精修 / 保留过渡结构(照片 / 头像 / 底稿)。这进一步支撑 opt-in、目视验收的设计取舍。

## 迁移计划

无历史用户。默认 `Staged` 逐字节不变、Gerstner opt-in。默认路径 golden **不变**（默认仍 Staged）。Gerstner 路径确定性由单测（同机重算逐字节）+ **一份合成小夹具 golden**（8×8、无二进制照片，见 golden-tests 增量）守——同机重算证不了结构回归，故加 golden 钉住那 5 个确定性机制。回滚：还原提交即可（无持久化状态）。

## 待解问题

- **`PatternGenerator` trait vs pipeline 分支**：本 design 取分支（YAGNI）；若第三种生成策略出现再抽 trait——留 review 确认。
- **saliency**：v1 不含（D7）；后续是否引入。
- **≤N**：本 design 复用 `GreedyReducer`（D3）；是否需要 Gerstner 原生联合缩减。
- **`T`/`m` 具体数值**：固定编译期常量，实现期以照片目视定（`S_x/S_y` 是运行期 `W/w`、`H/h`，非常量、公式固定）。是否随目标尺寸自适应留后。
- **Gerstner golden**：已定用合成小夹具（8×8、无二进制照片）——canonical arm64 字节或跨机结构不变量，守那 5 个确定性机制（见 golden-tests 增量、tasks 7.4）。
- **裁剪**：Gerstner 与 Staged 走同一 `crop_center` 同参（spec 已钉），避免两模式对同 `width/height` 裁剪不一致。
- **capability 命名**：已从 `pattern-generation` 改为 `gerstner-superpixel`；`GeneratorKind` 选择语义移入 `pipeline`。
