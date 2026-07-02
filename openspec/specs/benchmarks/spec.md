# benchmarks 规范

## 目的
用 Criterion（`bead-core` 的 dev-dependency、`harness=false`）基准 `generate_pattern` 端到端执行时间，覆盖五个固定目标尺寸（40×40 / 80×100 / 100×100 / 150×150 / 300×300），输入在 bench 内合成、源图为目标 2× 以真测 Lanczos 重采样，且 `criterion` 仅入 `[dev-dependencies]`、不污染 runtime 依赖。基准不进 CI `test` 矩阵（避免 3× 成本与计时噪声），但经 clippy `--all-targets` 保持可编译、防 bit-rot，`cargo bench` 按需手动跑。内存追踪是显式 YAGNI 决策、推迟 Phase-2（届时并行 + 降色让分配行为值得追踪、`dhat-rs` 为预定工具），归档时在 INIT 标注以保源文档与交付一致；吞吐量由「每尺寸时间 + 尺寸」派生。
## 需求
### 需求:Criterion 基准覆盖五个固定尺寸的端到端时间
系统**必须**用 Criterion（`bead-core` 的 **dev-dependency**、`[[bench]] harness=false`）基准 `generate_pattern` **端到端**执行时间，覆盖五个**目标**尺寸 **40×40 / 80×100 / 100×100 / 150×150 / 300×300**（作一个 `benchmark_group` + 每尺寸 `BenchmarkId`）。基准输入**必须**在 bench 内**合成**（同 M6 `demo_png` 公式 `RgbImage::from_fn` → 编码 PNG 字节）、**源图尺寸须为目标的 2×**（每维加倍；源=目标会命中 `imageops::resize` 拷贝短路、漏测**默认滤镜（`Triangle`）的真实重采样**），**禁止** committed 大图夹具。`criterion` **禁止**进入 runtime 依赖——仅 `[dev-dependencies]`，`cargo tree`（非 dev）不得新增 crate。

#### 场景:五尺寸基准可跑且无 runtime 依赖污染
- **当** 运行 `cargo bench`
- **那么** 五个尺寸各产出 `generate_pattern` 端到端时间测量；`criterion` 仅在 `[dev-dependencies]`、非 dev 的 `cargo tree` 不含它

### 需求:基准不进 CI 矩阵但保持可编译
基准**禁止**加入 CI 的 `test` 矩阵（避免 3× 成本与计时噪声）；但 bench 目标**必须**保持可编译——由既有 clippy `--all-targets` 覆盖编译、防 bit-rot。
`cargo bench` 按需手动跑。

#### 场景:bench 编译被 CI 覆盖、计时不进矩阵
- **当** CI 跑 `cargo clippy --all-targets`
- **那么** `benches/` 被编译（防 bit-rot）；CI 的 `test` 矩阵**不**执行基准计时

### 需求:内存追踪推迟 Phase-2（显式决策）
M7 基准**必须**覆盖执行时间；内存 profiling **推迟** Phase-2（届时并行 + 降色让分配行为值得追踪、`dhat-rs` 为预定工具）。这是显式 YAGNI 决策，
**禁止**读作遗漏——Phase-1 单线程、内存 `O(width·height)` 无风险可追。**INIT/ARCHITECTURE 字面要求基准追踪「时间 + 内存」（ARCHITECTURE 另提 throughput）**：本推迟须在归档时于 INIT 标注、使源文档与交付一致；吞吐量由「每尺寸时间 + 尺寸」派生、无需单列。

#### 场景:M7 只交付时间基准、内存以备注推迟
- **当** 评估 M7 基准交付范围
- **那么** 执行时间经 Criterion 覆盖；内存追踪以 design 备注形式显式推迟 Phase-2、非静默略过

