## 为什么

里程碑 **M7 — Golden Tests**。M1–M6 已把引擎各原语 + pipeline + CLI 建齐、端到端可跑，但**没有任何东西冻结它的输出字节**：一次重构、一次 `image`/`png` 依赖升级、或某个原语的细微改动，都可能**静默改变** `pattern.json` / `summary.txt` / 两张 PNG 而无人察觉。M7 用 golden 测试 + Criterion 基准把 Phase-1 引擎**钉死并可信**——「故意改算法 → 响亮失败」，这是 ROADMAP 对 M7 的 done-when，也是 M8「CLI == FFI」与 M7 之后冻结 Phase-1 的前提（**规则 2**：确定性是硬门，金标准是它的可执行证明）。

## 变更内容

- **新增 golden 测试**（`golden-tests` 能力）：对固定输入（`samples/gradient.png` @ 16×20、`Lanczos3`、`cell_size 10`、`artkal_s`）冻结**四个产物字节**——`pattern.json` + `summary.txt` + `preview.png` + `grid.png`，committed 到仓根 `tests/golden/`。
  - **走库 API**：测试在 `crates/bead-cli/tests/golden.rs` 调 `generate_pattern` + `pattern_json`、比对 `result.{summary,preview_png,grid_png}` 与 committed 字节（CLI 经 `main.rs` 的四个 `fs::write` 原样写这四块字节、结构上无变换——可由 `main.rs` 直接核验，故库级 golden 传递性覆盖 CLI 契约，规则 5；`bead-core` 保持无 fs，测试放 `bead-cli`）。
  - **跨平台策略 = Ubuntu 为基准**：用**真实默认 `Lanczos3`**（即 CLI 默认、用户真拿到的路径），但**字节金标准只在 canonical（**arm64 Linux**、gate `cfg!(target_os="linux") && cfg!(target_arch="aarch64")`、CI 参考 ubuntu-24.04-arm）断言**（回归探针；arm64 = 移动端 iOS/Android 同架构族，Apple Silicon 开发机可用原生 arm64 容器 bless）；其余平台（x86-64 Linux/macos/windows）跑同一测试，断言**浮点无关的结构不变量**（PNG 可解码、`preview` 160×200、`pattern.json` 键序/计数/索引合法、`summary.txt` 结构、`grid.png` 背景与粗线常量像素），**不**跨平台比对走浮点 resize 的 cell 字节（单进程「重跑相同」对纯函数近乎空断言，故改断结构不变量）。原因：`image 0.25.10` 的 `Lanczos3` 虽是纯标量 f32 累加（`t += vec*w`、无 `mul_add`/SIMD/FMA——FMA 仅在 blur 的 `filter_1d` 路径且 `target_feature` 门控、resize 够不到）、`default-features=false` 已关 rayon（且 resize 本就单线程、与 rayon 开关无关）、解码 + `to_rgb8`（RGB8 源）是纯整数、u8 经 `clamp`+`round`——**但** Lanczos 权重核 `lanczos3_kernel → sinc → f32::sin`（`sample.rs:149-156`）调 `f32::sin`，其精度未被规定跨平台/跨 libm（glibc/Apple/MSVCRT）逐位一致，故跨平台逐字节相同**无法证明、亦非 rule 2 所要求**。M2 在 `matcher`/`image`/`renderer` 注释里把硬编码跨架构金标准限制在纯整数输出、对 Lanczos3 f32 路径保守，是**正确**的，本 change 不推翻它。
  - **🚩 `.gitattributes`**（必加）：`tests/golden/*.json -text`、`tests/golden/*.txt -text`、`tests/golden/*.png binary`（gitattributes 用 gitignore 式模式、**不支持** `{json,txt}` 花括号展开，须分写）——保证 committed 文本金标准在任何平台 checkout/commit 都保持 `LF`（引擎只写 `\n`），堵 Windows `autocrlf` 把它改成 CRLF 而污染仓库金标准、令 ubuntu 字节断言失效。
  - **fail-loudly + 重生**：golden 不匹配时打印有用 diff（文本逐行、PNG 提示 + 写 `<name>.actual` 旁置）；`BLESS=1 cargo test` **重写** golden → `git diff tests/golden/` 强制评审，使「故意改算法」是一次刻意可审动作。
  - **dep bump 守卫**：CI 已跑 `cargo test --locked`，`Cargo.lock` 入库 → 依赖升级是可审 diff；golden 把它升级为**行为级 tripwire**——改了输出字节的 bump 在 golden 红、必须刻意 bless（承接 M5-D3「dep bump 响亮失败」）。
- **新增 Criterion 基准**（`benchmarks` 能力）：`crates/bead-core/benches/bench.rs`（`criterion` **dev-dep**、`harness=false`），5 个**目标**尺寸（40×40 / 80×100 / 100×100 / 150×150 / 300×300）**在 bench 内合成输入**（同 M6 `demo_png` 公式、不 committed 大图），测 `generate_pattern` 端到端。**合成源图须为目标的 2×**（每维加倍），否则 `imageops::resize` 命中「源=目标」拷贝短路（`sample.rs:984`）、不跑 Lanczos 重采样、基准测不到 resize 成本。**不进 CI 矩阵**（lint job 的 clippy `--all-targets` 在 ubuntu 已编译 benches、防 bit-rot）。
- **内存追踪推迟 Phase-2**：Criterion 只测时间；Phase-1 单线程、内存 `O(width·height)` 无风险，`dhat` 等留到 Phase-2（rayon/降色让分配行为值得追踪时）再上。**INIT/ARCHITECTURE 字面要求基准追踪「时间 + 内存」**：本 change 显式把内存推迟 Phase-2（非遗漏），归档时在 INIT 标注此推迟、使源文档与交付一致；吞吐量由「每尺寸时间 + 尺寸」即可派生、无需单列。

## 功能 (Capabilities)

### 新增功能
- `golden-tests`: Phase-1 引擎的**输出冻结契约**——固定输入下 `pattern.json`/`summary.txt`/`preview.png`/`grid.png` 的字节金标准（库 API 产出、**arm64-Linux-canonical 字节断言 + 其余平台（x86-64 Linux/macos/windows）结构不变量断言**、`.gitattributes` 文本金标准 LF 稳定、`BLESS` 重生〔仅 canonical〕、`--locked` + golden 的 dep-bump 守卫）；故意改算法或改字节的依赖升级在 canonical 平台**响亮失败**。
- `benchmarks`: Criterion 性能基准面——`generate_pattern` 端到端在 5 个固定尺寸的执行时间基线（输入 bench 内合成、不进 CI 矩阵）；内存追踪显式推迟 Phase-2。

### 修改功能
（无——golden 测试**验证**既有引擎的确定性，不改任何已生效需求；不触 pipeline/renderer/statistics 等规范。）

## 影响

- **代码**：
  - `tests/golden/`（仓根，新）——`pattern.json`/`summary.txt`/`preview.png`/`grid.png` 四个金标准 + `README.md`（记设置 + 重生命令）。
  - `crates/bead-cli/tests/golden.rs`（新）——库级 golden 测试 + `BLESS=1` 重生分支，经 `CARGO_MANIFEST_DIR/../..` 读仓根。
  - `crates/bead-core/benches/bench.rs`（新）+ `crates/bead-core/Cargo.toml`（改，加 `[dev-dependencies] criterion` + `[[bench]]`）。
  - `.gitattributes`（仓根，新）。
- **依赖**：`criterion` 作 `bead-core` **dev-dependency**（非 runtime；INIT 技术栈已列）；**无新 runtime 依赖**。
- **确定性**（规则 2，硬门）：golden 把「同输入逐字节相同」从文档承诺变成 CI 可执行门——本 change 范围内把规则 2 澄清为**同平台/同 `image` 版本**逐字节相同（ARCHITECTURE 字面义，从未承诺跨架构 libm 一致）；ubuntu 跑字节金标准、macos/windows 跑浮点无关结构不变量，float resize 路径的跨平台逐字节一致**不保证、不作门**。不改 CLAUDE.md 硬规则本身。
- **CI**：`ci.yml` 矩阵**加 `ubuntu-24.04-arm`**（canonical 字节平台；x86-64 ubuntu/macos/windows 跑结构断言）——`cargo test --locked --workspace --all-features` 自动拾取新测试目标；benches 由 lint job（ubuntu）的 clippy `--all-targets` 覆盖编译。
- **里程碑 / Phase**：里程碑 M7；Phase 1 收尾（引擎冻结、可信）。benches 是可裁的那半，golden 是 Phase-1-freeze 核心。
- **文档**：与 ROADMAP（M7）/ INIT（Golden Tests + Benchmark Tests）一致；INIT golden 清单是 `preview.png`+`pattern.json`+`summary.txt`，本 change **额外冻 `grid.png`**（INIT 清单是示例非禁止、M6 已标其 golden 状态「M7 决」，此处决定冻它——每个 CLI 写出的字节都进 golden）；归档时在 INIT 标注 `grid.png` 已纳入 M7 golden，避免 INIT 清单与冻结集静默分歧。
