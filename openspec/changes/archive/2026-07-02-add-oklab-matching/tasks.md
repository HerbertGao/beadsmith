## 1. bead-core — matcher 模块（`crates/bead-core/src/matcher/mod.rs`）

- [x] 1.1 抽 `linearize(rgb: [u8;3]) -> [f32;3]`（即现有 `srgb_to_lab` 的 `lin()` 三连），让 `srgb_to_lab` 改用它；保持现有 Lab 测试全绿（`cargo test -p bead-core matcher`）
- [x] 1.2 实现 `srgb_to_oklab(rgb) -> [f32;3]`：`linearize` → LMS（M1 全正矩阵）→ 裸 `cbrt` → Oklab（M2 矩阵），纯 `*`/`+`/`-`、**无 `mul_add`**
- [x] 1.3 实现 `OklabMatcher` struct（顺序保持的 Oklab 快照）+ `new`（复用 `check_palette_len`）+ `impl ColorMatcher`（欧氏平方和、跳 sqrt、严格 `<` 最低下标平局），骨架照搬 `LabMatcher`
- [x] 1.4 新增 `MatcherKind { Rgb, Lab, Oklab }`（`#[derive(Debug,Clone,Copy,PartialEq,Eq,Default)]`，`#[default] Oklab`），放 matcher 模块
- [x] 1.5 单测：sRGB→Oklab 已知值（黑 L≈0 / 白 L≈1 中性 / 标准红有限值无 NaN）、精确命中=0、Oklab 平局取最低下标、构造守卫（空/65537 拒绝、65536 接受）— 抄 `LabMatcher` 测试改 oracle（`cargo test -p bead-core`）
- [x] 1.6 证据测试 `oklab_differs_from_lab_in_blue`：蓝/紫区取一像素 + 含多个邻近蓝候选的调色板，断言 `OklabMatcher` 与 `LabMatcher` 选**不同**下标（抄 `lab_off_palette_can_differ_from_rgb` 范式，matcher/mod.rs:554）

## 2. bead-core — pipeline（`crates/bead-core/src/pipeline/mod.rs`）

- [x] 2.1 `GenerateOptions` 加 `matcher: MatcherKind` 字段；`Default` 填 `MatcherKind::default()`（=Oklab）；更新「默认选项填充」相关单测断言 `matcher==Oklab`
- [x] 2.2 `generate_pattern` 把硬编码 `LabMatcher::new(palette)?` 改为 `match opts.matcher` 构造 `Box<dyn ColorMatcher>`（Rgb/Lab/Oklab 各自 `new`），以 `&dyn` 喂 `match_pattern`
- [x] 2.3 更新/补 pipeline 单测：默认路径用 Oklab；显式三 `MatcherKind` 各产出合法 `BeadPattern`；`generate_pattern_chains_faithfully` 的手工原语链按 `opts.matcher` 选择同一个 matcher（`cargo test -p bead-core pipeline`）
- [x] 2.4 更新 pipeline 错误/序列化测试：空 palette 经 `MatcherKind::Rgb`/`Lab`/`Oklab` 均透传 `InvalidPalette`（无新增变体）；`pattern_json` 的 `Σ stats.count == total` 定理仍锚定“所选 matcher 的同序调色板快照”，不写入 matcher provenance（`cargo test -p bead-core pipeline`）

## 3. bead-core — 导出（`crates/bead-core/src/lib.rs`）

- [x] 3.1 `pub use` 导出 `OklabMatcher`、`MatcherKind`（沿用 `add-color-reduction` 导出 `Quantizer`/`MedianCutQuantizer` 的位置与风格）；`cargo build -p bead-core` 通过

## 4. bead-cli（`crates/bead-cli/src/main.rs` + `tests/cli.rs`）

- [x] 4.1 `generate` 加 `--matcher`：CLI 侧 `#[derive(clap::ValueEnum)]` 枚举（rgb/lab/oklab，`default_value = "oklab"`）+ 手写 `match` 映射到 core `MatcherKind`，写入 `opts.matcher`
- [x] 4.2 集成测试 `tests/cli.rs`：`--matcher rgb|lab|oklab` 各退出 0 写四文件；`--matcher hsv` 非法值退出码 **2**、stderr 含可选值、不 panic；不给 `--matcher` 时等同 oklab（`cargo test -p bead-cli --test cli`）

## 5. bead-ffi / Dart 默认路径（`crates/bead-ffi/src/api.rs` + `crates/bead-ffi/dart/lib/src/api.dart` + `crates/bead-ffi/dart/test/determinism_gate_test.dart`）

- [x] 5.1 不给 FFI/Dart API 新增 matcher 入参；更新 Rust/Dart 文档注释，说明 `GenerateOptions { width, height, ..Default::default() }` 现在等价于默认 `Lanczos3` / `cell_size=10` / `shape=Square` / `matcher=Oklab`
- [x] 5.2 重跑 host FFI 决定性闸门：`cargo build -p bead-ffi` 后执行 Dart `CLI == FFI` 测试，确认 FFI 默认 Oklab 路径与不传 `--matcher` 的 `bead-cli generate` 在同机四产物逐字节一致；不要求也不测试 `--matcher lab|rgb` 的 FFI 对账

- [x] 6.1 更新 golden 测试/README 注释，把默认 `LabMatcher` 改为默认 `OklabMatcher`，固定设置显式包含 `MatcherKind::default()==Oklab`
- [ ] 6.2 在 arm64-Linux 跑 `BLESS=1 cargo test -p bead-cli --test golden` 重生四个 golden（`pattern.json`/`summary.txt`/`preview.png`/`grid.png`），随默认翻 Oklab 而变
- [x] 6.3 人工核对重烤 diff：`gradient.png`（深蓝→紫）的 `cells`/`summary.txt` 用色相对 Lab 默认确有变化（证明默认真翻、fixture 见证），非 canonical 平台结构不变量测试仍绿

## 7. 文档一致性（review-loop 必查）

- [x] 7.1 `ARCHITECTURE.md`：matcher 模块描述加 `OklabMatcher`（Phase-3 新默认、Lab 降备选）；确定性段把 Oklab 归入 Lab 同档（`cbrt`/`powf`、canonical arm64、无新浮点面）
- [x] 7.2 sync/archive 阶段更新主规范 prose：`openspec/specs/color-matching/spec.md` / `pipeline/spec.md` / `cli/spec.md` / `golden-tests/spec.md` / `flutter-ffi/spec.md` 把「默认匹配器为 `LabMatcher`」改为 `OklabMatcher`，让 pipeline 的 `pattern_json` 定理/错误透传措辞改为“由 `opts.matcher` 选定的 matcher”，并明确 FFI 不暴露 matcher、CLI==FFI 只比较默认 Oklab 路径（增量需求已在本 change 的 specs/ 落，目的段在 `/opsx:sync` 或归档时同步）

## 8. 收尾验证

- [x] 8.1 全量 `cargo test`（含 core 单测 + cli 集成 + golden 结构不变量）全绿
- [ ] 8.2 `cargo run -p bead-cli -- generate ... --matcher oklab` 与 `--matcher lab` 实拍对比同一蓝紫图，肉眼确认 Oklab 蓝区配色更自然（产品验收）
