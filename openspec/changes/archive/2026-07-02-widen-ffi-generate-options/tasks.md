## 1. bead-ffi 边界放宽(Rust)

- [x] 1.1 在 `crates/bead-ffi/src/api.rs` 定义 `GeneratorKind` 的 FFI 镜像枚举(`Staged`|`Gerstner`,FRB `mirror` 或本地枚举 + `From` 映射到 `bead_core::GeneratorKind`),映射与 CLI 的 `From<CliGenerator>` 同性质、零算法。
- [x] 1.2 `crates/bead-ffi/src/api.rs`:给 `generate` 与 `generate_inner` 增加三个可选参数 `max_colors: Option<u32>`、`despeckle: Option<u32>`、`generator`(镜像枚举),并以 `GenerateOptions { width, height, max_colors, despeckle, generator, ..Default::default() }` 调 `generate_pattern`;不新增任何减色/去斑/生成逻辑(仍只调 `load_palette` / `generate_pattern` / `pattern_json`)。
- [x] 1.3 `crates/bead-ffi/src/api.rs` in-crate `#[test]`:断言「三项未设置(`None`/`None`/`Staged`)」构造出的 `GenerateOptions` 与 `{ width, height, ..Default::default() }` 产出的 `GenerateResult` 关键字段一致(默认路径不回退)。
- [x] 1.4 `crates/bead-ffi/src/api.rs` in-crate `#[test]`:断言「设置 `max_colors`(如 `Some(8)`)」相对未设置**改变**了 `stats` 的颜色数(证明选项确被转发进引擎);另断言镜像枚举 `Gerstner` 映射后走 Gerstner 路径(生成成功、结构不变量成立)。
- [x] 1.5 `crates/bead-ffi/src/api.rs` in-crate `#[test]`:断言新可达的越界值 `max_colors = Some(0)` 经 `generate` 返回 `Err(String)`(引擎 `InvalidImage` 已扁平化),桥层不 panic、不新增校验;与 `despeckle = Some(0)` 合法通过(不对称)对比。
- [x] 1.6 `cargo test -p bead-ffi` 通过;`cargo build` 全绿。

## 2. FRB 绑定重生成(Dart glue)

- [x] 2.1 用锁定的 `flutter_rust_bridge_codegen`(2.12.0,见 `crates/bead-ffi/Cargo.toml`)按 `crates/bead-ffi/flutter_rust_bridge.yaml` 重生成 `crates/bead-ffi/dart/lib/src/{api.dart,frb_generated.dart,frb_generated.io.dart}`。
- [x] 2.2 核对生成的 Dart `generate` 签名新增三个可选/命名参数(镜像枚举以 Dart enum 呈现);用同版本工具再跑一次确认 no-diff(与既有 task 4.2 的 no-diff 守卫一致)。

## 3. CLI == FFI 同机逐字节对账(决定性闸门)

- [x] 3.1 `crates/bead-ffi/dart/test/determinism_gate_test.dart`:保留现有两默认尺寸(16×20、30×24)、三项未设置的用例,确认对齐**不带旗标**的同机 `bead-cli generate`(四个具名文件按原始字节逐一相等)——闸门不回退。
- [x] 3.2 同文件新增「选项已设置」用例:以 `max_colors`(如 8) + `despeckle`(如 2)、默认 `staged` 路径调用桥接,与同机 `bead-cli generate --max-colors 8 --despeckle 2` 同输入逐字节比较四个具名文件(整数路径,host 同 libm 稳)。
- [x] 3.3 同用例解析该次返回的 `pattern_json`,断言其 `width`/`height`/`cells`/`stats`/`brand` 与 FRB 结构化字段逐一相等(禁止结构化数组自身再序列化比较)。
- [x] 3.4 `generator=gerstner` 只做「与同机 `bead-cli generate --generator gerstner` 对齐」的同机对账 + 结构不变量,不承诺跨目标 byte-exact(f32 路径,同机 canonical)。
- [x] 3.5 在同机跑通 `dart test`(dart/ 包)确认 3.1–3.4 全绿。

## 4. App 调用点适配(行为不变)

- [x] 4.1 `apps/mobile/lib/infrastructure/bead_bridge.dart` / `pattern_engine.dart`:随新签名更新调用,暂传 `maxColors: null` / `despeckle: null` / `generator: staged`,使 App 现有行为逐字节不变(设置屏控件属后续工作流,不在本变更)。
- [x] 4.2 `apps/mobile/test/pattern_engine_test.dart` 等受影响单测随签名更新并通过;`flutter analyze` 无新增告警。

## 5. 验收

- [x] 5.1 全量回归:`cargo test`(工作区)+ `bead-ffi` dart 包 `dart test` + `apps/mobile` `flutter test` 全绿。（cargo 工作区全过、bead-ffi dart 闸门 4 例全过;apps/mobile 两套件为既有 `@Skip`「需 iOS 原生库」故 skipped-非-failed，与本变更前一致、无回归）
- [x] 5.2 复核 ROADMAP「Post-M9 — Mobile UI Refinement」FFI 放宽验收点:三项可选、未设即默认逐字节不变、已设与 CLI 同旗标对齐;`filter`/`cell_size`/`shape`/`matcher` 仍不暴露;`bead-core` 零改动、无新依赖。
