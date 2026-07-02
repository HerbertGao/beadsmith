## 为什么

引擎在 Post-M9 已落地三项能力——调色板感知减色（`max_colors`）、去斑（`despeckle`）、Gerstner 生成模式（`generator`），CLI 均已暴露对应旗标。但 `bead-ffi` 的桥接边界仍停留在 M8 刻意设定的「仅 `width`/`height`」，其余选项一律取引擎 `Default`。结果是：这些能力对 Flutter App **不可达**，用户既不能把图案压到手头豆子套装的颜色数，也不能去杂点或切到照片模式。

ROADMAP「Post-M9 — Mobile UI Refinement」要在设置屏加这三个控件，而控件在 FFI 放宽之前是死的。本变更就是那条前置依赖：把边界从「仅 `width`/`height`」放宽为「`width`/`height` + 三个可选项」，取代 M8 的窄边界。

## 变更内容

- **BREAKING**（边界签名）：`bead-ffi` 的 `generate` 在 `width`/`height` 之外新增三个**可选**入参，透传进 `GenerateOptions`：
  - `max_colors: Option<u32>` — `Some(n)` 减到 ≤n 色，`None` 跳过（引擎默认）。
  - `despeckle: Option<u32>` — `Some(s)` 合并 ≤s 豆的同色连通块，`Some(0)` 合法空操作，`None` 跳过。
  - `generator`（镜像 `GeneratorKind`：`staged` | `gerstner`）— 默认 `staged`。
- 边界仍**只**开放这三项 + 尺寸。`filter`/`cell_size`/`shape`/`matcher` 继续不暴露：CLI 对前三者无法表达非默认值（暴露即使「CLI == FFI」对非默认输入不可测），`matcher` 本轮移动端仍只承诺默认 Oklab。放宽的边界恰好对齐 **CLI 已暴露且 UI 需要** 的集合。
- 桥仍是零逻辑薄桥：三参数原样构造进 `GenerateOptions` 后调 `generate_pattern`，不新增任何算法。`generator` 的 FFI 镜像→core 枚举映射与 CLI 的 `From<CliGenerator>` 同性质，属平凡 marshalling 而非业务逻辑。
- 扩展「CLI == FFI 逐字节」Dart 测试：新增「设置了选项」的对账用例（对同机 `bead-cli` 加 `--max-colors`/`--despeckle`/`--generator` 的输出逐字节比较）。

### 非目标

- 不改引擎任何算法（减色/去斑/Gerstner 已就绪，本变更只做透传）。
- 不暴露 `filter`/`cell_size`/`shape`/`matcher`。
- 不做设置屏 UI / 不改 `apps/mobile` 界面（属 ROADMAP 另一工作流「四屏重写」）。
- 不引入抖动（算法 Phase 4，仍推迟）。
- 不碰移动端打包/签名/发布。

## 功能 (Capabilities)

### 新增功能
<!-- 无新增功能：本变更修改既有 flutter-ffi 契约。 -->

### 修改功能
- `flutter-ffi`: 「桥接边界契约」由「M8 仅 width/height」放宽为「width/height + `max_colors`/`despeckle`/`generator` 三个可选项」；「CLI 与 FFI 同机逐字节相等」需求扩展到覆盖「选项已设置」的对账用例；「移动端打包·边界与桥逻辑零改动」场景中「仍只 width/height」的表述随之修订为「尺寸 + 三可选项，其余档位仍不暴露」。

## 影响

- **代码**：
  - `crates/bead-ffi/src/api.rs` — `generate` / `generate_inner` 签名加三参数；`GenerateOptions` 构造从 `{ width, height, ..Default::default() }` 改为填入三项；新增 `GeneratorKind` 的 FFI 镜像枚举与映射。in-crate `#[test]` 增补选项透传用例。
  - `apps/mobile` 的 Dart 桥封装（`bead_bridge.dart` / `pattern_engine.dart` 调用点）随生成的 FRB 绑定更新签名；调用方暂以「全 `None`/`staged`」传入，保持现行为不变（UI 控件属后续工作流）。
  - 「CLI == FFI」Dart 测试扩展。
- **bead-core**：零改动（`GenerateOptions` 字段早已就绪）。**里程碑**：Post-M9（Mobile UI Refinement 之 FFI 放宽），不涉及 M0–M9 重排。**受影响模块**：仅 `pipeline`（作为既有入口被调用，不改其实现）。
- **确定性**：
  - 三项均**未设置**（`None`/`None`/`staged`）时，`GenerateOptions` 等价于当前 `..Default::default()`，输出与现 FFI、及不带旗标的 CLI **逐字节一致**——既有闸门不回退。
  - **设置**后，等价于 CLI 带对应旗标。决定性档位：`despeckle` 是纯整数路径（跨架构位精确）；`max_colors` 在默认 Oklab 下复用 matcher 的 **f32 感知度量**（`GreedyReducer` 的 `ColorSnapshot::Perceptual`，非整数），与 `generator=gerstner` 同属 f32 **同机 canonical**（非跨目标 byte-exact）。但本轮对账均为**同机** FFI-vs-CLI 比较，host 同 libm 下 f32 与整数路径都逐字节一致，故三者闸门都成立——沿用 spec 既有决定性边界。
  - **越界选项**：`max_colors=Some(0)` 会被引擎 `GreedyReducer::new` 拒为 `InvalidImage`（与 `despeckle=Some(0)` 合法空操作**不对称**），经既有边界扁平化抛为 Dart 异常，与 CLI `--max-colors 0` 一致，桥层不新增校验。
- **依赖**：不新增任何 Rust/Dart 依赖（`GeneratorKind` 已在 `bead-core` 公共 API；FRB 镜像用现有 flutter_rust_bridge 能力）。
