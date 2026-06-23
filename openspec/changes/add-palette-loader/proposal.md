## 为什么

里程碑 **M1 — Palette Loader**。引擎要把图片映射到真实豆子颜色，第一步是有
一个"真实豆子调色板"的可信来源。在此之前 bead-core 只有一个占位 `BeadError`，
没有任何业务能力。M1 先打通"加载 + 校验调色板"，为 M2/M3（缩放、配色匹配）
提供输入，并把"字节进、数据出、无文件系统假设"的引擎边界模式定下来。

## 变更内容

- 在 `bead-core` 新增 `palette` 模块：从 JSON **字节**加载并校验调色板。
  - `load_palette(&[u8]) -> Result<Palette, BeadError>`：解析 + 校验，一步到位。
  - `validate_palette(&Palette) -> Result<(), BeadError>`：复查结构不变量。
- 新增数据模型 `Palette { brand, colors }` 与 `PaletteColor { code, name, rgb }`。
  JSON 里 `rgb` 是 `"#RRGGBB"` 字符串，加载时解析为 `[u8; 3]`。
- 校验：`colors` 非空、`code` 唯一、每个 `rgb` 是合法 `#RRGGBB`（fail-fast）。
- 给 `BeadError` 增加两个变体：`PaletteParse`（JSON 语法/结构错）、
  `InvalidPalette { reason }`（语义错）。
- 附带一个真实调色板 `palettes/artkal_s.json`（Artkal 5mm Midi S 系列，199 色：159 标准 S + 40 扩展），
  并新增仓根 `NOTICE` 与数据出处说明。
- 新增依赖 `serde`(derive) + `serde_json`（见「影响」中的理由）。

> 契约校准：ROADMAP M1「Done when」提到"坏 hex 的单测"，实际由 `load_palette`
> 拒绝（hex 在加载阶段就转成 `[u8;3]`，构造出的 `Palette` 按类型即合法，
> `validate_palette` 不再、也无法复查 hex）。spec 措辞按此校准。

## 功能 (Capabilities)

### 新增功能
- `palette`: 从 JSON 字节加载并校验豆子调色板（结构、唯一 code、合法 hex），
  产出内存中有序的 `Palette`；并约定调色板 JSON 文件格式。

### 修改功能
<!-- 无：M1 之前没有任何生效 spec。 -->

## 非目标（Non Goals）

本次**不做**（按 YAGNI 推迟到对应里程碑/或永不做）：

- `code → index` 查找索引 → M3（匹配器是第一个按 code 查的消费者）。
- collect-all 校验报告（一次列出所有错）→ M6 的 `bead-cli palette validate`；
  M1 只做 fail-fast。
- `#RGB` 简写 / 无 `#` / 8 位 `#RRGGBBAA` / 命名色 / alpha 透明度。
- `brand` 取值约束（枚举 Artkal/Perler/Hama）→ 现为自由字符串。
- `Palette` 的 `Serialize` 回写 JSON、调色板合并 / 注册表 / 多调色板。
- 把 `InvalidPalette` 拆成 typed 子变体 → 等 M6 CLI 需要分类时再拆。
- 任何文件系统读取写在 core 里 → 由 `bead-cli`（M6）负责，core 只收字节。

## 影响

- **代码**：
  - `crates/bead-core/src/lib.rs`（改：`pub mod palette` + 重导出 + 两个 `BeadError` 变体）
  - `crates/bead-core/src/palette/mod.rs`（新：模型、加载、校验、hex 解析、测试）
  - `crates/bead-core/Cargo.toml`、根 `Cargo.toml`（加依赖）
- **数据 / 法务**：
  - `palettes/artkal_s.json`（新，真实数据）、`NOTICE`（新）、`palettes/` 数据说明（新）
  - 数据基于 `maxcleme/beadcolors`（**MIT**，与 Apache-2.0 兼容），需保留 MIT 署名。
  - hex 为**社区近似值、非官方**，且仅含 199/~200+ 官方 S 色——须如实注明。
- **依赖（新增，需理由）**：
  - `serde`（features=`["derive"]`）+ `serde_json`，加到 `[workspace.dependencies]`（`"1"`）。
  - 理由：stdlib 无 JSON 解析；手写 JSON 解析器是典型过度工程；INIT.md 技术栈
    已列入二者。`serde_json::from_slice` 直接吃 `&[u8]`，契合"收字节"的边界。
- **确定性**：M1 全程不使用 `HashMap`（重复 code 用有序扫描，错误只点名单个 code），
  避免迭代顺序泄漏进错误信息或未来 golden 输出。加载对同一字节输入逐字节确定。
- **里程碑 / Phase**：里程碑 M1；不涉及算法 Phase（尚无配色匹配）。
