## 上下文

`add-perceptual-matching` 把 `LabMatcher`（CIELAB + ΔE76）设为引擎默认配色，但 CIELAB 在蓝→紫区有已知感知畸变——不同蓝/紫被压到相近 Lab 坐标，配色易选错豆。Oklab（Björn Ottosson, 2020）是为修正这一畸变设计的感知色彩空间。

当前 `pipeline::generate_pattern` 把 `LabMatcher` 硬编码（`matcher/mod.rs` 已有 `RgbMatcher`/`LabMatcher` 两个 `ColorMatcher` 实现，trait 是 object-safe、以 `&dyn` 传给 `match_pattern`，但全工程只 new 过 `LabMatcher`）。matcher 选择对调用方不可见：`GenerateOptions` 无字段、CLI 无 flag。

约束：ARCHITECTURE.md 五条硬规则；可替换算法走 `ColorMatcher` trait、不改 `generate_pattern` 主流程形态；确定性是闸门；本项目**无历史用户**。

## 目标 / 非目标

**目标：**
- 新增 `OklabMatcher`（算法 Phase 3，与 `LabMatcher` 同档），蓝紫区更准。
- 引入 matcher 选择层（`MatcherKind` + `GenerateOptions.matcher` + `--matcher`），让三个 matcher 可被显式选用——既为 A/B 验证，也为后续开放给前端用户。
- 翻引擎默认 Lab→Oklab。

**非目标：**
- 不加输出溯源字段（`pattern.json` 维持六键）。
- 不在 FFI / 移动端新增 matcher 入参或 UI（现有 FFI 继续 width/height-only，随 `GenerateOptions::default()` 使用 Oklab）。
- 不引入 ΔE94/CIEDE2000/抖动。
- 不暴露 resize/render 为 flag（只开 matcher 一个口子）。

## 决策

### D1：Oklab + 欧氏 ΔEok²，挂现有 `ColorMatcher` trait
`srgb_to_oklab` 复用 `srgb_to_lab` 的第一步（`/255` → sRGB 反 gamma 线性化），换后续：linear sRGB → LMS（M1 矩阵，全正系数）→ 裸 `cbrt`（**无** 6/29 阈值分段、**无**白点归一化除法，比 Lab 更简）→ LMS' → Oklab（M2 矩阵）。距离侧**逐字照搬** `LabMatcher`：欧氏平方和（`(ΔL)²+(Δa)²+(Δb)²`）、不开方（`√` 单调保序）、严格 `<` 更新 → 最低下标平局。构造守卫复用 `check_palette_len`（空/超 65536 拒绝），存顺序保持的 Oklab 快照。
- **替代方案 ΔE94 / CIEDE2000**（留在 Lab 体系）：否决——ΔE94 性价比不及换空间；CIEDE2000 最准但最重（`atan2`、跨 libm 浮点面最大、收益递减）。
- **替代方案 维持 Lab**：否决——蓝紫畸变正是要解决的问题。

### D2：`MatcherKind` 枚举 + `GenerateOptions.matcher` + `Box<dyn ColorMatcher>`
`pipeline:108` 由 `LabMatcher::new(palette)?` 改为 `match opts.matcher` 构造 `Box<dyn ColorMatcher>`；`match_pattern` 已收 `&dyn`，天然兼容（trait 的 object-safety 到此才真正用上）。
- **替代方案 编译期 feature flag 切换**：否决——无法运行时 A/B，也无法开放给用户。
- **替代方案 `generate_pattern` 泛型化 `<M: ColorMatcher>`**：否决——trait 必须保持 object-safe（设计 D2），管线用 `dyn`；泛型会污染 FFI 边界。

### D3：默认翻为 Oklab（`MatcherKind::default() == Oklab`）
- **替代方案 保持 Lab 默认、Oklab 仅 opt-in**：否决——无历史用户需保护；Oklab 既是更优默认，做成"没人看得见价值的可选项"等于死选项。`LabMatcher`/`RgbMatcher` 不删，分别经 `--matcher lab`/`rgb` 保留为备选与跨架构整数基准。

### D4：不加 provenance 字段
`pattern.json` 维持 `{brand,width,height,cells,total,stats}` 六键、`summary.txt` 不变。
- **替代方案 加 `matcher` 字段**：否决——破坏六键 golden 断言；违背规则 3（matcher 在 `BeadPattern` 上游、不属 pattern）；与"现有输出对任何 option 都不记溯源"不一致。翻默认 = 静默改 `cells`，符合架构。

### D5：CLI `--matcher` 用 `clap::ValueEnum`（本仓库首个枚举 flag）
CLI 侧定义 `ValueEnum`，手写 `match` 映射到 core 的 `MatcherKind`（core 不依赖 clap）。沿用 `add-color-reduction` 的 CLI↔core 手映射风格。
- **替代方案 不暴露**：否决——defeats 选择层目的。
- **替代方案 收 `String` 自行解析**：否决——`ValueEnum` 免费给校验 + help + 非法值退出码 2。

### D6：`srgb_to_oklab` 与 `srgb_to_lab` 共享线性化
抽一个 `linearize(rgb) -> [f32;3]`（即现有 `lin()` 三连）供两者复用。
- **替代方案 复制 ~10 行**：可接受但留重复；倾向抽小 helper（细节留 apply）。

### D7：`rgb` 也暴露
- **替代方案 只 `lab|oklab`**：否决——`rgb` 几乎免费（impl 现成 + 枚举本就有该变体），白送一个"跨架构位精确/最快整数"选项。

## 风险 / 权衡

- **全量 golden 重烤**（4 文件随默认翻而变）→ `BLESS=1` 仅 arm64-Linux 重生；`samples/gradient.png` 是深蓝→紫渐变、落在 Oklab 差异区，diff 真实可见（fixture 见证变化，非哑证人）。
- **Oklab 跨架构非位精确**（`cbrt`/`powf` 跨 libm 不保证逐位）→ 与 `LabMatcher` **同档**，由 canonical=arm64-Linux 字节 golden + 同机 CLI==FFI 承担；**不新增**确定性类别。无新浮点风险面：全程纯 `*`/`+`/`-`、**无 `mul_add`/FMA**（满足 T4 的 CLI==FFI codegen 不分叉），M1 全正系数 × 非负线性 rgb ⇒ LMS≥0 ⇒ `cbrt` 有限、**无 NaN**。
- **打破 CLI 极简边界**（首个枚举 flag）→ 有意接受，是本变更目的；口子仅限 matcher，resize/shape 仍不暴露。
- **FFI 默认路径跟随变化**（已有 `bead-ffi` 用 `GenerateOptions { width, height, ..Default::default() }`）→ 不扩 FFI 入参，避免把 CLI 新增的 A/B flag 直接推到移动边界；CLI==FFI 闸门继续对比“不传 `--matcher`”的默认路径，即 Oklab。`--matcher lab|rgb` 是 CLI/core A/B 能力，不是本轮 FFI/mobile 能力。
- **文档一致性传播**（review-loop 必查）→ 见 tasks：`ARCHITECTURE.md` matcher 模块 + 确定性段、主规范 `color-matching`/`pipeline`/`cli`/`golden-tests`/`flutter-ffi` 的「默认 LabMatcher」措辞、`lib.rs` 导出、golden 与 FFI 注释。

## 迁移计划

无历史用户 → 翻默认与 `GenerateOptions` 结构变更无迁移成本。若未来有调用方手写 `GenerateOptions` struct literal，迁移为补 `matcher: MatcherKind::default()` 或使用 `..Default::default()`。回滚：还原代码 + `BLESS=1` 重生回 Lab 默认的 golden 即可。

## 待解问题

- 是否在 `gradient.png` 之外再补一张蓝紫为主的 golden/证据 fixture，把 Oklab 卖点拍得更实？（倾向：`oklab_differs_from_lab_in_blue` 单测已足够钉证据，额外 fixture 可选，apply 阶段定。）
