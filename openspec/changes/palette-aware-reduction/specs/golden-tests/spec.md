# golden-tests 规范（增量）

## MODIFIED Requirements

### 需求:固定输入与固定设置（不扩 CLI 面）
golden **必须**用固定夹具 `samples/gradient.png` 与固定设置 `width=16` / `height=20` / `ResizeOptions::default()`（**`Triangle`**）/ `RenderOptions::default()`（`cell_size 10`）/ `MatcherKind::default()`（`Oklab`）/ `palettes/artkal_s.json`。设置**必须**钉在测试代码里，**禁止**为此暴露除已有 `--matcher` 之外的任何新 CLI flag；golden 本身仍走库 API 默认路径，不通过 CLI flag 选 matcher 或 filter。`16:20 == 4:5 == 32:40` → 裁切为 no-op（消除裁切偏移变量），且与 M6 `cli.rs` e2e 同宽高/渲染设置（金标准与既有 e2e 构造上一致）。

#### 场景:固定设置产出确定性四产物
- **当** 用上述固定夹具 + 设置运行 golden 测试
- **那么** 产出与金标准一致；**改变任一设置**（宽高/filter/cell_size/palette/matcher）即与金标准不符——设置是金标准的一部分（`filter` 默认值已由 `Lanczos3` 翻为 `Triangle`，金标准随之重烤）

### 需求:真实默认路径、arm64-Linux-canonical 字节断言 + 跨平台结构不变量
golden **必须**用引擎**真实默认 `Triangle` + `MatcherKind::default()==Oklab`**（CLI 默认、用户真拿到的路径），**禁止**为求跨平台通过而改用非默认 filter（如 `Nearest`）或非默认 matcher（如 `Lab`/`Rgb`）掩盖真实默认路径的回归覆盖。**字节级**金标准断言**只在 canonical 平台（**arm64 Linux**，gate `cfg!(target_os="linux") && cfg!(target_arch="aarch64")`，CI 参考 ubuntu-24.04-arm）**成立；其余平台（x86-64 Linux/macOS/Windows）跑**同一测试**但仅断言**浮点无关的结构不变量**（PNG 可解码、`preview` 160×200、`pattern.json` 键序与计数、`summary.txt` 结构、`grid.png` 固定常量像素），**禁止**跨平台比对走浮点 resize / 感知 matcher 的 cell 颜色字节。依据 design D1：默认 `Triangle` 是 `f32` 重采样（线性权重、无 `Lanczos3` 的 `f32::sin`；跨架构 byte 一致的**校验**仍落 M8「CLI==FFI」，M2/M7 不预烤 f32 重采样 golden），加之默认 `OklabMatcher` 经 `cbrt`/`powf` → 跨平台 ULP 不保证 → resize 与配色字节跨平台一致**不保证、不要求**。确定性（规则 2）在本 change 范围内澄清为**同平台/同 `image` 版本**逐字节相同。

#### 场景:canonical 平台字节冻结、非 canonical 平台结构断言
- **当** CI 在 ubuntu-24.04-arm（canonical, arm64）跑 golden 测试
- **那么** 四产物**逐字节等于** committed 金标准（arm64-Linux-canonical 核心冻结）
- **当** CI 在 x86-64 Linux/macOS/Windows 跑同一 golden 测试
- **那么** 浮点无关结构不变量全部通过（PNG 可解码、`preview` 160×200、`grid` 尺寸=几何公式、`pattern.json` 键序/计数/索引合法、`summary.txt` 结构/色行计数和=320（仅空行后 body、排除 header）、`grid.png` 背景与粗线像素为常量色）；**不**断言 cell 颜色字节；任一结构不变量不符即**响亮失败**
