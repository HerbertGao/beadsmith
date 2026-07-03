## 上下文

`generate` 在 iOS 上返回时 100% panic:`PanicException(assertion left == right)`,位置
`flutter_rust_bridge-2.12.0/src/codec/sse.rs:129` = `SseDeserializer::end()` 的
`assert_eq!(data_len, cursor.position())`,恒定差 **+6 字节**。被 `GeneratePage` 的 try/catch
接住显示为错误文案。

**二分诊断(模拟器 headless 集成测试,不需相册操作)**:
- 与**图片大小无关**:9KB 图 `left:26751/right:26745`、13MB 图 `left:13397116/right:13397110`,恒定 +6。
- 与**调用参数无关**:`probe7`(与 `generate` 完全相同的 7 参、含 widen-ffi 的 `max_colors`/`despeckle`/
  `generator`)、`probe_full`(同 7 参 + 426KB 大图)返回普通类型时**均正常**。
- **触发点 = 响应类型**:`generate` 返回 `GenerateOutput`(内含 `#[frb(mirror)]` 的 `BeadPattern`、
  `Vec<ColorStat>`)时崩;返回普通结构体(`Vec<u8>`/`Vec<u16>`/`String` 字段)一律正常。
- **仅 iOS 目标**:同一 dylib + 同一字节在 **macOS 上无论 CLI 直调还是 FRB 桥(dart test / flutter
  host runtime)都正常**;只有 `aarch64-apple-ios-sim` 编译的库崩。故属 iOS 目标下 FRB SSE 编解码
  对镜像返回类型的 codegen/ABI 级 bug。
- 早在 **M8** 就潜伏(`GenerateOutput`/镜像自 M8 未变);M9 iOS 验收的 `engine_on_ios_test.dart` 带
  `@TestOn('ios')`,该选择器使其**静默从未运行**,故 `generate`-返回-`GenerateOutput` 在 iOS 上从未真跑过。

## 目标 / 非目标

**目标:** 让 `generate` 在 iOS 上返回有效 `GenerateOutput` 不 panic;保留钉死的 FRB `=2.12.0`;
保持「结构化数组跨边界」「CLI == FFI 逐字节相等」契约与 `bead-core` 零改动;补一个**真正会运行**的
iOS 回归测试守住该保证。

**非目标:** 升级/更换 FRB;切 DCO 编解码;改 `generate` 对外签名或边界契约;改 `bead-core`;
动裁剪器(`upgrade-crop-editor`);给 picker 加尺寸上限(与本 bug 无关)。

## 决策

**决策 1:响应结构类型改用 bead-ffi 自有 DTO,不用 `#[frb(mirror)]`。**
`GenerateOutput.pattern` / `stats` 由镜像 `BeadPattern` / `ColorStat` 改为 bead-ffi 自有的同名同字段
普通 FRB struct;`generate_inner` 从 `bead_core::GenerateResult` 逐字段拷贝(`width`/`height`/`cells`、
`code`/`name`/`count`)。二分已证明「普通 struct + 完整大请求」在 iOS 上稳过,故这是对症、最小、
且**不动 FRB 版本**的修复。
- 替代 A:升级 FRB 到 2.13.0-beta.x。否决:beta 风险 + 动 M8 刻意钉死的版本,爆炸半径大,且无
  changelog 证据确认已修。
- 替代 B:切 DCO 编解码。否决:改动面更大、需全量回归,而 DTO 已足够对症且保留 SSE 默认路径。
- 替代 C:给 picker 封顶源图。否决:恒定 +6 与大小无关,封顶无效(已实测 400px 仍崩)。

**决策 2:`GeneratorKind` 保持 `#[frb(mirror)]`。**
它只作**参数**跨桥,`probe7` 已证明带该镜像参数正常。仅**响应**里的镜像触发 panic,故无需动它,
也避免无谓改动。

**决策 3:DTO 派生 `Debug` + `PartialEq`。**
`GenerateOutput` 派生 `Debug`(要求字段 `Debug`);in-crate 测试对两个 `GenerateOutput` 做 `assert_ne!`
(DTO vs DTO)需 `PartialEq`。跨类型比较(DTO vs `bead_core` 参考)改为逐字段 `assert_eq!`。

**决策 4:回归测试去掉 `@TestOn('ios')`,on-device 实调 `generate`。**
`@TestOn('ios')` 使老引擎测试静默从未运行——正是 bug 漏网之因。新测试合成一张图、在 booted 设备上实调
`generate` 断言返回有效 pattern;不带平台选择器(panic 是 iOS 特有,但断言在他处无害),确保它**真的跑**。

## 风险 / 权衡

- **根因未在 FRB 上游修复**:本变更绕开(响应不放镜像),未修 FRB 的 iOS SSE bug 本身。缓解:回归测试
  钉住「响应无镜像」,若将来有人把镜像重新放进返回值会被测试抓到;并在 spec 记明约束与原因。
- **DTO 与 core 的字段漂移**:DTO 手抄 `bead_core` 字段,core 若加字段需同步。缓解:字段少且稳定;
  「CLI == FFI 逐字节」闸门会在产物层抓到不一致。
- **测试对设备的依赖**:回归测试需 booted 设备(与既有 `integration_test` 同性质);CI 是否跑 integration
  另议,不在本变更范围。
