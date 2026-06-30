# golden-tests 规范

## 目的
对一组固定夹具与固定设置，把 `generate_pattern` + `pattern_json` 经库 API 产出的四块产物（`pattern.json` / `summary.txt` / `preview.png` / `grid.png`）冻结成 committed 字节金标准，作为算法/编码/依赖回归的守卫。设置钉在测试代码、不扩 CLI 面；库级 golden 经「CLI 原样 `fs::write`」传递性覆盖 CLI 写出（规则 5），且测试只在 CLI 层读盘、`bead-core` 不碰文件系统（规则 1）。字节断言在 canonical 平台（arm64 Linux）成立，因 Lanczos3 经 `f32::sin`、默认 LabMatcher 经 `cbrt`/`powf` 跨平台 ULP 不保证——其余平台跑同一测试但只断言浮点无关的结构不变量，确定性（规则 2）在本范围内澄清为「同平台/同 `image` 版本逐字节相同」。`.gitattributes` 保文本金标准跨平台恒为 LF；任何改字节的改动须响亮失败，`BLESS=1`（仅 canonical 生效）重生使变化进 `git diff` 受评审。

## 需求

### 需求:golden 冻结四个产物的字节金标准
对一组固定输入（夹具 + 固定设置），系统**必须**提供 committed 金标准并（在 canonical 平台 **arm64 Linux**、CI 参考 ubuntu-24.04-arm）断言 `pattern_json(&result)`、`result.summary`、`result.preview_png`、
`result.grid_png` **逐字节等于** `tests/golden/{pattern.json, summary.txt, preview.png, grid.png}`。金标准**必须**经**库 API**（`generate_pattern` +
`pattern_json`）产出比对——因 CLI 经 `fs::write` **原样**写这四块字节（M6 已证、无变换），库级 golden **传递性覆盖** CLI 写出（规则 5）。golden 测试**必须**
只在测试/CLI 层读盘，`bead-core` **禁止**碰文件系统（规则 1）——故测试落 `crates/bead-cli/tests/golden.rs`、经 `CARGO_MANIFEST_DIR/../..` 读仓根。

#### 场景:四产物逐字节等于金标准
- **当** 对固定输入调 `generate_pattern` 再 `pattern_json(&result)`
- **那么** `pattern.json`/`summary.txt`/`preview.png`/`grid.png` 四块产物**分别逐字节等于** `tests/golden/` 下对应文件；两张 PNG 另可解码、`preview` 尺寸为 `160×200`

### 需求:固定输入与固定设置（不扩 CLI 面）
golden **必须**用固定夹具 `samples/gradient.png` 与固定设置 `width=16` / `height=20` / `ResizeOptions::default()`（`Lanczos3`）/ `RenderOptions::default()`
（`cell_size 10`）/ `palettes/artkal_s.json`。设置**必须**钉在测试代码里，**禁止**为此暴露任何新 CLI flag。`16:20 == 4:5 == 32:40` → 裁切为 no-op（消除裁切
偏移变量），且与 M6 `cli.rs` e2e 同设置（金标准与既有 e2e 构造上一致）。

#### 场景:固定设置产出确定性四产物
- **当** 用上述固定夹具 + 设置运行 golden 测试
- **那么** 产出与金标准一致；**改变任一设置**（宽高/filter/cell_size/palette）即与金标准不符——设置是金标准的一部分

### 需求:真实默认路径、arm64-Linux-canonical 字节断言 + 跨平台结构不变量
golden **必须**用引擎**真实默认 `Lanczos3`**（CLI 默认、用户真拿到的路径），**禁止**为求跨平台通过而改用非默认 filter（如 `Nearest`）掩盖真实 resize 路径的回归覆盖。**字节级**金标准断言**只在 canonical 平台（**arm64 Linux**，gate `cfg!(target_os="linux") && cfg!(target_arch="aarch64")`，CI 参考 ubuntu-24.04-arm）**成立；其余平台（x86-64 Linux/macOS/Windows）跑**同一测试**但仅断言**浮点无关的结构不变量**（PNG 可解码、`preview` 160×200、`pattern.json` 键序与计数、`summary.txt` 结构、`grid.png` 固定常量像素），**禁止**跨平台比对走浮点 resize 的 cell 颜色字节。依据 design D1：`image 0.25.10` 的 Lanczos3 权重经 `f32::sin`（`sample.rs:149-156`）、加之默认 `LabMatcher` 经 `cbrt`/`powf` → 跨平台 ULP 不保证 → resize 与配色字节跨平台一致**不保证、不要求**。确定性（规则 2）在本 change 范围内澄清为**同平台/同 `image` 版本**逐字节相同。

#### 场景:canonical 平台字节冻结、非 canonical 平台结构断言
- **当** CI 在 ubuntu-24.04-arm（canonical, arm64）跑 golden 测试
- **那么** 四产物**逐字节等于** committed 金标准（arm64-Linux-canonical 核心冻结）
- **当** CI 在 x86-64 Linux/macOS/Windows 跑同一 golden 测试
- **那么** 浮点无关结构不变量全部通过（PNG 可解码、`preview` 160×200、`grid` 尺寸=几何公式、`pattern.json` 键序/计数/索引合法、`summary.txt` 结构/色行计数和=320（仅空行后 body、排除 header）、`grid.png` 背景与粗线像素为常量色）；**不**断言 cell 颜色字节；任一结构不变量不符即**响亮失败**

### 需求:跨平台字节稳定经 .gitattributes 保障
仓库**必须**提供 `.gitattributes` 把 `tests/golden/*.json` 与 `tests/golden/*.txt` 标 `-text`（永不规范化行尾）、`tests/golden/*.png` 标 `binary`。
否则 Windows `autocrlf` 会把文本金标准 checkout 成 CRLF，而引擎只写 `\n`（`statistics` 的 summary 用 `\n`）→ 金标准字节带 `\r`、与引擎输出**因非引擎
原因**不符。此文件保证文本金标准跨平台 checkout/commit 恒为 LF、是 canonical(arm64 Linux) 字节断言长期有效的前提。

#### 场景:Windows checkout 不改金标准字节
- **当** 在启用 `autocrlf` 的 Windows 上 checkout 仓库并跑 golden
- **那么** committed 文本金标准逐字节保持 `LF`、与引擎输出一致（无 `\r` 注入）

### 需求:故意改算法响亮失败且重生可审
任何改变输出字节的算法/编码改动**必须**使 golden 测试**响亮失败**（清晰消息 + 指向重生命令；统一字节比对、失败旁置
`<name>.actual` 供目视），**禁止**静默通过。系统**必须**提供 `BLESS=1` 重生路径：置位时写当前输出**覆盖**金标准并旁路断言，使重生进入 `git diff` 被评审。`BLESS` **必须**仅在 canonical 平台（Linux）生效——非 canonical 平台置位须拒绝/不写（否则会提交非 canonical 字节、令 ubuntu CI 立刻变红）。
改字节的依赖升级（`image`/`png`）在 CI 的 `--locked` 下经 golden **同样响亮失败**（`Cargo.lock` 入库 + golden = dep-bump 守卫，承接 M5-D3）。

#### 场景:改算法后测试红、bless 后 diff 可审
- **当** 故意改一处影响输出的算法（或升级改字节的依赖）并跑 golden
- **那么** 测试**失败**并提示重生命令；运行 `BLESS=1` 后金标准更新、`git diff tests/golden/` **精确显示**字节变化供评审通过/驳回（静默漂移不可能）
