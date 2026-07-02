# benchmarks 规范（增量）

## MODIFIED Requirements

### 需求:Criterion 基准覆盖五个固定尺寸的端到端时间
系统**必须**用 Criterion（`bead-core` 的 **dev-dependency**、`[[bench]] harness=false`）基准 `generate_pattern` **端到端**执行时间，覆盖五个**目标**尺寸 **40×40 / 80×100 / 100×100 / 150×150 / 300×300**（作一个 `benchmark_group` + 每尺寸 `BenchmarkId`）。基准输入**必须**在 bench 内**合成**（同 M6 `demo_png` 公式 `RgbImage::from_fn` → 编码 PNG 字节）、**源图尺寸须为目标的 2×**（每维加倍；源=目标会命中 `imageops::resize` 拷贝短路、漏测**默认滤镜（`Triangle`）的真实重采样**），**禁止** committed 大图夹具。`criterion` **禁止**进入 runtime 依赖——仅 `[dev-dependencies]`，`cargo tree`（非 dev）不得新增 crate。

#### 场景:五尺寸基准可跑且无 runtime 依赖污染
- **当** 运行 `cargo bench`
- **那么** 五个尺寸各产出 `generate_pattern` 端到端时间测量；`criterion` 仅在 `[dev-dependencies]`、非 dev 的 `cargo tree` 不含它
