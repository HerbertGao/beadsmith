## 1. Quantizer 模块（bead-core/src/quantizer）

- [x] 1.1 新建 `crates/bead-core/src/quantizer/mod.rs`：`pub trait Quantizer { fn quantize(&self, grid: &PixelGrid) -> PixelGrid; }`（object-safe，复刻 `matcher::ColorMatcher` 缝法）。
- [x] 1.2 `pub struct MedianCutQuantizer` + `pub fn new(max_colors: u32) -> Result<MedianCutQuantizer, BeadError>`：校验 `max_colors >= 1`，`==0` → `Err(BeadError::InvalidImage { reason })`（reason 含 "max_colors"，复用零维度同变体、**不新增变体**）；配置存快照、值语义。
- [x] 1.3 `impl Quantizer for MedianCutQuantizer`：RGB Median Cut，规则按 design D2 全钉死——**step 0 short-circuit**：统计网格不同色数 `d`，`d <= max_colors`（含空网格 `d==0`）→ **原样返回输入**（保证 no-op + 空网格不除零）；否则选最大单通道展布的桶+通道（平局桶下标小者→R<G<B）、按严格全序键 `(选定通道值,R,G,B,原始行优先下标)` 排序（末键唯一→真·全序、与排序稳定性无关）、中位 `len/2` 切（**下半原位替换桶 `i`、上半插 `i+1`**）、桶代表色 = 分量均值 **`sum:u64 / count`** 整数截断、每像素映射到桶代表色。全整数、无 `f32`/`sqrt`、无随机/rayon；全函数不 panic。
- [x] 1.4 `crates/bead-core/src/lib.rs`：从 crate 根**重导出** `Quantizer` / `MedianCutQuantizer`；更新 `InvalidImage` 文档注释（补 `max_colors==0` 场景，现仅列图像维度）。

## 2. 单元测试（quantizer 模块内）

- [x] 2.1 固定小 `PixelGrid` + 固定 `max_colors` → `pixels` 等于**硬编码期望网格**；重复调用一致（跨架构位精确，整数无浮点）。
- [x] 2.2 `MedianCutQuantizer::new(0)` → `Err(InvalidImage)`（reason 含 "max_colors"），`new(>=1)` → `Ok`，均不 panic。
- [x] 2.3 `max_colors >= 不同色数`（含远超）→ 输出与输入**逐像素相同**（short-circuit no-op）。**含偏态分布反例** `A×8,B×1` @ `max_colors=4`（k=2）：必须仍逐像素相同（验证 short-circuit 而非涌现行为——无 short-circuit 时此例会改色）。
- [x] 2.4 `1 <= max_colors < 不同色数` → 输出**不同色数 ≤ max_colors**（上限语义）。
- [x] 2.5 `max_colors == 1` → 单桶、全图均值色，合法不报错。**空网格**（`w==0` 或 `h==0`，`pixels` 空）→ 原样返回、**不 panic**（不计算 `sum/count`）。

## 3. pipeline 接入（bead-core/src/pipeline）

- [x] 3.1 `GenerateOptions` 加 `max_colors: Option<u32>`，`derive(Default)` 产 `None`；更新结构体文档。
- [x] 3.2 `generate_pattern`：`image_to_grid` 后插可选阶段——`opts.max_colors` 为 `Some(n)` 时 `MedianCutQuantizer::new(n)?.quantize(&grid)`（`?` 透传 `Some(0)` 的 Err）、`None` 时 grid 原样；再进 `match_pattern`。其余链不动。
- [x] 3.3 更新 `generate_pattern` 文档注释（复述链时加可选量化阶段，与 pipeline 规范一致）。
- [x] 3.4 跑 `cargo test -p bead-core`：既有管线测试（默认 `max_colors=None`）应**全绿不变**（忠实串联 / 单一 Palette / 错误透传 / 确定性）。
- [x] 3.5 新增管线测试：`max_colors=None` 与不设时 `GenerateResult` 逐字段相等（默认路径不变）；`Some(0)` → `generate_pattern` 返 `Err` 不 panic；`Some(n)` 小于全色时 stats 的不同色数 ≤ n。

## 4. CLI 接入（bead-cli/src/main.rs）

- [x] 4.1 `generate` 加可选 `--max-colors <N>`（clap `Option<u32>`），传入 `GenerateOptions.max_colors`；help 文案提示常见档位 24/36/48/72。
- [x] 4.2 跑 `cargo test -p bead-cli`：既有 CLI 测试（不给 `--max-colors`）应**全绿不变**。
- [x] 4.3 新增/扩展 CLI 测试：给 `--max-colors N` 退出码 0、写出四文件、`summary.txt` 的色数 ≤ N；`--max-colors 0` 非零退出 + stderr 带语境、不 panic。

## 5. Golden（默认不变；区分量化器单元 vs 端到端）

- [x] 5.1 确认默认 golden（无 max_colors）**逐字节不变、无需重 bless**（默认路径恒等跳过量化）。
- [x] 5.2 （可选）**量化器单元** golden（grid→grid，纯整数）→ 可在**任意平台**字节断言（不限 canonical）。**注意**：这是 §2.1 那种 grid→grid 单元用例，**不是** `tests/golden/` 的端到端四产物——端到端 `--max-colors` 路径仍经 Lanczos3 + LabMatcher 浮点，**与默认 golden 一样 canonical-only**；若往 `tests/golden/` 加端到端 max_colors 用例，须 canonical-only 且补 `golden-tests` 规范的固定设置 delta。

## 6. 全量验证

- [x] 6.1 `cargo build && cargo test --workspace --all-features` 全绿；`cargo clippy --workspace --all-targets` 0 警告；`cargo fmt --all -- --check` 通过。
- [x] 6.2 确认 FFI 边界签名**零改动**（`generate(bytes,palette,w,h)` 不变），flutter-ffi 规范不动；M8 「CLI==FFI 同机逐字节」测试仍通过。
- [x] 6.3 `cargo run -p bead-cli -- generate … --max-colors 24` 实跑一张彩色样图，对比无 `--max-colors` 版，肉眼确认色数明显减少而主体可辨。
- [x] 6.4 文档：`ARCHITECTURE.md:138-149` quantizer 段标注 Median Cut 已实现（`MedianCutQuantizer`），体例同 matcher 段 `LabMatcher`「— implemented as the default」；abbreviated `fn quantize(...)` 签名可留。
