## 上下文

里程碑 M1。当前 `bead-core` 只有占位 `BeadError`，无业务能力。本设计在 `bead-core`
新增 `palette` 模块：从 JSON 字节加载并校验调色板。这是引擎第一段真实逻辑，也借此
把"字节进、数据出、无文件系统假设"（ARCHITECTURE 规则 1）的边界模式定下来——后续
所有加载器/导出器都继承它。约束：纯库、确定性、`thiserror + Result<T, BeadError>`。

设计已在探索阶段收敛并冻结（option A：typed-by-construction）。

## 目标 / 非目标

**目标：**
- `load_palette(&[u8]) -> Result<Palette, BeadError>`：解析 + 校验一步到位。
- `validate_palette(&Palette) -> Result<(), BeadError>`：复查结构不变量。
- 严格 `#RRGGBB` → `[u8;3]` 解析；fail-fast 校验；确定性。
- 附带真实 `palettes/artkal_s.json`。

**非目标：**
- `code → index` 索引（M3）、collect-all 校验（M6）、`Serialize` 回写、多调色板。
- `#RGB`/无 `#`/8 位/命名色/alpha、`brand` 枚举约束、`InvalidPalette` 拆 typed 子变体。
- 任何文件系统读取写进 core（由 CLI 负责）。

## 决策

**D1 — I/O 边界：收字节，不收路径。** `load_palette(&[u8])`，CLI 才做 `fs::read`。
- 替代方案：`load_palette(path)`。否决理由：违反 ARCHITECTURE 规则 1（core 不感知
  文件系统）；FFI/移动端也无统一路径语义。

**D2 — `Palette` 容器 + 私有 DTO。** 公开 `Palette { brand, colors: Vec<PaletteColor> }`
与 `PaletteColor { code, name, rgb: [u8;3] }`（derive `Debug+Clone+PartialEq`，**不 derive
`Eq`**）；另设**私有** `RawPalette/RawColor`（`rgb: String`）专供 serde 反序列化。
- **未知字段策略**：`RawPalette/RawColor` 不加 `#[serde(deny_unknown_fields)]`——多余字段
  静默忽略（宽松）。理由：schema 极简且文件由我们生成；宽松保留对未来可选元数据的前向
  兼容；缺必需字段（如缺 `rgb`）仍由 serde 报 missing-field → `PaletteParse`。严格校验
  用户调色板是 M6 的事。
- **模型位置**：M1 把 `Palette/PaletteColor` 放在 `palette/mod.rs`；ARCHITECTURE 画的
  `models/` 层尚未存在（暂无其它模型），等 M2 引入 `BeadCell` 等时再建 `models/` 并经
  re-export 迁移（非破坏）。
- 替代方案：直接给 `Palette` 派生 `Deserialize` 并自定义 `rgb` 反序列化器。否决理由：
  会把 hex 解析逻辑塞进 serde 回调，分散且难测；DTO 让 serde 当"哑映射"，解析+校验
  集中在一处可审计。`ponytail:` DTO 是私有的，第二个调用方真需要原始字符串时再合并。
- 替代方案：派生 `Eq`。否决理由：M1 只需 `PartialEq`（`assert_eq!` + 确定性比较）；`Eq`
  是公开 API 承诺，且会阻挡 Phase 3 加感知浮点字段（如 `Lab:[f32;3]`）——按 YAGNI 不
  derive，真有 HashMap-key 需求再加。

**D3 — hex 解析时机与严格度。** serde 把 `rgb` 反序列化成 `String`，加载时 `parse_hex`
转 `[u8;3]`；严格只认 `#RRGGBB`（前导 `#`、正好 6 位 ASCII 十六进制、大小写不敏感）。
`parse_hex(&str)` 自身返回**不含 code 的错误**；由 `load_palette` 捕获并包装成
`InvalidPalette { reason }`、reason 点名当前颜色的 `code`（满足 spec「reason 点名出错 code」）。
- 替代方案：宽松接受 `#RGB`/无 `#`/8 位。否决理由：调色板文件由我们生成，单一严格
  格式无歧义；宽松是 speculative，违反 YAGNI。

**D4 — `validate_palette` 只查结构（契约校准）。** hex 在 load 阶段已转 `[u8;3]`，构造
出的 `Palette` 按类型即合法，故 `validate_palette` 只复查 `colors` 非空、`code` 唯一。
- 替代方案：在 Raw（字符串）形态上校验以便 validate 能查 hex。否决理由：那样 `Palette`
  可能装着未校验数据，违背"用类型让非法状态不可表示"。
- 注意：ROADMAP M1「Done when」的"坏 hex 单测"实际打在 `load_palette`，spec 已校准。

**D5 — 校验策略：fail-fast + 确定的校验顺序。** 遇第一个错即返回；`BeadError` 是单值，
不是错误列表。为使"第一个错"确定（多个错并存时报告哪个不依赖实现顺序），`load_palette`
必须按固定顺序：**① `colors` 非空检查 → ② 按 `colors` 顺序逐个 `parse_hex`（首个坏 hex
即返回，点名其 `code`）→ ③ 唯一 `code` 检查（首个重复即返回，点名该 `code`）**。
spec / tasks 与此顺序一致。
- 替代方案：collect-all 一次列全部错。否决理由：那是 M6 `bead-cli palette validate` 的
  需求；M1 不需要，YAGNI。

**D6 — 错误模型：两个变体。** `PaletteParse(#[from] serde_json::Error)`（`?` 自动包装
JSON 语法/结构错）+ `InvalidPalette { reason: String }`（语义错，reason 确定性点名单个
code）。`BeadError` 已 `#[non_exhaustive]`。
- 替代方案：每类失败一个 typed 变体（`EmptyPalette`/`DuplicateCode`/`InvalidHex`）。
  否决理由：M1 无人 match 变体，是过早分类；`#[non_exhaustive]` 允许 M6 需要分类时
  无破坏性地再拆。`ponytail:` `InvalidPalette` 是 stringly-typed 兜底，M6 需机读分类再拆。

**D7 — 依赖：serde + serde_json。** `serde`(`derive`) + `serde_json`，加到根
`[workspace.dependencies]`（`"1"`），core 以 `.workspace = true` 引用。
- 替代方案：手写 JSON 解析。否决理由：stdlib 无 JSON；手写解析器是典型过度工程；
  INIT.md 技术栈已列入二者；`from_slice` 直接吃 `&[u8]` 契合 D1。

**D8 — 确定性：禁 HashMap。** 重复 code 检测用有序 `Vec` 扫描；错误只点名单个 code。
- 替代方案：用 `HashSet`/`HashMap` 查重。否决理由：迭代顺序不确定，会泄漏进错误信息
  与未来 golden 输出，违反确定性硬规则。

**D9 — 文件布局：目录模块 `palette/mod.rs`。** 对齐 ARCHITECTURE 画的 `palette/` 目录，
给 M3 的 `index.rs` 兄弟预留。
- 替代方案：单文件 `palette.rs`。否决理由：M3 必然加 index 兄弟，届时要改目录；多一个
  目录的成本可接受。

**D10 — 数据来源与署名。** `palettes/artkal_s.json` 基于 `maxcleme/beadcolors`
（MIT，199 色：159 标准 S + 40 扩展，`code,name,R,G,B`）转换；记为 5mm Midi S；字段用 `rgb`（`"#RRGGBB"`）。
**`brand` 取值为系列标签 `"Artkal S"`**——与 INIT.md 摘要格式（第 247 行 `Palette: Artkal S`）
一致；有意区别于 INIT schema 示例的缩写 `"Artkal"`，**勿"改回"**。
仓根新增 `NOTICE` 保留 MIT 署名（含上游 GitHub URL + commit hash）+ `palettes/` 旁数据
说明（社区近似 hex / 非官方 / 199 of ~200+）。调色板 schema 保持极简（brand+colors），
不加 source/license 字段。
- 替代方案：抓官方 PDF/JPG（~206–225 色）。否决理由：非机读、厂商版权、出处不干净，
  不适合 Apache-2.0 仓库；近似 hex 对最近色匹配足够。

## 风险 / 权衡

- [hex 非官方、仅 199/~200+ 官方色] → 在 `NOTICE`/数据说明如实注明；最近色匹配容忍
  小偏差；完整官方色表无干净公开源，不为此引入版权风险。
- [`include_bytes!` 硬编码相对路径耦合] → 文件移动会在**编译期**报错（响亮、可接受）；
  spec 固定规范位置 `palettes/artkal_s.json`，M6 CLI 与测试对齐同一路径。
- [`serde_json::Error` Display 文案随版本变化] → 测试只断言变体，不断言 Display 文案；
  自家 `InvalidPalette` 的 reason 可断言。
- [数据源可复现性] → tasks 1.1 引用上游 MIT 源（`maxcleme/beadcolors` 的 GitHub URL +
  commit hash + `raw/artkal_s.csv` 路径），不依赖临时文件；`NOTICE` 引用同一稳定 URL。
- [校验顺序不定导致"第一个错"不确定] → D5 已钉死顺序（空表→逐项 hex→唯一 code），错误
  确定性由此保证（spec 增「错误信息确定性」场景）。

## Migration Plan

无运行时迁移：纯新增能力，无现有生效 spec。部署即随 `bead-core` 编译；回滚为撤销本
变更（删 `palette` 模块、回退两个 `BeadError` 变体与依赖）。

## Open Questions

无——探索阶段决策已全部冻结。
