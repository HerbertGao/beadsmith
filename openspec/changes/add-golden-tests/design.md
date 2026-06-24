## 上下文

里程碑 M7。M1–M6 已建齐引擎并端到端可跑（`pipeline::generate_pattern` 串联 image→match→stats→render，`bead-cli generate` 写四文件）。现状缺口：**没有任何东西冻结输出字节**——确定性（规则 2）目前只是文档承诺 + 单测里的「同一次运行重算相等」，没有 committed 金标准来抓「跨提交/跨依赖版本的静默漂移」。`Cargo.lock` 已 git 跟踪，CI `test` job 跑 `cargo test --locked --workspace --all-features` 于 **ubuntu/macos/windows 三平台**矩阵，clippy `--all-targets` 编译 bench 目标。`samples/gradient.png`（32×40、像素 `r=x%256,g=y%256,b=(x+y)%256`）是 M6 交付的固定输入。设计经探索 + Software Architect 实读 `image` 源码核实 + 主 agent QA。

## 目标 / 非目标

**目标：** `golden-tests` 能力（库级 golden 测试，冻结 `pattern.json`/`summary.txt`/`preview.png`/`grid.png` 四个产物字节，arm64-Linux-canonical 字节断言 + 其余平台结构不变量断言，`BLESS` 重生〔仅 canonical〕，`.gitattributes` 文本金标准 LF 稳定，`--locked`+golden 的 dep-bump 守卫）；`benchmarks` 能力（Criterion 5 尺寸时间基线）；Phase-1 引擎冻结、可信。

**非目标：** 内存追踪（推迟 Phase-2，D9）；新 runtime 依赖（criterion 仅 dev-dep）；改动 CI workflow（自动拾取，D2/D8）；CLI 加 flag（用既有默认设置，D4）；第二个 golden 夹具（YAGNI，D4）；改任何既有引擎需求（golden 只验证、不改 pipeline/renderer/statistics 规范）。

## 决策

**D1 — 跨平台策略 = arm64-Linux-canonical（字节金标准仅 arm64 Linux；x86-64 Linux/macos/windows 断言浮点无关结构不变量）。推翻原 A+「三平台字节断言」。**

- **结论先行**：byte-golden **仅在 canonical 平台** 断言；其余平台（x86-64 Linux/macos/windows）跑同一测试、只断言**浮点无关的结构不变量**（见下）。`Lanczos3` resize 路径的跨平台/跨架构**字节一致**显式**不保证、也不要求**。canonical = **arm64 Linux**（gate=`cfg!(target_os="linux") && cfg!(target_arch="aarch64")`）；CI 参考 runner = **ubuntu-24.04-arm**（GitHub 已 GA 的 arm64 Linux runner），committed 字节即其 glibc 下参考值。**选 arm64 的理由**：引擎生产目标是移动端 arm64（iOS/Android）的同架构族，且 Apple Silicon 开发机可用**原生** arm64 容器 bless（无 QEMU 模拟）；x86-64 Linux 因架构不同即非 canonical、走结构断言。残余：不同 glibc 版本的 arm64 Linux 理论上可有 ULP 级差异（同 `sinf` 残余类），故 bless 以 CI ubuntu-24.04-arm 为准、其它 arm64-Linux 字节不符按环境处理。**注**：golden 是回归探针、本就无法匹配生产——iOS（Apple libm）与 Android（Bionic libm）虽同为 arm64、`sinf` 仍可不同，故无单一金标准能同时匹配两者，此处 canonical 只为「稳定可复现 + 易 bless」。
- **为何推翻原 A+（实读 `image 0.25.10`）**：原 A+ 论据**逐条为真、唯独漏了权重 kernel 里的 `sin`**——
  - `imageops::resize`（`src/imageops/sample.rs`）只做 `vertical_sample` + `horizontal_sample`，内层累加是**纯标量** `t.0 += vec.0 * w`（无 `mul_add`、无 SIMD、无 intrinsics）。
  - 全 crate 唯一的 FMA（`src/math/utils.rs` `multiply_accumulate`）双重 gated：`#[cfg(target_feature="fma"|"neon")]` **且**只被 `filter_1d`（gaussian_blur 路径）调用，`resize` 够不到；本仓无 `.cargo/config`/`RUSTFLAGS`/`target-feature`，stable 默认 baseline x86-64 无 fma。
  - `default-features=false, features=["png","jpeg","webp"]` **关掉了 `rayon`**（在 image 的 default feature 里）；且 `resize` 本就单线程、与 rayon 开关无关（rayon 只加可选 `par_*` helper、resize 不用）→ 无并行归约序非确定。
  - u8 转换经 `clamp` → `round`（IEEE-754 spec'd、平台无关）。
  - **❌ 漏点（推翻原结论）**：Lanczos 权重核 `lanczos3_kernel → sinc → f32::sin`（`sample.rs:149-156`，`a.sin() / a`）调 `f32::sin`，其精度 IEEE-754 **未规定**跨 libm（glibc/Apple/MSVCRT）逐位一致——这是唯一破坏字节可移植的运算。「相同标量运算、相同序」当某子运算（`sinf`）本身平台相关时，**不**蕴含「相同结果」。
  - **故 M2 是对的**：`matcher/mod.rs:319`、`image/mod.rs:300-302`、`renderer/mod.rs:77` 把硬编码跨架构金标准限制在纯整数输出、对 Lanczos3 f32 路径保守——本 change **不推翻**它，是源码读证实了它。
- **H2（实测，支撑 arm64-Linux-canonical）**：「即便像素 ±1，matcher 量化吃掉、cells 仍稳」**不成立**——artkal_s 199 色太密，全 320 像素 min 距离 gap=1、个个压边界。即任一 1-LSB resize 漂移即翻 cell → 四产物全变；matcher **零鲁棒**。这正是不能赌跨平台字节一致的理由（`sinf` 的 1-ULP 差就够翻），同时是 golden 在 canonical 平台**极敏感**的好处（抓回归一抓一个准）。
- **非 canonical 平台（macos/windows）断言什么**（浮点无关、真能抓回归；单进程「重跑相同」对纯函数近乎空断言、故不用它）：
  - **关键事实**：四产物**无一**逃得过浮点路径——全部下游于 resized grid → matched cells，任一 cell 翻则字节变；故**没有**任何 committed 文件可跨平台字节断言（否决「只断平台不变产物」方案）。
  - 改断**浮点无关结构不变量**：① 两 PNG 经 `bead_core::decode_image` 可解码、`preview` 160×200、`grid` 尺寸=几何公式值；② `pattern.json` 键序 `[brand,width,height,cells,total,stats]`（对 `pattern_json(&result)` 串做字节位置单调性检查）、`result.pattern.{width=16,height=20,cells.len=320}`、`total_beads=320`、各 cell 索引 `< palette.colors.len()`（=199、用 `.len()`、勿写死字面量）（**直接在 `GenerateResult` 结构上断言**——规则 3，pattern 是真相源、比重解析序列化更诚实）；③ `summary.txt`（`result.summary` 串）结构：首行 `Bead Pattern Summary`、`Size: 16 x 20`、`Total Beads: 320`、`Palette: Artkal S`、空行分隔、各色行 `<code> <name>: <count>`；**仅对空行后 body 求和**==320（header `Total Beads: 320` 也匹配 `rsplit_once(": ")`、对整串求和会 double-count 成 640）；④ `grid.png` 解码后 `(0,0)`=BG `[255,255,255]`、`(118,19)`=BOLD `[120,120,120]`（几何 margin 18/14、cell 10：bx=10 竖粗线 x=118、y=19 避标号/横边界；坐标须落 STEP 边界、避 bead 内部，纯整数几何、跨平台不变）。**cell 颜色值/具体色行内容**走浮点 → **不**断。
  - **零新依赖**：以上全用 `GenerateResult` 结构 + `bead_core::decode_image`（已 re-export）+ std 字符串操作完成，**不**引 `serde_json`/`image` 进 `bead-cli`（golden.rs 真零新依赖）。
- **理由**：golden 本职=「锁住用户真拿到的输出、故意改算法即响亮失败」。canonical(arm64 Linux) 字节冻结满足 ROADMAP M7 done-when（done-when 未指定平台、单 canonical tripwire 足够、且抓到 cell-值回归）；非 canonical 的结构断言**加**纵深——几何/编码/序列化/格式/计数回归在所有平台都响亮失败，唯「仅 arm64-Linux 的 resize cell-值漂移」只由 canonical 抓（而那类回归近乎不可能）。仍用**真实默认 `Lanczos3`**、不改 `Nearest`（否则漏真 resize 路径回归覆盖）。
- **否决的替代**：① **A+ 三平台字节**——`sinf` 证伪、会在错误平台误红；② **per-platform 三套金标准**——3× 金标准 + 3× bless、CI runner 镜像变更即静默作废某平台金标准，为近乎不可能的回归类买单；③ **只断平台不变产物**——无此产物（见上）；④ **Nearest / 预缩放夹具**——测非默认路径、漏 Lanczos3 覆盖。

**D2 — golden 走**库 API**、测试放 `crates/bead-cli/tests/golden.rs`（不放 bead-core）。**

- 测试调 `bead_core::generate_pattern(bytes, &palette, &opts)` + `pattern_json(&result)`，比对 `result.summary`/`result.preview_png`/`result.grid_png` 与 committed 字节。
- **传递覆盖 CLI 契约（规则 5）**：`main.rs` 经 `fs::write` **原样**写 `&result.preview_png`/`&result.grid_png`/`pattern_json(&result)`/`&result.summary`（结构上无变换、可由 `main.rs` 直接核验——非「M6 测试已证」）→ 库级 golden 钉住库输出即钉住 CLI 写出。免 subprocess/路径/shell 坑、失败直指引擎。
- **放 bead-cli 而非 bead-core**：golden 测试读文件，`bead-core` 须无 fs（规则 1）——把读盘测试塞进 core crate 浑浊「core 不碰 fs」叙事、招未来漂移；`bead-cli` 已有 fs 集成测试（`cli.rs`）+ `CARGO_MANIFEST_DIR/../..`→仓根惯例（M6 用过）。`golden.rs` 经 `Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden")` 读。
- **理由**：最小 + 诚实 + 复用既有惯例。**代价（自觉）**：golden 不在「bead-core 单独 checkout」下跑——可接受（仓是 workspace、CI 跑 `--workspace`）。

**D3 — 冻结**全四个**产物（`pattern.json` + `summary.txt` + `preview.png` + `grid.png`）。**

- `pattern.json` + `summary.txt`：INIT 强制；虽为整数派生（cells=palette 索引、counts），但其**值**经 resize 走浮点 → 仍**逐平台相关**（非跨架构稳），故 canonical 冻字节、非 canonical 断结构不变量。
- `preview.png`（INIT 标 optional）：**纳入**——`BeadPattern`+palette → PNG、编码参数钉死（`CompressionType::Fast`/`FilterType::Adaptive`，M5-D3）、M5/M6 已证同运行字节稳；冻它把渲染器 + PNG 编码版本一并锁（`png`/`image` bump 改字节即红）。
- `grid.png`（INIT 清单外）：**也冻**——同样字节确定、且是更复杂的渲染器（坐标 + 网格线），最值得钉；INIT 清单是示例非禁止，M6 已标其 golden 状态「M7 决」，此处决定冻。冻全四 = 每个 CLI 写出的字节都进 golden（最强「引擎冻结」）。
- **理由**：四产物在 canonical 平台均确定（D1），ubuntu 逐字节冻全四 = 最强引擎冻结；跨平台 cell 值不保证、由非 canonical 的结构不变量覆盖。

**D4 — 夹具复用 `samples/gradient.png`，固定设置 16×20 / `Lanczos3` / `cell_size 10` / `artkal_s`。**

- 复用既有 committed 32×40 夹具（不新增）；16:20==4:5==32:40 → **crop 是 no-op**（裁切偏移不成变量），且**与 M6 `cli.rs` e2e 同设置** → golden 与既有 e2e 测试构造上一致。
- 设置**钉在测试代码里、非 CLI flag**（不扩 CLI 面）。
- **替代：加更大/更花夹具**：否决——单夹具足够（gradient 已宽色谱、可人审）；第二夹具 YAGNI 到 Phase-2 算法需要更难用例时再加。

**D5 — 必加 `.gitattributes`（仓根）：`tests/golden/*.json -text`、`tests/golden/*.txt -text`、`tests/golden/*.png binary`（gitattributes 用 gitignore 式模式、不支持 `{json,txt}` 花括号展开、须分写）。**

- **问题**：当前无 `.gitattributes`。git `autocrlf`（尤其 Windows 贡献者 checkout/commit）会把 committed 文本 golden 变 CRLF，而引擎只写 `\n`（`statistics/mod.rs`）→ golden 字节带 `\r`、令仓库文本金标准被污染、canonical(arm64 Linux) 字节断言失效。
- `-text` = 永不规范化行尾；`binary`（=`-text -diff`）= PNG 不规范化、不当文本 diff。
- **理由**：这一个文件保证 committed 文本金标准跨平台 checkout/commit 恒为 `LF`，是 canonical(arm64 Linux) 字节断言长期有效的前提。

**D6 — fail-loudly：有用 diff + `BLESS=1` 重生。**

- 不可读的 `assert_eq!(bytes)`（320 格 JSON diff 没法看）→ 统一**字节比对**、失败时写 `tests/golden/<name>.actual` 旁置并提示「比对 `<name>` vs `<name>.actual`」（文本/PNG 皆一条 `diff`/目视命令可达；panic 消息对 UTF-8 可解码输入可附首个差异行）。由纯 `compare_or_panic(name, expected, actual)` 承载；`assert_golden(name, actual)` 在其上包 BLESS 重生 + canonical-gate + 读盘（见 D10.6 / tasks §2.3）。
- **重生**：`BLESS=1 cargo test -p bead-cli --test golden` **写**当前输出覆盖 golden（手写 ~10 行 env-gate、无新依赖，同 `insta` bless 模式）→ 重生文件进 `git diff` 强制评审。`tests/golden/README.md` 记确切命令 + 设置。
- **理由**：使 ROADMAP done-when「故意改算法 → 响亮失败」具体可操作：改算法 → `cargo test` 红 + 清晰消息 → `BLESS=1` → `git diff tests/golden/` 精确显示改了啥 → 评审通过/驳回。静默漂移不可能。

**D7 — dep-bump 守卫 = `Cargo.lock --locked` + golden（承接 M5-D3）。**

- `Cargo.lock` 入库、CI 跑 `--locked` → 解析版本冻结、升级是可审 diff。golden 把它升级为**行为级 tripwire**：未来 `image`/`png` bump 若改 resize 输出或 PNG 编码字节，golden 在 bump PR 红——响亮、刻意、阻塞，而非静默漂移。design/README **明记**：golden + `--locked` 共同构成 dep-bump 守卫，改字节的 bump 必须刻意 bless。

**D8 — Criterion 基准：dev-dep、5 尺寸、in-bench 合成输入、测端到端、不进 CI 矩阵。**

- `criterion`（`version="0.8", default-features=false`，关 html/cargo-criterion 保精简）作 `bead-core` **dev-dependency**（非 runtime；INIT 技术栈已列）；`crates/bead-core/benches/bench.rs` + `[[bench]] name="bench" harness=false`。
- **输入 bench 内合成**（同 M6 `demo_png` 公式 `RgbImage::from_fn` → 编码 PNG 字节，每尺寸 setup 一次），**合成源图须为目标的 2×**（每维加倍）——否则 `imageops::resize` 命中「源=目标」拷贝短路（`sample.rs:984`）、跳过 Lanczos 重采样、基准漏测 resize 成本；**不 committed 大图**；palette 经 `include_bytes!` 编译期内嵌（bead-core 运行时不碰 fs，rule 1/D2）。
- 5 尺寸（40×40 / 80×100 / 100×100 / 150×150 / 300×300）作一个 `benchmark_group` + 每尺寸 `BenchmarkId`（一组五输入、干净对比表）；测**库入口 `generate_pattern` 端到端**（M8/FFI 与 CLI 真调的东西）。可选第二组拆 stage（`image_to_grid`/`match_pattern`/`render_*`）给 Phase-2 优化留基线——nice-to-have、非必需。
- **不进 CI 矩阵**（3× 成本 + 噪声）；`cargo bench` 按需跑。lint job（ubuntu）的 clippy `--all-targets` 已编译 `benches/` → **编译防 bit-rot 免费覆盖**（仅 ubuntu 编译、足够）。

**D9 — 内存追踪推迟 Phase-2（显式决策、非遗漏）。**

- INIT/ARCHITECTURE 提「track execution time and memory」，但 Criterion **只测时间**。Phase-1 单线程、分配简单，峰值内存 `O(width·height)`（300×300 = 90k 格 × 数字节 + 两 PNG buffer）、无内存**风险**可追。上 `dhat`（dev-dep + feature flag + 独立 harness）或 peak-RSS 抓取是为没人会据以行动的数（直到 Phase-2 rayon/降色）付真复杂度。
- **决策**：M7 基准只覆盖执行时间；内存 profiling 推迟 Phase-2（并行 + 降色让分配行为值得追踪时，`dhat-rs` 为预定工具）。YAGNI-正确、保 M7 是干净的「冻结 + 计时」交付。**INIT/ARCHITECTURE 字面要求「时间 + 内存」（ARCHITECTURE 另提 throughput）**：此推迟是显式决策、非遗漏，归档时在 INIT 标注；吞吐量由「每尺寸时间 + 尺寸」派生、无需单列。

**D10 — M7 须钉死的边界（测试/验收清单）。**

1. **golden 四文件字节冻结**：对固定输入（D4）调 `generate_pattern` → `pattern_json(&result)`/`result.summary`/`result.preview_png`/`result.grid_png` 分别**逐字节等于** `tests/golden/{pattern.json,summary.txt,preview.png,grid.png}`；两张 PNG 另可解码校验尺寸（preview `160×200`）。
2. **arm64-Linux-canonical**：同测试在四 runner 都跑——ubuntu-24.04-arm 字节断言四产物、x86-64 ubuntu/macos/windows 断言浮点无关结构不变量（CI 自动）。
3. **BLESS 重生**：`BLESS=1` 写回 golden、正常断言被旁路；无 `BLESS` 时只读比对。
4. **benches 编译可跑**：`cargo bench` 五尺寸跑通；`cargo build --benches`/clippy `--all-targets` 编译过。
5. **无 runtime 依赖新增**：`criterion` 仅 `[dev-dependencies]`；`cargo tree`（非 dev）不新增。
6. **fail-loudly 可证**：加一条 `#[should_panic]` 自测——**直接调纯 `compare_or_panic("selftest", b"AAAA", b"BBBB")`**（**绝不**经 `assert_golden`/BLESS/写真 golden 分支——否则非 canonical 平台 `assert_golden` 会 `if !CANONICAL { return; }` 不 panic、令 `#[should_panic]` 在 macos/windows CI 红，且 `BLESS=1` 下走写分支亦不 panic）、断言它 panic（证明 diff/旁置 `.actual` 路径真会触发），写出的 `tests/golden/selftest.actual` 由 `.gitignore` 排除、**不**篡改 committed 金标准、三平台 + `BLESS=1` 下都安全、不破坏 CI（与 tasks §2.6 一致）。

## 风险 / 权衡

- [D1 arm64-Linux-canonical] → `sinf` 证伪了跨平台/跨架构字节一致、故不赌：arm64 Linux 冻字节、x86-64/macos/windows 断结构不变量；arm64 贴近移动端生产族 + Apple Silicon 原生容器 bless；代价是放弃跨平台 cell-值字节覆盖（本就无法证明）。
- [D3 冻 grid.png（超 INIT 清单）] → INIT 清单示例非禁止、M6 已标「M7 决」；冻它=每字节进 golden，代价仅 2 个小 PNG 入库。
- [D5 .gitattributes] → 漏了它 Windows 贡献者 commit 可污染文本金标准为 CRLF、令 canonical 字节断言失效；列为 M7 必交付、proposal 显著标注。
- [D9 内存推迟] → INIT 字面提「memory」；按 YAGNI 推迟并显式备注，Phase-2 上 dhat。非静默略过。
- [D2 库级 golden 不直接跑 CLI subprocess] → M6 已证 CLI==库字节，传递覆盖；若要 belt-and-suspenders 可让既有 `cli.rs` 额外比一个 CLI 产物 vs golden，但 YAGNI、增 3× subprocess 成本。

## Migration Plan

无运行时迁移：纯新增 `golden-tests` + `benchmarks` 两能力 + 测试/基准/夹具/`.gitattributes`；`criterion` 追加为 dev-dep（非破坏、非 runtime）。**不改** 任何既有引擎需求与代码、**无新 runtime 依赖**；CI workflow 仅**加 `ubuntu-24.04-arm` runner**（canonical 字节平台、其余 runner 跑结构断言）。回滚 = 撤销本变更。Phase-2 内存追踪是既定后续。

## Open Questions

- **benches 是否拆第二个 change**：默认**不拆**（一个 `add-golden-tests` 含 golden+benches）；若 PR 体积/评审节奏要求，benches 是可裁的那半（golden 是 Phase-1-freeze 必交付）。
- **M8 / Phase-2 前向**：① golden 冻的是**库面**（`generate_pattern`+`pattern_json` 字节）、非 FFI marshalling——M8「CLI==FFI」须另加**同机**边界字节比对（CLI 与 FFI 同输入产物字节相等；同机 → per-platform 确定性恰够），golden 不传递覆盖 C-ABI/Dart 层。② Phase-2 给 resize 上 rayon 会因并行**归约序**破坏**连同机**确定性——届时须先承诺确定性归约序（固定分块 + 有序合并、非 `par_iter().sum()`），否则 golden 在 canonical 平台亦 flaky；若以确定性换速度，golden 须改逐通道 tolerance 比对、且该 trade 须是 documented + blessed 决策（承接 D6）。
- **bench 是否加 per-stage 第二组**：默认只端到端（满足 ROADMAP）；per-stage 是 Phase-2 优化的 nice-to-have，可后续非破坏加。
