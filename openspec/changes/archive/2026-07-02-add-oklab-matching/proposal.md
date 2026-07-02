## 为什么

`add-perceptual-matching` 让 `LabMatcher`（CIELAB + ΔE76）成为引擎默认配色，但 CIELAB 在**蓝→紫区**存在已知的感知畸变：不同的蓝/紫会被压到相近的 Lab 坐标，配色时容易选错豆。Oklab 正是为修正这一畸变而设计的感知色彩空间，蓝紫区更准，且与 `LabMatcher` 同构——只换色彩空间转换、距离侧照搬，`cbrt` 已在用、不引入新的跨架构浮点面，是配色精度性价比最高的下一步。

同时，引擎目前把 `LabMatcher` **硬编码**在 `pipeline::generate_pattern`，matcher 选择对调用方完全不可见（`GenerateOptions` 无字段、CLI 无 flag）。本变更顺势引入 matcher 选择层，让 Oklab/Lab/RGB 可被显式选用——这既是 A/B 验证 Oklab 效果的手段，也为后续把配色选择开放给前端用户铺路。

本项目无历史用户，翻默认无迁移包袱。

## 变更内容

- 新增 `OklabMatcher`（算法 **Phase 3** 感知配色，与 `LabMatcher` 同档）：Ottosson Oklab 色彩空间 + 欧氏 ΔEok² 距离，挂现有 `ColorMatcher` trait，扩 `crates/bead-core/src/matcher/mod.rs`（不新建模块）。
- 新增 `MatcherKind { Rgb, Lab, Oklab }` 枚举，`GenerateOptions` 增 `matcher` 字段；`generate_pattern` 由硬编码 `LabMatcher::new` 改为按 `matcher` 构造 `Box<dyn ColorMatcher>`。
- **BREAKING（无外部用户）**：引擎默认 matcher 从 `LabMatcher` 翻为 `OklabMatcher`（`MatcherKind::default() == Oklab`）。同 `image+palette+dimensions+默认options` 的 `cells`/统计/渲染逐字节改变；Rust API 侧 `GenerateOptions` 新增必填字段 `matcher`（非 `#[non_exhaustive]` struct，手写 struct literal 的调用方需补字段或用 `..Default::default()`）。
- CLI 新增 `--matcher rgb|lab|oklab`（默认 `oklab`）——本仓库首个暴露 core 枚举的 flag，引入 `clap::ValueEnum` + 手写映射到 core 枚举。
- `LabMatcher` 保留为 `--matcher lab` 备选（不删，留作对比与可选）；`RgbMatcher` 继续作跨架构整数基准，并经 `--matcher rgb` 转为可选项。
- 新增证据测试 `oklab_differs_from_lab_in_blue`，钉住「Oklab 在蓝紫区与 Lab 选不同豆」。
- 全量重烤 golden（4 个文件随默认翻而变）。

### 非目标

- **不**加输出溯源字段：`pattern.json` 维持 `{brand,width,height,cells,total,stats}` 六键、`summary.txt` 形状不变。matcher 在 `BeadPattern` 上游、不属于 pattern 本身（规则 3），且现有输出对任何 option（resize/shape/max-colors）都不记溯源，加 matcher 字段既破坏六键、又违背既有设计。
- **不**在 FFI / 移动端新增 matcher 入参或 UI：现有 `bead-ffi` 继续只接 `imageBytes/paletteJson/width/height`，用 `GenerateOptions { width, height, ..Default::default() }` 跟随引擎默认 Oklab；CLI==FFI 闸门只比较 CLI 默认路径，不覆盖 `--matcher lab|rgb`。
- **不**引入 ΔE94 / CIEDE2000 / 抖动（Floyd–Steinberg 是算法 Phase 4，本档不提前引入）。
- **不**暴露 resize filter / render shape 为 flag（维持 CLI 既有极简边界，本次只开 matcher 一个口子）。

## 功能 (Capabilities)

### 新增功能
<!-- 无：Oklab 是 color-matching 既有能力内的新需求，非独立能力。 -->

### 修改功能
- `color-matching`: 新增 `OklabMatcher`（Oklab + ΔEok²）需求；默认 matcher 改为 Oklab，`LabMatcher` 降为备选；新增 matcher 选择语义（`MatcherKind`）。确定性档位不变——Oklab 与 Lab 同属 `f32`（`cbrt`/`powf`）同机 canonical=arm64-Linux 字节 golden 档，非跨架构位精确。
- `pipeline`: `GenerateOptions` 增 `matcher: MatcherKind` 字段；`generate_pattern` 按字段选 `Box<dyn ColorMatcher>`；默认链浮点源说明由「默认 `LabMatcher`」改为「默认 `OklabMatcher`」（仍 `cbrt`/`powf`，确定性界不变）。
- `cli`: `generate` 子命令新增 `--matcher rgb|lab|oklab`（默认 `oklab`），非法值非零退出（参数错误，退出码 2）；映射到 `GenerateOptions.matcher`。
- `golden-tests`: 默认路径从 `LabMatcher` 改为 `OklabMatcher`，固定设置显式包含 `MatcherKind::default()==Oklab`；重烤四个 golden。
- `flutter-ffi`: FFI 边界保持 width/height-only，不暴露 matcher；默认路径随 `GenerateOptions::default()` 变为 Oklab，CLI==FFI 对账命令仍不传 `--matcher`。

## 影响

- **bead-core 模块**：`matcher`（新 `OklabMatcher` + `srgb_to_oklab` + 证据测试）、`pipeline`（`GenerateOptions.matcher` + 选择分支 + `Box<dyn>`）、`lib.rs`（导出 `OklabMatcher`、`MatcherKind`）。
- **bead-cli**：`main.rs`（`--matcher` clap `ValueEnum` + 映射）、`tests/cli.rs`（各值 ok / 非法值拒绝）。
- **bead-ffi / Dart**：不改 API 字段集合；更新文档注释/规范默认路径说明，重跑 host CLI==FFI 决定性闸门以证明默认 Oklab 路径同机逐字节一致。
- **确定性**：默认链浮点源仍为 `Lanczos3` f32 + 感知 matcher 的 `cbrt`/`powf`，跨架构非位精确不变，canonical=arm64-Linux 字节 golden + 同机 CLI==FFI 兜底照旧；无新浮点风险面（无 `mul_add`/FMA，LMS≥0 保证 `cbrt` 有限、无 NaN）。
- **Golden**：`tests/golden/*`（`pattern.json`/`summary.txt`/`preview.png`/`grid.png`）随默认翻自动重烤，`BLESS=1` 仅 arm64-Linux 重生；`samples/gradient.png`（深蓝→紫渐变）落在 Oklab 差异区，diff 真实可见。
- **依赖**：无新增。`clap` 已启用 derive（`Parser`/`Subcommand` 在用），`ValueEnum` derive 属同一 feature。
- **文档**：`ARCHITECTURE.md`（matcher 模块描述 + 确定性段把 Oklab 归入 Lab 同档）、`tests/golden/README.md`、`crates/bead-cli/tests/golden.rs` 注释、`crates/bead-ffi` Rust/Dart 注释、`openspec/specs/{color-matching,pipeline,cli,golden-tests,flutter-ffi}.md` 主规范（经归档同步）。
