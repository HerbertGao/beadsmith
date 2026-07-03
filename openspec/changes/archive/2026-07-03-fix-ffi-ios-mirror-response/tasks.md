## 1. mirror→DTO 实现(crates/bead-ffi)

- [x] 1.1 `api.rs`:删除 `#[frb(mirror(BeadPattern))] _BeadPattern` / `#[frb(mirror(ColorStat))] _ColorStat`,改为 bead-ffi 自有 DTO `pub struct BeadPattern { width, height, cells }` / `pub struct ColorStat { code, name, count }`,同名同字段,加 `#[derive(Debug, PartialEq)]`。
- [x] 1.2 `api.rs`:`pub use bead_core::{...}` 去掉 `BeadPattern` / `ColorStat`(保留 `GeneratorKind` —— 仅作参数、镜像不受影响)。
- [x] 1.3 `api.rs` `generate_inner`:从 `bead_core::GenerateResult` 的 `pattern` / `stats` 逐字段拷贝进 DTO(`stats` 用 `into_iter().map(...).collect()`)。
- [x] 1.4 更新受影响注释(模块头「为何 DTO 不用 mirror」、`GenerateOutput` doc)。

## 2. 重新生成 FRB 胶水

- [x] 2.1 `flutter_rust_bridge_codegen generate`(2.12.0)重生 `frb_generated.rs` / `dart/lib/src/api.dart`;`bead-core` 零改动。
- [x] 2.2 重建 iOS 模拟器静态库(`scripts/build-ios.sh aarch64-apple-ios-sim`),内容哈希与新胶水一致。

## 3. 回归测试

- [x] 3.1 新增 `apps/mobile/integration_test/generate_ios_regression_test.dart`:on-device 实调 `generate` 断言返回有效 pattern;**不带 `@TestOn('ios')`**(该选择器是老引擎测试从未运行、bug 漏网之因)。
- [x] 3.2 修 `api.rs` in-crate 测试:DTO-vs-DTO 比较靠 `PartialEq`;DTO-vs-`bead_core` 参考比较改逐字段 `assert_eq!`。

## 4. 验证(全部已通过)

- [x] 4.1 iOS 模拟器 headless 实跑 `generate`(9.7MB 大图)→ 返回有效 pattern,**不再 panic**。
- [x] 4.2 决定性闸门 `dart test`(crates/bead-ffi/dart)FFI==CLI 逐字节 **4/4 通过**。
- [x] 4.3 `cargo test -p bead-ffi --lib` **15/15**;`apps/mobile` `flutter analyze` 无问题。
- [x] 4.4 不变量核对:FRB 版本仍 `=2.12.0`;`bead-core` 依赖集与代码零改动;边界契约「结构化数组跨边界」保持;Dart 侧 App 代码不变(DTO 同名同字段)。
