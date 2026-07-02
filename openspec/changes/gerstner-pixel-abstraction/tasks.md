# 任务：Gerstner 像素化抽象（可选生成模式）

## 1. GeneratorKind 与选项

- [x] 1.1 定义 `GeneratorKind { Staged, Gerstner }`（`derive(Debug,Clone,Copy,PartialEq,Eq,Default)`，`#[default] Staged`），放 `crates/bead-core/src/gerstner/mod.rs` 或 pipeline 旁；从 crate 根 `pub use`（`crates/bead-core/src/lib.rs`）
- 1.2 `GenerateOptions` 增 `generator: GeneratorKind`，`Default` 填 `Staged`；更新「默认选项填充」单测断言 `generator==Staged`（`crates/bead-core/src/pipeline/mod.rs`）

## 2. gerstner 模块：确定性超像素（①核心）

- [x] 2.1 新建 `crates/bead-core/src/gerstner/mod.rs`，内部入口 `fn superpixel_assign(img: &RgbImage, palette: &Palette, width: u32, height: u32) -> Result<BeadPattern, BeadError>`（裁剪由 pipeline 先 `crop_center`；因上采样守卫/非法 palette 需返回 `Err`）
- [x] 2.2 确定性 SLIC（严格按 gerstner-superpixel 规范）：**实数 per-axis 步长** `S_x=W/w`/`S_y=H/h`；`w×h` 种子播源区中心（`(i+0.5)S_x,(j+0.5)S_y`）、**round-0 质心 = 中心最近整数像素（`round`=Rust `f32::round` ties-away）的 Oklab + 原始源像素位置**（质心存原始坐标、距离才归一，明确初值）；固定 `T` 轮无提前停；**每轮快照式「先全量分配、后全量更新」**；**候选集按原始网格锚定**（像素候选 = 其原始网格 cell 及 8-邻的种子、按下标非漂移位置 → `T>1` 仍无漏）；距离 = ΔOklab² + `m²·((Δx/S_x)²+(Δy/S_y)²)`、**平局取最小种子下标**；**质心 f32 行优先固定序累加**、**空簇保留上轮质心**。复用 `pub(crate) srgb_to_oklab`/`linearize`、**禁 `mul_add`/`rayon`/`HashMap`**
- [x] 2.3 收敛后每 cell 代表色 = 簇第 `T` 轮 **Oklab 质心**
- [x] 2.4 **上采样守卫**：裁剪后校验 `W>=w && H>=h`（`S>=1`），否则返回 `Err(InvalidImage)`（reason 点名 Gerstner target≤source）、不 panic、不进 `S<1` 退化。（**v1 不做**工作分辨率 clamp——见 design：clamp+rayon 留 Phase 2）

## 3. palette-constrained 贴板 + ≤N（①）

- [x] 3.1 每 cell 簇质心（Oklab）→ **对珠板 Oklab 快照做 ΔEok² argmin**（同 `OklabMatcher` 平局规则、同 `pub(crate) srgb_to_oklab` 快照，但**入参 Oklab 坐标**——**不是** `find_best_match`，后者只吃 RGB）贴到固定珠板，产**全板** `BeadPattern`（`cells` 均合法下标、**永不发明中间色**）
- [x] 3.2 确认 `max_colors==Some(n)` 由 pipeline 后段的 `GreedyReducer` 施加（gerstner **不**内置色数缩减，复用现减色语义）

## 4. pipeline 分支切换

- [x] 4.1 `generate_pattern` 按 `opts.generator` 分支（`crates/bead-core/src/pipeline/mod.rs`）：`Staged`=现路径（逐字节不变）；`Gerstner`=`crop_center` → `gerstner::superpixel_assign` → 全板 pattern；两分支后**共用**「可选 `GreedyReducer` 减色 →（可选 despeckle 去斑，若 `bead-despeckle` 已落地）→ `count_colors`/`generate_summary` → `render_*`」后段
- [x] 4.2 确认**不新增 `BeadError` 变体**（Gerstner 失败经既有 `ImageDecode`/`InvalidImage`/`InvalidPalette` 透传）、**不内联算法**（超像素在 gerstner 模块）

## 5. CLI

- [x] 5.1 `generate` 加 `--generator staged|gerstner`（`clap::ValueEnum`，`default_value = "staged"`）+ 手写 `match` 映射到 core `GeneratorKind`，写入 `opts.generator`（`crates/bead-cli/src/main.rs`）
- [x] 5.2 集成测试：`--generator staged|gerstner` 各退出 0 写四文件；`--generator <非二值>` 退出码 **2**、stderr 含可选值、不 panic；不给时等同 staged（`crates/bead-cli/tests/cli.rs`）

## 6. 单测（确定性/语义）

- [x] 6.1 Gerstner 产同形状 pattern（`width/height`、`cells.len()==w*h`、每格合法下标）
- [x] 6.2 Gerstner 同机重复**逐字节相等**（规则播种 + 固定 `T` + 固定平局 → 无非确定性）
- [x] 6.3 输出色都在珠板内（`cells` 无板外色）
- [x] 6.4 Gerstner + `max_colors==Some(n)` 上限成立（不同珠色数 ≤ n，经 `GreedyReducer`）
- [x] 6.5 `generator==Staged` 默认路径**逐字节不变**（对照「同 `opts` 走 Staged」/ 未引入 generator 分支前）
- [x] 6.6 **上采样守卫**：`target>source`（`S<1`）→ `Err(InvalidImage)`（reason 含 Gerstner 约束）、不 panic
- [x] 6.7 **每像素必被分配**（原始网格锚定）：构造 `T>1` 使种子漂移，断言输出无「未分配/未定义」cell（每格合法下标）；分配平局取最小种子下标可复现
- [x] 6.8 **max_colors==0 + 非法 palette** 在 `Staged` 与 `Gerstner` 下**都**先返回 `InvalidImage`（非 `InvalidPalette`）——减色器 fail-fast **在图像预处理之后、配色之前**（先于两模式的匹配器构造，不先于解码）；另测**零维**由顶层 ⓪ 守卫**先于解码**两模式一致
- [x] 6.9 `generate_pattern` 顶层先校验 `opts.width>0 && opts.height>0`（零维 `InvalidImage`、**先于 generator 分支与解码**、两模式一致）

## 7. 端到端 + 文档

- [x] 7.1 端到端跑 **一张真实照片**（`--generator gerstner`，多尺寸）+ UncleGao 目视：照片边缘/特征保留优于 `staged`；同机确定性一致（人工验收）
- [x] 7.2 `cargo build` / `cargo test` / `cargo clippy --all-targets` 全绿
- [x] 7.3 同步 `ARCHITECTURE.md`（新增「生成模式」段：`Staged` 默认 / `Gerstner` 照片路径 opt-in、走 `gerstner` 模块、palette-constrained、确定性同机）；`INIT.md`/`ROADMAP.md` 若需注明生成策略层
- [x] 7.4 golden：确认**默认 `Staged` golden 不变**；**为 Gerstner 加一份合成小夹具 golden**（手构 8×8 两色/渐变 `RgbImage`、**无二进制照片**）——canonical arm64 字节冻结，或跨机结构不变量（cell 数 = w×h、色都在板内、若干已知 cell 下标）；这守住 review 抓的 5 个确定性机制（同机重算单测证不了结构回归，见 SA 评审）
- [ ] 7.5 归档时更新主规范 `pipeline` 的 `## 需求:管线错误透传` 零维归因（当前写「reason 源自 `image_to_grid` 的目标维度守卫」）：改为「零维现由 `generate_pattern` **顶层 ⓪ 维度守卫**返回、**镜像** `image_to_grid` 的维度 reason（含 "target width"/"target height"）、**两 generator 一致**」——delta 未整体 MODIFY 该需求，故与 `目的` 段同属归档时 prose 对齐（F10，非行为矛盾、reason 仍满足既有断言）

## 8. 性能 + CLI==FFI

- [x] 8.1 bench 覆盖 Gerstner 路径（`crates/bead-core/benches/bench.rs`，固定 `T`、几个目标尺寸 + 源尺寸），**如实标注 v1 性能特征**：`O(源像素×T)`、大源图慢、缓解=预缩输入；clamp+rayon 留 Phase 2
- [x] 8.2 CLI==FFI 复核：FFI 默认路径**不含** `generator`（仍 `Staged`），「CLI==FFI」门只比较默认 `Staged` 路径、**不**覆盖 `--generator gerstner`（同 `--matcher lab/rgb` 的边界处理）
