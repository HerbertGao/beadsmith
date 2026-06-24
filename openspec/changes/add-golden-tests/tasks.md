## 1. 仓根脚手架：.gitattributes + tests/golden/

- [x] 1.1 新建 `/.gitattributes`（仓根，D5）：
  `tests/golden/*.json -text` / `tests/golden/*.txt -text` / `tests/golden/*.png binary`
  （`-text`=永不规范化行尾、`binary`=`-text -diff`；堵 Windows `autocrlf` 把文本金标准改 CRLF、令 canonical(arm64 Linux) 字节断言失效——是 canonical 字节断言长期有效的前提）；同时 `.gitignore` 加 `tests/golden/*.actual`（失败旁置物、不入库）
- [x] 1.2 新建 `tests/golden/`（仓根首建）下 `README.md`：记**固定设置**（`samples/gradient.png`、16×20、`Lanczos3`、`cell_size 10`、`artkal_s`）、
  四个金标准文件清单、**重生命令** `BLESS=1 cargo test -p bead-cli --test golden`、以及「golden + `--locked` = dep-bump 守卫」说明（D6/D7）。金标准四文件由 §2 在 **arm64 Linux canonical 平台**生成后入库；README 记明 canonical 平台（arm64 Linux；x86-64 Linux/macos/windows 只跑结构断言、不比字节）、夹具是 committed 32×40 `gradient.png`（缩放到 16×20）、`.gitignore` 已排除 `tests/golden/*.actual`。

## 2. golden 测试 + 重生 + 生成金标准（crates/bead-cli/tests/golden.rs）

- [x] 2.1 新建 `crates/bead-cli/tests/golden.rs`，模块 doc：库级 golden（调 `bead_core::generate_pattern`，比对四产物字节 vs `tests/golden/`；
  CLI 经 `fs::write` 原样写这四块字节，故库级 golden 传递性覆盖 CLI，规则 5）；经 `Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")` 读仓根
  （`samples/gradient.png`、`palettes/artkal_s.json`、`tests/golden/`）；**零新依赖**（仅 `bead-core` + std；**不**解析 JSON——结构断言直接读 `GenerateResult` 结构字段 + 对 `pattern_json(&result)` 串做字节位置检查、PNG 用 `bead_core::decode_image`，故 `bead-cli` 不引 `serde_json`/`image`）。`pattern_json` 在 `bead_core::pipeline::pattern_json`（非 crate 根），按 `main.rs:9` 写全路径 import。
- [x] 2.2 固定输入助手：读 `samples/gradient.png` 字节 + `load_palette(&fs::read(artkal_s))`；构 `GenerateOptions { width: 16, height: 20, ..Default::default() }`
  （`Lanczos3`/`cell_size 10` 由 Default 来，**不暴露 CLI flag**，D4；夹具是 committed 32×40 `gradient.png`、此处缩放**到** 16×20，**勿重生成** 16×20 夹具，16:20==32:40 → crop no-op）；`let result = generate_pattern(&img_bytes, &palette, &opts).expect(...)`。
- [x] 2.3 两个助手（D6）：
  - `fn compare_or_panic(name: &str, expected: &[u8], actual: &[u8])`（**纯比对、不读盘、不看 BLESS**）：相等则过；**不等**则把 `actual` 写到 `tests/golden/<name>.actual` 旁置，再 `panic!` 带清晰消息（点名 `<name>`、提示「比对 `<name>` vs `<name>.actual`、若有意改动跑 `BLESS=1 cargo test -p bead-cli --test golden` 重生并 `git diff` 评审」）。
  - `fn assert_golden(name: &str, actual: &[u8])`：`const CANONICAL: bool = cfg!(target_os = "linux") && cfg!(target_arch = "aarch64")`（canonical = arm64 Linux、CI 参考 ubuntu-24.04-arm）。`BLESS` 置位（`std::env::var("BLESS").is_ok()`）→ `assert!(CANONICAL, "BLESS 仅限 Linux canonical，避免提交非 canonical 字节")` 后 `fs::write(path, actual)`（重生、旁路断言）；否则 `if !CANONICAL { return; }`（**字节比对仅 canonical**、非 canonical 由 §2.4b 覆盖）后读 golden、调 `compare_or_panic(name, &golden, actual)`。
  - `// ponytail: 拆出纯 compare_or_panic（自证测试只调它、绝不经 BLESS/写 golden 分支）+ ~10 行 BLESS 重生，同 insta 模式、无新依赖`
- [x] 2.4a `#[test] fn golden_matches_canonical()`（D10.1；canonical 字节冻结）：对 `result` 调 4 次 `assert_golden`（其内部自 gate：非 canonical 平台每次调用直接 return、不比字节）：
  `assert_golden("pattern.json", pattern_json(&result).as_bytes())`、`assert_golden("summary.txt", result.summary.as_bytes())`、
  `assert_golden("preview.png", &result.preview_png)`、`assert_golden("grid.png", &result.grid_png)`。
- [x] 2.4b `#[test] fn golden_structure_all_platforms()`（D10.2；三平台跑、浮点无关结构不变量、**零新依赖**）：
  ① PNG：`bead_core::decode_image(&result.preview_png)?` 成功且 `.dimensions()==(160,200)`、`decode_image(&result.grid_png)?` 成功且尺寸=几何公式值；
  ② `pattern.json`：直接断言 `result.pattern.{width==16,height==20,cells.len()==320}`、`total_beads(&result.pattern)==320`、各 cell 索引 `< palette.colors.len()`（=199、用 `.len()`、勿写死字面量）；键序对 `pattern_json(&result)` 串做 `find()` 字节位置单调性检查（`brand<width<height<cells<total<stats`）；
  ③ `summary.txt`：对 `result.summary` 拆行断言首行 `Bead Pattern Summary`、`Size: 16 x 20`、`Total Beads: 320`、`Palette: Artkal S`、空行分隔；**按首个空行切出 body（header 4 行之后）**、仅对 body 各色行 `rsplit_once(": ")` 出末段 `parse::<u32>()`、计数和==320（**勿对整串求和**——header `Total Beads: 320` 也匹配 `rsplit_once`、会 double-count 成 640）；
  ④ `grid.png`：解码后断言 `(0,0)`==BG `[255,255,255]`（左上 margin、标号从 x≥104/y≥104 起、(0,0) 必空）、且粗分隔线像素 `(118,19)`==BOLD `[120,120,120]`（几何 margin_left=18/margin_top=14/cell=10：bx=10 竖粗线 x=18+10·10=118、y=14+5=19 避开横边界与标号；坐标须落 STEP 倍数边界、避开 bead 内部与标号带，否则会变 float 相关）。
  - `// ponytail: 结构断言全走 GenerateResult 结构 + bead_core::decode_image + std 字符串，不引 serde_json/image 进 bead-cli（真零新依赖）；不为「漂亮 diff」引 similar（YAGNI）`
- [x] 2.5 **生成并提交金标准（须在 arm64 Linux canonical 平台——原生 arm64 容器或 ubuntu-24.04-arm）**：跑 `BLESS=1 cargo test -p bead-cli --test golden` 生成 `tests/golden/{pattern.json,summary.txt,preview.png,grid.png}`；
  **肉眼看** `preview.png`/`grid.png` 合理（是 gradient 拼豆图、非空/非乱）；确认 `summary.txt` 首行 `Bead Pattern Summary`、`pattern.json` 键序 `{brand,width,height,cells,total,stats}`；
  把四文件 + `.gitattributes` + `README.md` 入库。随后**去掉 BLESS** 跑 `cargo test -p bead-cli --test golden` 应**绿**（只读比对）。
- [x] 2.6 fail-loudly 自证（D10.6）：加一条 `#[should_panic(expected = ...)] fn compare_or_panic_detects_mismatch()`——**直接调 `compare_or_panic("selftest", b"AAAA", b"BBBB")`**（绝不经 `assert_golden`/BLESS/写真 golden 分支；写出的 `tests/golden/selftest.actual` 已被 `.gitignore` 排除）、断言它 panic，证明 diff/旁置路径真触发。三平台都跑、`BLESS=1 cargo test` 下也安全（自证不走 bless 写 golden、不污染金标准）。常驻自动测试、不破坏 CI。

## 3. Criterion 基准（crates/bead-core/benches/bench.rs）

- [x] 3.1 `crates/bead-core/Cargo.toml`：加 `[dev-dependencies] criterion = { version = "0.8", default-features = false }`（关 html/cargo-criterion 保精简；**dev-dep、非 runtime**，D8）
  + `[[bench]] name = "bench" harness = false`。
- [x] 3.2 新建 `crates/bead-core/benches/bench.rs`（D8）：`criterion_group!`/`criterion_main!`；一个 `benchmark_group("generate_pattern")`，五尺寸
  `[(40,40),(80,100),(100,100),(150,150),(300,300)]`（**目标**尺寸）各一个 `BenchmarkId`。每尺寸 **setup 时合成源图（须大于目标、每维 2×）**：`image::RgbImage::from_fn(2*w,2*h,|x,y| Rgb([(x%256)as u8,(y%256)as u8,((x+y)%256)as u8]))`（源=目标会命中 `imageops::resize` 拷贝短路、漏测 Lanczos 重采样）
  → 编码 PNG 字节（同 M6 `demo_png`）；palette 经 `CARGO_MANIFEST_DIR/../..` 读 `palettes/artkal_s.json` 一次。`b.iter(|| generate_pattern(black_box(&png_bytes), black_box(&palette), black_box(&opts)))`
  （`opts = GenerateOptions{width:w,height:h,..Default::default()}`）。**不 committed 大图**。
  - `// ponytail: 测库入口 generate_pattern 端到端（M8/FFI 与 CLI 真调的东西）；per-stage 拆分留 Phase-2 优化基线、非必需`

## 4. 收尾验证 + delta 确认

- [x] 4.1 `cargo fmt --check`、`cargo clippy --locked --all-targets --all-features -- -D warnings`（含 benches 编译，防 bit-rot）、`cargo test -p bead-cli --test golden`（本机平台绿）、
  `cargo bench`（五尺寸跑通、或至少 `cargo build --benches` 编译过）全绿。
- [x] 4.2 确认**无新 runtime 依赖**：`criterion` 仅在 `bead-core` 的 `[dev-dependencies]`；`cargo tree -e normal`（非 dev）**不**新增 crate；`bead-cli` 依赖不变（golden 测试零新依赖）。
- [x] 4.3 **三平台由 CI 验证**：本机只能跑当前平台；PR 上 CI `cargo test --locked --workspace --all-features` 在 ubuntu/ubuntu-24.04-arm/macos/windows 跑 golden——
  **ubuntu-24.04-arm（canonical, arm64）字节断言四产物**（`golden_matches_canonical`）、其余 runner（x86-64 ubuntu/macos/windows）断言浮点无关结构不变量（`golden_structure_all_platforms`）。全绿 = M7 done-when 满足（canonical 冻结 + 跨平台结构防护）；**无 fallback 决策**——posture 已定为 arm64-Linux-canonical。
- [x] 4.4 确认 delta 与文档：`golden-tests` + `benchmarks` 两个**新增**能力规范（归档时建 `openspec/specs/{golden-tests,benchmarks}/spec.md`）；
  **无修改能力**（golden 只验证既有引擎、不改 pipeline/renderer/statistics 需求）；ROADMAP(M7)/INIT(Golden+Benchmark) 一致；归档时在 INIT 标注两处偏离——① `grid.png` 已纳入 M7 golden（INIT 清单原不含、design D3）、② 基准内存追踪推迟 Phase-2（INIT 字面提「时间+内存」、design D9）——使源文档与交付一致；③ 在 ARCHITECTURE 显式 carve-out：rule 2/确定性对 float(Lanczos3) 路径按**同平台/同架构/同 `image` 版本**理解（canonical=arm64 Linux、CI 参考 ubuntu-24.04-arm）——不改 CLAUDE.md 硬规则、只在 ARCHITECTURE 锚定该解释。
