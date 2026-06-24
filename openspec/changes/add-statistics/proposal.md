## 为什么

里程碑 **M4 — Statistics**。M3 产出了 `BeadPattern { width, height, cells: Vec<u16> }`——配色后的真理源
（CLAUDE 规则 3）。M4 从它逐格统计每色豆数，产出 `ColorStat { code, name, count }` 列表与一段**可直接复制**
的 summary 文本（INIT「Summary Format」契约）。这是把"真理源"变成用户拿得到的"购物清单"的第一步；M5 预览
渲染、M6 pipeline 串联都会复用这里的统计原语。

## 变更内容

- `bead-core` 新增 `statistics` 模块（三个全函数原语，均纯整数、不 panic）：
  - `pub fn count_colors(grid: &BeadPattern, palette: &Palette) -> Vec<ColorStat>`——逐格按调色板下标计数，
    **只返回用到的色（count>0）**，按 **count 降序、平局取最低下标** 排序（确定性门，见 design D2）。
  - `pub fn total_beads(grid: &BeadPattern) -> u32`——`= cells.len() as u32`（豆总数，不依赖 palette）。
  - `pub fn generate_summary(grid: &BeadPattern, palette: &Palette) -> String`——逐字产出 INIT「Summary
    Format」（4 行头 + 空行 + 每用到色一行 `{code} {name}: {count}`）。
- 扩 `models` 模块：新增 `ColorStat { code: String, name: String, count: u32 }`（derive `Debug+Clone+PartialEq`，
  **不 derive `Eq`**，同 `PixelGrid`/`BeadPattern`）。
- 错误模型**复用** M1 的 `BeadError`——三个原语全 total、**不新增变体**（见 design D7）。
- **`BeadPattern` 不动、保持纯净**：统计是**按需派生**的独立产物，**不**作为 `BeadPattern` 的字段（design D1）。

> **自觉的、经 review 的文档反转（D1）**：早期 ARCHITECTURE/INIT 草图把 `stats: Vec<ColorStat>` 画在
> `BeadPattern` 里、并在 M3 标注「filled from M4」。M4 经探索后**改为 B 方案**：`BeadPattern` 永远只持
> `{width, height, cells}`，统计按需从 `cells` 派生（CLAUDE 规则 3 原话是 "derive from" 而非 "store"；
> `cells` 是 `pub`，存一个能与之脱节的 `stats` 字段 = M3-D4 拒绝的"会撒谎的派生数据"的晚一里程碑版）。
> 故本 change **同步校正**全套真理源（`ARCHITECTURE.md` 三处 + `INIT.md` + `ROADMAP.md` + `models/mod.rs`
> doc-comment + `color-matching` 规范 delta，逐处见 tasks 5.3–5.7、与「修改功能」一致），把
> `stats` 从 `BeadPattern` 拿掉、改述为"统计是派生产物、在 M6 pipeline 层与 grid 打包"。说明：M3 当时只辩护
> "保留注释（stats 将在 M4 来）"、并未 ship 该字段；B 是回答那条注释悬而未决的"怎么来"——故不矛盾，是有意
> 演进。`ColorStat` 结构本身不变；color-matching 主规范「BeadPattern 不含 stats」的要求被 B 强化（永久成立）。

## 功能 (Capabilities)

### 新增功能
- `statistics`: 从 `BeadPattern` 逐格统计每色豆数，产出确定性排序的 `ColorStat` 列表（count 降序、平局最低
  下标、只列用到的色）、豆总数 `total_beads`，与逐字匹配 INIT 契约的可复制 summary 文本。

### 修改功能
- `color-matching`：「BeadPattern 输出形状」需求的**括注**由"（统计属 M4）"改为"（统计在 M4 以派生函数
  `count_colors`/`total_beads`/`generate_summary` 提供，**永不**作为 `BeadPattern` 字段；见 M4-D1）"。规范性要求
  「`BeadPattern` 不含 `stats` 字段」**不变、被 D1 永久强化**；仅修正那条*前向括注*——它原读作"stats 字段将在 M4 到来"，
  D1 反转后会误导，故经 `## 修改需求` delta（`specs/color-matching/spec.md`，本 change 内）显式修正，而非靠 prose
  "顺带澄清"（一条已生效规范的需求文字不应在 delta 之外被悄悄改）。**不改** palette / image-grid 的任何需求。

## 非目标（Non Goals）

按 YAGNI 推迟 / 不做：

- `stats` 作为 `BeadPattern` 字段 → 永不（D1：派生而非存储）。
- 排序"双模式"（count 降序 ↔ 按 code）→ App（M9）层：消费方对返回的 `Vec<ColorStat>` 自行 `.sort_by(code)`
  即可（ColorStat 自带 code，引擎零改动）；M4 只产一个规范序，留 `ponytail:` 升级路径、不留代码（D2）。
- `pipeline::generate_pattern` 串联与 `GenerateResult`（grid+stats 打包类型）→ M6（M4 的三个 `pub` 是库内/
  pipeline 复用原语，非 FFI 入口）。
- preview / grid 渲染 → M5（同样从 `BeadPattern`/`cells` 派生）。
- CSV / 其它导出格式 → 非 MVP（INIT 明确「CSV export is not required」）。
- `rayon` 并行 → Phase 2（M4 单线程）。

## 影响

- **代码**：
  - `crates/bead-core/src/statistics/mod.rs`（新）
  - `crates/bead-core/src/models/mod.rs`（扩 `ColorStat`；并改 `BeadPattern` doc-comment 的 in-M3 时态措辞）
  - `crates/bead-core/src/lib.rs`（改：`pub mod statistics;` + 重导出 `count_colors / total_beads /
    generate_summary` 与 `ColorStat`；**不新增 `BeadError` 变体**）
- **依赖**：无新增（统计是纯整数计数 + M1 `Palette` + M3 `BeadPattern`）。
- **确定性**：计数全程整数（稠密 `Vec` 计数、无 `HashMap`/`HashSet`、无 `f32`、无 `rayon`）、排序键全整数
  （count 降序 + 下标升序平局）→ **同字节输入 → 逐字节相同 `Vec<ColorStat>` 与 summary，且跨架构位精确**。
  据此钉硬编码 golden（`Vec<ColorStat>` + 精确 `summary.txt` 字符串），延续 M3-D8、为 M7 frozen 减负。
- **里程碑 / Phase**：里程碑 M4；Phase 1（单线程）。
- **文档**：真理源同步校正（D1，**逐处见 tasks 5.3–5.7**）——`ARCHITECTURE.md` **三处**（Data Model Layer 去
  `BeadPattern.stats` 字段 + 其后「forward-looking field … populated starting in M4」prose 段 + statistics 段无冒号
  summary 示例改带冒号）、`INIT.md`（Data Models 同步）、`ROADMAP.md` M4（"`BeadPattern.stats` populated" →
  "statistics derived from `BeadPattern`"）、`crates/bead-core/src/models/mod.rs` 的 `BeadPattern` doc-comment；
  **外加** `color-matching` 规范的 `## 修改需求` delta（修正"（统计属 M4）"括注，见「修改功能」）。
