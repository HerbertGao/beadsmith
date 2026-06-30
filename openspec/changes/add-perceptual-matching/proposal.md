## 为什么

当前配色（M3 起）用 **RGB 平方欧氏距离**取最近调色板色。RGB 空间与人眼感知不一致：相同的数值距离在不同色域观感差异很大，导致拼豆成品在中间调、肤色、低饱和区出现可见的「配错色」。这是当前引擎输出质量的主要短板（**设计假设**——感知收益待 apply 阶段以样图对比验证，见 tasks §5.3）。

`bead-core` 早为此预留了扩展点：`matcher` 模块的 `ColorMatcher` trait（object-safe）文档里直接写明「CIELAB/ΔE matcher is the known second implementation」。M0–M9 已收尾、引擎已被 M7 golden 与 M8 「CLI == FFI 同机逐字节」闸门冻结，现在是把这个早就规划好的第二实现落地的时机——这是 INIT.md「Pattern Generation」里的**算法 Phase 3（CIELAB + ΔE）**。

## 变更内容

- 新增 `LabMatcher`：实现既有 `ColorMatcher` trait，用 **CIELAB + ΔE76**（Lab 空间欧氏距离）取最近色，复用 `RgbMatcher` 既有的最近搜索骨架（比较距离、严格 `<` 更新、平局取最低下标）。Lab 转换与距离用浮点（`f32`，纯 std 数学，无新依赖）。
- **懒路默认升级**：`pipeline::generate_pattern` 内部由 `RgbMatcher::new(palette)` 改为构造 `LabMatcher::new(palette)`，使感知匹配成为**所有前端（CLI / FFI）的默认且唯一**行为。**不**新增 `GenerateOptions` 选择位、**不**加 CLI flag、**不**动 FFI 边界（YAGNI——目前无「运行时切换匹配器」需求）。
- 保留 `RgbMatcher` 不删：它仍是 trait 的 Phase-1 整数实现、其跨架构位精确 golden 与单测继续作为确定性基准与回归护栏。
- 更新 `matcher` 模块不变量文档（**拆分，勿一刀切**）：「no f32」自此**特指 `RgbMatcher`**（`LabMatcher` 显式引入 f32）；「最近距离比较 no sqrt」**仍适用所有匹配器**（`LabMatcher` 比较平方 ΔE76，同样不开方）。
- golden masters 在 **canonical 平台（arm64 Linux）重新 bless**：默认匹配器改变会使四样产物字节改变，这是 M7 设计的「故意算法改动 → 响亮失败」预期路径。

### 算法 Phase 声明

本变更属 **算法 Phase 3（CIELAB + ΔE）**，见 INIT.md「Pattern Generation」。不跨档提前引入 Phase 2（降色）或 Phase 4（抖动）。

### 对确定性的影响（关键）

- **同 image + palette + dimensions + options 在同机/同平台仍逐字节相同**——CLI == FFI 是**同机**闸门（flutter-ffi 规范：非 canonical 平台与「同机当场运行的 CLI」比对，而非字节 master），同机同 libm → 浮点结果一致，**该闸门不受影响**。
- **跨架构（arm64 ↔ x86_64）配色不再保证位精确**：CIELAB 的 `cbrt`/`powf` 跨 libm 实现不保证逐位一致——这与 M2 `Lanczos3` 的 `f32::sin` 是**同构**的既有情形，项目早已用「canonical = arm64 Linux 字节 golden + 其它平台结构不变量」模型吸收。canonical 选 arm64 Linux 是因为可在 Apple Silicon 原生容器稳定 bless（架构与移动端生产同族）；但该 golden 是**回归基准、非生产字节保真**——iOS（Apple libm）/ Android（Bionic libm）即便同为 arm64 也产生不同字节（见 `tests/golden/README.md`）。移动端生产正确性由**结构不变量 + 同机 CLI==FFI** 保证，不与 Linux master 比字节。
- 无随机、无 `rayon`、无迭代顺序泄漏仍是硬性要求；浮点是**唯一**新增的非确定性面，且被既有 canonical 平台模型覆盖。

## 功能 (Capabilities)

### 新增功能
（无新功能能力；感知匹配是既有 `color-matching` 能力的第二实现，作为修改纳入。）

### 修改功能
- `color-matching`: 新增「感知匹配器 `LabMatcher`（CIELAB + ΔE76）」需求；并将既有「确定性（含跨架构整数一致）」需求**重新限定**——纯整数/跨架构位精确特指 `RgbMatcher`，`LabMatcher` 的确定性按「同机/同平台逐字节 + canonical arm64 golden、跨架构非位精确」表述。
- `pipeline`: `generate_pattern` 的默认内部匹配器由 `RgbMatcher::new` 改为 `LabMatcher::new`（链顺序、单一 Palette 不变量、错误透传、下标值域定理均不变，仅默认实现替换）。

## 影响

- **代码**：
  - `crates/bead-core/src/matcher/mod.rs`：新增 `LabMatcher` + sRGB→Lab 转换 + ΔE76 距离；按上「拆分」更新模块头注不变量；头注 `:5` 的「Phase 2's CIELAB/ΔE matcher」改为「Phase 3」。
  - `crates/bead-core/src/pipeline/mod.rs:84`：默认匹配器替换。
  - `crates/bead-cli/tests/golden.rs` + `tests/golden/`：arm64 重新 bless 四样字节 master。
- **文档一致性传播（本次 review 补全——改动只动了 2 份 delta + matcher 头注，下列等价旧声明若不同步，归档后将自相矛盾）**：
  - `ARCHITECTURE.md` Rule 3（约 :73-80，CLAUDE.md 硬规则）：「Pure-integer paths (matcher, …) are bit-identical across architectures」中的 matcher 须**限定为 RgbMatcher**，默认 LabMatcher 归浮点列（`flutter-ffi` 规范 :172 引用此条）。
  - `ARCHITECTURE.md:156-157` matcher 模块 Phase 标号「Phase 2: CIELAB」与 INIT「算法 Phase」轴冲突（INIT Phase 2 = 降色/quantizer，CIELAB+ΔE = Phase 3）→ 改 Phase 3 并标「已实现为默认」。
  - `color-matching` / `pipeline` 主规范 **目的** 段（delta 的需求块不覆盖目的，须在归档/sync 时改写）：color-matching 目的「纯整数…跨架构位精确」改两档；pipeline 目的的浮点源补「+ LabMatcher」。
  - `tests/golden/README.md:3`「Phase-1 engine's output」改为「当前默认引擎（Lanczos3 + LabMatcher）」；浮点源理由补 LabMatcher。
  - 仅理由文案、**无行为 delta**（RC 已 diff 确认 normative 不变）：`golden-tests` 规范、`flutter-ffi` 同机证明、`golden.rs` 注释——凡把 Lanczos3 列为唯一浮点源处补「+ LabMatcher」。
- **API**：`pipeline::generate_pattern`、CLI flags、FFI 边界签名**均不变**（纯默认行为升级，前端零改动）。
- **依赖**：**无新增**。Lab 转换仅用 std 浮点数学（`f32::powf` / `f32::cbrt`）；`rayon` 仍不引入（单线程，性能并行属后期）。
- **确定性边界**：见上「对确定性的影响」——同机 CLI==FFI 闸门不变；跨架构配色精度沿用既有 canonical-arm64 模型，无需新机制。
- **里程碑**：引擎算法增强，属 ROADMAP「M9 之后的 Phase 2 algorithm work」批次（ROADMAP:187；注：此 Phase 2 是 ROADMAP 的平台后算法批次轴，与 INIT「算法 Phase 3 = CIELAB+ΔE」是不同编号轴，勿混）；不改 ROADMAP M0–M9 结构。
