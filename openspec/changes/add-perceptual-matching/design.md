## 上下文

配色（`color-matching` 能力）当前唯一实现是 `RgbMatcher`：对每格原始 RGB 取调色板中 RGB 平方欧氏距离最小者、平局取最低下标，全程纯整数、跨架构位精确。RGB 距离与人眼感知不一致，是输出质量短板。

`ColorMatcher` trait（`crates/bead-core/src/matcher/mod.rs`）是为第二实现预留的 object-safe 扩展点（`&dyn` / `Box<dyn>`），模块文档已点名 CIELAB/ΔE 为「known second implementation」。`pipeline::generate_pattern` 当前硬编码 `RgbMatcher::new(palette)`（pipeline/mod.rs:84），CLI 与 FFI 都只传 `GenerateOptions { width, height, ..Default::default() }`、不暴露匹配器选择。

确定性边界已有先例：M2 `Lanczos3` resize 用 `f32::sin`，跨架构非位精确，项目用「canonical = arm64 Linux 字节 golden + 其它平台只断言结构不变量」（golden-tests 规范）+「CLI == FFI **同机**逐字节」（flutter-ffi 规范，非 canonical 平台与同机当场运行的 CLI 比对而非字节 master）吸收。

## 目标 / 非目标

**目标：**
- 用 CIELAB + ΔE76 替代 RGB 距离，作为引擎**默认且唯一**的配色行为，提升感知配色质量。
- 复用 `RgbMatcher` 的最近搜索骨架（比较、严格 `<`、最低下标平局），新实现尽量小、与既有 impl 镜像。
- 前端零改动：`generate_pattern` / CLI flags / FFI 边界签名不变。
- 确定性沿用既有 canonical-arm64 模型，不引入新机制、不引入新依赖。

**非目标：**
- ΔE94 / ΔE2000 等更高阶色差（起步用 ΔE76；可后续在同 trait 后替换距离函数）。
- 运行时可选匹配器（不加 `GenerateOptions` 选择位 / CLI flag / FFI 边界）。
- 删除 `RgbMatcher`（保留为 trait 整数实现 + 跨架构位精确测试基准）。
- 把 Lab 转换改为定点整数以换取跨架构位精确（起步用浮点，定点是未来可选优化）。
- 算法 Phase 2（降色）/ Phase 4（抖动）；自定义白点 / ICC / 色彩管理；`rayon` 并行。

## 决策

**D1 — ΔE76（Lab 欧氏距离），不用 ΔE94/2000。**
ΔE76 = `√((ΔL)²+(Δa)²+(Δb)²)`，是 Lab 空间的欧氏距离。`√` 单调，**取最近时跳过开方、比较平方 ΔE 即可**保序——于是新实现与 `RgbMatcher` 的搜索骨架逐行同构（`u32`→`f32` 距离、严格 `<` 更新、最低下标平局），仅把「整数平方距离」换成「Lab 平方差之和」。
替代方案：ΔE2000 感知更准，但公式重（含 atan2、多项权重项）、跨 libm 浮点发散面更大、平局语义更复杂。**理由**：ΔE76 **预期**优于 RGB（感知收益是设计假设，待样图验证）、改动最小、确定性面最窄；ΔE2000 留作后续变更在同 trait 后替换距离函数。

**D2 — `f32`，不用 `f64`。**
Lab 转换用 `f32`（`f32::powf` / `f32::cbrt`，纯 std）。
**理由**：与既有 `Lanczos3` 的 `f32` 浮点路径一致；确定性是**同机**保证（同 libm → 同结果），f32/f64 都满足；对 u8 RGB 输入 + 数十至数百色调色板，f32 精度足以分辨最近色。f64 仅在追求跨架构位精确时才有意义，而那条路属定点整数（非目标）。

**D3 — 默认升级，不加选择位（懒路）。**
`generate_pattern` 内部 `RgbMatcher::new` → `LabMatcher::new`，其余链顺序 / 单一 Palette 不变量 / 错误透传 / 下标值域定理不变。
替代方案：给 `GenerateOptions` 加 matcher enum 并串到 CLI flag + FFI 边界。**理由**：当前无「运行时切换匹配器」需求，双实现可选会让 `GenerateOptions`→CLI→FFI 三处边界膨胀换取零当下收益（YAGNI）。感知配色本就该是所有人的默认。

**D4 — `LabMatcher` 构造时一次性快照 Lab。**
`LabMatcher::new(&Palette)` 把调色板各色 sRGB→Lab 转换一次，存 `Vec<[f32; 3]>`（顺序保持，下标 `i` ≡ `palette.colors[i]`，承载最低下标平局规则）；`find_best_match` 对 target 现转 Lab 再线性扫描。沿用 `RgbMatcher` 的快照值语义与构造守卫（空调色板 / `>65536` 色 → `InvalidPalette`，复用既有变体，不新增）。
**理由**：调色板色数远小于像素数，Lab 转换一次性摊销；与 `RgbMatcher` 构造/守卫/值语义一致，错误透传对 `pipeline` 透明（仅 `RgbMatcher::new`→`LabMatcher::new` 文案替换）。

**D5 — sRGB→Lab 标准管线，D65 白点。**
`u8`/255 → sRGB 反 gamma 线性化 → XYZ（sRGB/D65 矩阵）→ L\*a\*b\*（含 6/29 阈值分段 + `cbrt`）。`find_best_match` 全函数、对有界 u8 输入全程有限值、不产 NaN、不 panic。

**D6 — 确定性沿用既有 canonical-arm64 模型。**
同 image+palette+dimensions+options 在**同机/同平台**逐字节相同（CLI==FFI 同机闸门据此不受影响）；跨架构配色因 `cbrt`/`powf` 不保证位精确，由 canonical=arm64 Linux 字节 golden + 其它平台结构不变量吸收（与 `Lanczos3 f32::sin` 同构）。canonical 选 arm64 Linux 仅为可稳定 bless 的**回归基准、非生产字节保真**（iOS/Android libm 各异，见 README）；移动端正确性走结构不变量 + 同机 CLI==FFI。无随机 / 无 `rayon` / 无迭代顺序泄漏仍硬性。

## 风险 / 权衡

- **跨架构配色精度漂移**（边界像素在 arm64 与 x86_64 可能落到不同最近色）→ 既有 canonical-arm64 golden 模型吸收；canonical arm64 Linux 是稳定回归基准（**非生产字节保真**，iOS/Android libm 各异）；非 canonical 平台只断结构不变量。
- **f32 与 f64 在临界像素可能选不同色** → 确定性只要求同机一致；golden 锚 arm64；ΔE76 + 实际调色板规模下 f32 区分度足够。属可接受的实现选择，非正确性缺陷。
- **浮点平局（两色 ΔE 平方完全相等）** → 与 `RgbMatcher` 同：严格 `<` 更新、最低下标胜；浮点精确相等概率极低但规则仍确定可复现。
- **golden 重 bless 需 arm64 环境** → 用 CI `ubuntu-24.04-arm` 或 Apple Silicon 原生容器 bless；x86 机器**禁止** bless（golden README 已规定），否则提交错误 master。
- **`matcher` 模块「no f32」不变量被打破** → 「no f32」重新限定为**特指 `RgbMatcher`**（「最近距离比较 no sqrt」仍适用所有匹配器）；模块头注与 `color-matching` 规范同步更新，避免后人误读为整个能力仍纯整数。
- **同机 CLI==FFI 浮点字节相等的实现前提**（apply 阶段约束，T4）→ `LabMatcher` 实现须用普通 IEEE 浮点运算，**避免** `f32::mul_add` / FMA 收缩等可能在 CLI 二进制与 FFI staticlib/cdylib 间 codegen 分歧的操作，否则同机字节相等可能被破坏；落到 tasks §1.3 的实现约束 + §5.2 验证。
- **文档一致性传播被遗漏**（review 发现）→ 本变更只改了 2 份 delta + matcher 头注，`ARCHITECTURE.md` Rule 3 / Phase 标号、两份主规范 目的、`tests/golden/README.md` 仍持等价旧声明，归档后会自相矛盾 → 见 tasks §6 的同步清单。

## 迁移计划

1. 落地 `LabMatcher` + Lab/ΔE76 单测（Lab 转换已知值校验、精确命中、离色取最近、平局最低下标）。
2. pipeline 默认匹配器替换（pipeline/mod.rs:84）。
3. 在 **arm64 Linux** 经 golden 测试 `BLESS` 路径重生四样 master，审阅 diff（应是配色改变导致的合理像素/统计变化）。
4. 更新 `matcher` 模块头注不变量文案（按「拆分」+ Phase 3 标号）。
5. **文档一致性传播**（tasks §6）：`ARCHITECTURE.md` Rule 3 + Phase 标号、`color-matching`/`pipeline` 主规范 目的、`tests/golden/README.md`、`golden-tests`/`flutter-ffi` 浮点源理由——同步到「默认 LabMatcher 浮点、canonical-arm64」口径。

**回滚**：恢复 pipeline 那一行为 `RgbMatcher::new` + 还原 `tests/golden/` 四样 master（git revert）即可，`LabMatcher` 代码可保留不被调用。
