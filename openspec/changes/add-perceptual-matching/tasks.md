## 1. LabMatcher 实现（bead-core/src/matcher）

- [x] 1.1 新增 sRGB→Lab 转换：`fn srgb_to_lab(rgb: [u8; 3]) -> [f32; 3]`（/255 → 反 gamma 线性化 → XYZ（sRGB/D65 矩阵）→ L\*a\*b\*，含 6/29 阈值分段 + `cbrt`）。私有函数。
- [x] 1.2 新增 `pub struct LabMatcher { colors: Vec<[f32; 3]> }`，构造 `pub fn new(palette: &Palette) -> Result<LabMatcher, BeadError>`：复用 `RgbMatcher::new` 的空 / >65536 守卫（同 `InvalidPalette` reason 文案），把各色一次性 `srgb_to_lab` 存顺序快照。
- [x] 1.3 `impl ColorMatcher for LabMatcher`：`find_best_match` 把 target 转 Lab、线性扫描比较 Lab 平方差之和（**无 sqrt**）、严格 `<` 更新、最低下标平局；全函数、不 panic、不产 NaN。实现用普通 IEEE 浮点运算，**禁用** `mul_add`/FMA 收缩（避免 CLI 二进制 vs FFI staticlib/cdylib codegen 分歧破坏同机字节相等，T4）。
- [x] 1.4 更新 `matcher` 模块头注（**拆分，勿一刀切**）：「no f32」特指 `RgbMatcher`（`LabMatcher` 引入 f32）；「最近距离比较 no sqrt」**仍适用所有匹配器**（LabMatcher 比平方 ΔE76）。同时把头注 `matcher/mod.rs:5` 的「Phase 2's CIELAB/ΔE matcher」改为「Phase 3」（与 INIT「算法 Phase」轴一致，见 §6.2）。

## 2. 单元测试（matcher 模块内）

- [x] 2.1 Lab 转换已知值校验：纯黑 [0,0,0]→L≈0、纯白 [255,255,255]→L≈100、若干已知 sRGB→Lab 参考值在 `f32` 容差内。
- [x] 2.2 精确命中：像素 == 调色板某色 RGB → 返回该下标（距离 0）；同 RGB 多色 → 最低下标。
- [x] 2.3 离色取感知最近 + **与 RgbMatcher 可不同**：构造一个像素，断言 `LabMatcher` 与 `RgbMatcher` 在该像素返回不同下标（证明是感知匹配而非 RGB 别名）。
- [x] 2.4 Lab 平局取最低下标；构造守卫拒绝空 / 65537 色、接受 65536 色（镜像 RgbMatcher 既有测试）。
- [x] 2.5 同机重复匹配 `cells` 逐字节一致。

## 3. 默认匹配器替换（bead-core/src/pipeline）

- [x] 3.1 `pipeline/mod.rs:84` 把 `RgbMatcher::new(palette)?` 改为 `LabMatcher::new(palette)?`；更新 `use` 导入；其余链 / 单一-Palette / 错误透传不动。
- [x] 3.2 同步更新 pipeline 模块内复述「各原语」的文档注释里提到 matcher 的措辞（如有），与规范一致。
- [x] 3.3 跑 `cargo test -p bead-core`：管线既有测试（忠实串联 / 单一 Palette / 错误透传 / 确定性）应仍通过；若 `generate_pattern_chains_faithfully` 用 `RgbMatcher` 重算对比，改为 `LabMatcher`。

## 4. Golden 重 bless（canonical = arm64 Linux）

- [x] 4.1 在 **arm64 Linux**（CI `ubuntu-24.04-arm` 或 Apple Silicon 原生容器）经 `crates/bead-cli/tests/golden.rs` 的 `BLESS` 路径重生 `tests/golden/` 四样 master（pattern.json / summary.txt / preview.png / grid.png）。**禁止**在 x86 / macOS / Windows 上 bless。（已在 Apple Silicon 的 `--platform linux/arm64 rust:latest` 容器内 `BLESS=1 --locked` 重生；非-BLESS `golden_matches_canonical` 在该容器内通过。）
- [x] 4.2 审阅 golden diff：确认变化是「RGB→Lab 配色改变」导致的合理像素 / 统计差异，而非意外结构破坏。（summary：S13 Black 247→190、新增 S64 Black Rock 110、S91 73→20；cells 把近黑灰格由纯黑 12 改配 Black Rock 63；Total 320、下标合法、结构不变。）
- [x] 4.3 非 canonical 平台跑 golden 测试，确认结构不变量仍通过（PNG 解码 / 尺寸 / 键集 / 计数 / 下标）。

## 5. 全量验证

- [x] 5.1 `cargo build && cargo test --workspace --all-features` 全绿。
- [x] 5.2 确认 `pipeline::generate_pattern` / CLI flags / FFI 边界签名**零改动**；FFI 「CLI == FFI 同机逐字节」测试（M8）仍通过。
- [x] 5.3 `cargo run -p bead-cli -- generate` 实跑一张样图，肉眼对比 preview 感知配色较 RGB 版有改善。（UncleGao.png @96×96：RGB 把肤色配成黄绿、Lab 配成自然暖肤；23.3% 格不同、Lab 用色 49 vs RGB 71，更聚合。用户已确认改善。）
- [x] 5.4 确认 §6.1/6.2/6.4/6.5 的纯文档同步已落地：grep 全仓无残留把 matcher 当作「跨架构整数位精确」、把 Lanczos3 当作「唯一浮点源」、或把 golden master 当作「Phase-1 engine 输出」的旧声明。注：grep 范围**仅限** §6.1/6.2/6.4/6.5 的直接编辑目标文件（`ARCHITECTURE.md`、`tests/golden/README.md`、`golden-tests`/`flutter-ffi` 规范、`golden.rs`、`INIT.md`）；`openspec/specs/{color-matching,pipeline}` 的**全部内容（目的 + 本 change 的修改需求）**随归档/sync 才落地（晚于本 apply 阶段），其一致性在 archive 步骤单独核对，本 grep **不**断言这两份尚未 sync 的主规范（其当前 body 仍含被 delta 替换的旧 determinism 文案，属正常未同步态）。

## 6. 文档一致性传播（review 发现：改动只动了 2 份 delta + matcher 头注，下列等价旧声明须同步，否则归档后自相矛盾。可与 §1–§3 并行）

- [x] 6.1 `ARCHITECTURE.md` Rule 3（约 :73-80，CLAUDE.md 硬规则）：把「Pure-integer paths (matcher, …) are bit-identical across architectures」中的 matcher **限定为 RgbMatcher**；默认 `LabMatcher` 归浮点列（与 Lanczos3 同：跨架构非位精确、canonical-arm64 字节 golden + 同机 CLI==FFI）。`flutter-ffi` 规范 `:172` 引用此条，一并核对。
- [x] 6.2 `ARCHITECTURE.md:156-157` matcher 模块 Phase 标号：「Phase 2: CIELAB」与 INIT「算法 Phase」轴冲突（INIT Phase 2 = Color Reduction/降色，是 quantizer 非 matcher；CIELAB+ΔE = Phase 3）→ 改为「Phase 3」并标注「已实现为默认」。与 §1.4 的 matcher 头注一致。
- [ ] 6.3 主规范 **目的** 段（归档/sync 时改写，delta 的需求块不覆盖目的）：`color-matching` 目的「逐格用 RGB 平方欧氏距离…纯整数…跨架构位精确」改为两档（RgbMatcher 整数跨架构基准；默认 LabMatcher f32/canonical-arm64）；`pipeline` 目的「默认 Lanczos3 f32 重采样非跨架构 byte 稳」补「+ LabMatcher」为第二浮点源。
- [x] 6.4 `tests/golden/README.md:3`「freeze the Phase-1 engine's output」改为「当前默认引擎（Lanczos3 + LabMatcher）输出」；`:44-48` 浮点源理由（「Lanczos3 … f32::sin」一句起于 :45）补 LabMatcher。
- [x] 6.5 仅理由文案、**无行为 delta**（normative 需求/场景不变，RC 已 diff 确认）：`golden-tests` 规范 `:4,28`、`flutter-ffi` 同机证明文本、`crates/bead-cli/tests/golden.rs:11-16` 注释、`INIT.md:287-288`（「since the default Lanczos3 resize runs f32::sin」）——凡把 Lanczos3 列为「唯一浮点源 / canonical 唯一理由」处补「+ LabMatcher（`cbrt`/`powf`）」。
