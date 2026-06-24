## 1. models：ColorStat

- [x] 1.1 扩 `crates/bead-core/src/models/mod.rs`，定义公开 `ColorStat { code: String, name: String, count: u32 }`
  （derive `Debug+Clone+PartialEq`，**不 derive `Eq`**，同 `PixelGrid`/`BeadPattern`，见 design D8）；文档注明
  `count` 是该色在 `BeadPattern.cells` 中的出现次数、最大为 `width*height`
- [x] 1.2 改 `crates/bead-core/src/models/mod.rs:46-47` 的 `BeadPattern` doc-comment（**精确原文**："There is **no
  `stats` field** in M3 — per-color statistics arrive in M4 (design D4)."）→ 改为 "There is **no `stats` field** —
  per-color statistics are a derived artifact (see M4-D1)."；去掉 in-M3 时态、把 "(design D4)" 改指 M4-D1（**注**：D4 是
  越界守卫决策、D1 才是放置决策，旧引用指错决策，须改）（D1；与 5.3/5.4/5.5 同口径）

## 2. statistics 模块：count_colors / total_beads / generate_summary

- [x] 2.1 新建 `crates/bead-core/src/statistics/mod.rs`，定义公开
  `pub fn count_colors(grid: &BeadPattern, palette: &Palette) -> Vec<ColorStat>`：用稠密数组
  `let mut counts = vec![0u32; palette.colors.len()];` 单遍扫 `grid.cells` 计数（**无 `HashMap`/`HashSet`**，延续 M1
  有序 `Vec` 先例，D3）；**只产 count>0 的色**；按 **count 降序、平局取最低下标**排序——按下标序取非零项后做**稳定
  排序** `sort_by(|a,b| b.count.cmp(&a.count))`（稳定 → 等 count 组内保留下标升序，D2；**必须 `sort_by`（稳定），
  禁 `sort_unstable_by`——后者会破坏等 count 平局的最低下标序**，用 `// ponytail:` 注释钉死；并加 `// ponytail:` 注明
  双模式由 app 层对返回 Vec 自排 code 实现、引擎不留代码）
- [x] 2.2 `count_colors` 越界守卫（D4）：计数处用边界检查（`if (idx as usize) < counts.len() { counts[idx as usize] += 1; }`
  或 `counts.get_mut`），越界下标**跳过 per-color 计数但仍计入 total**、**不 panic**；**不加 `debug_assert!`**——它会在
  `cargo test`（debug，`debug_assertions` 默认开）下对测试 4.6 的越界输入 panic，违 spec「不 panic」+ 5.1 的 debug 跑、
  且与全函数契约自相矛盾（D4）；改用一行 `// ponytail:` 注释说明"越界=传错 palette，靠 `Σ count < total` 可观测信号暴露，
  不用会 panic 的断言"
- [x] 2.3 `pub fn total_beads(grid: &BeadPattern) -> u32`——`= grid.cells.len() as u32`（不依赖 palette、不依赖 stats，D5）
- [x] 2.4 `pub fn generate_summary(grid: &BeadPattern, palette: &Palette) -> String`——逐字 INIT「Summary Format」（D6）：
  `Bead Pattern Summary` / `Size: {w} x {h}`（`x` 两侧空格）/ `Total Beads: {total_beads}` / `Palette: {palette.brand}`
  + 单空行 + 每用到色一行 `{code} {name}: {count}`（**带冒号**，D2 序）+ **末尾换行**；内部**复用** `count_colors`
  与 `total_beads`（同源、不分歧）；空网格 → 仅 4 行头（`Total Beads: 0`）+ 空行、无色行、不 panic

## 3. lib.rs：重导出

- [x] 3.1 `crates/bead-core/src/lib.rs`：`pub mod statistics;` + 重导出 `count_colors / total_beads / generate_summary`
  （statistics）与 `ColorStat`（models）；**不新增 `BeadError` 变体**（三原语全 total，D7）

## 4. 测试（映射 spec 需求 + 确定性门）

- [x] 4.1 `count_colors_counts_used_only`——构造已知 `BeadPattern`+`Palette`，断言每用到色的 `count`/`code`/`name` 正确，
  且 `cells` 中未出现的色**不在结果内**（spec「从 BeadPattern 统计每色豆数」）
- [x] 4.2 `sort_count_desc_tiebreak_lowest_index`——构造两个出现次数相同的色（下标 i<j），断言下标 i 排在前；多个不同
  count 断言降序；重复调用一致（**确定性门**，spec「统计排序确定性」/ D2）
- [x] 4.3 `total_beads_cross_check`——断言 `total_beads == cells.len() == width*height == Σ count_colors(...).count`
  （三/四相等交叉校验，spec「total_beads 等于网格豆数」/ D5）；**fixture 必须满足 `cells.len()==width*height` 且传
  *正确* palette**（每个 `cells[i]` 合法），使四相等成立——与 4.6 越界 `Σ < total` 的退化侧区分
- [x] 4.4 `summary_exact_format`——对已知 grid+palette 断言 `generate_summary` 逐字节等于硬编码期望串（含 4 行头、
  `Size: {w} x {h}` 的空格、单空行、每色行**带冒号**、末尾换行、`Palette` == `palette.brand`，spec「generate_summary
  逐字匹配 INIT」/ D6）
- [x] 4.5 `summary_empty_grid`——取**两个**具体实例（覆盖通式 `Size: {width} x {height}` 的两分支）：(a) `width==0,height==5`
  → 断言 summary **逐字节**等于 `"Bead Pattern Summary\nSize: 0 x 5\nTotal Beads: 0\nPalette: {brand}\n\n"`；(b)
  `height==0,width==5` → 逐字节等于 `"Bead Pattern Summary\nSize: 5 x 0\nTotal Beads: 0\nPalette: {brand}\n\n"`（均
  4 行头 + 空行分隔符、无色行、**结尾 `\n\n`**、不 panic）；两实例均 `count_colors` 返 `[]`、`total_beads` 返 `0`（spec
  空网格场景 / D6 精确字节 / D9.2）
- [x] 4.6 `smaller_palette_skips_not_panic`——用比产出 `cells` 时更小的 `Palette`（存在越界下标）调 `count_colors`：
  **debug 与 `--release` 下均不 panic**（因 2.2 不含 `debug_assert!`），越界格不计入任何 `ColorStat`，但仍计入
  `total_beads`（断言 `Σ count < total_beads`）；附一个空 `Palette`（`colors==[]`）子断言：返 `[]`、不 panic（spec「容错越界」
  含空调色板特例 / D4）
- [x] 4.7 `duplicate_rgb_counts_by_index`——调色板含两个 RGB 相同、code 不同的色（下标 i<j）。**(a)** matcher 产出的全命中
  网格 → 只产下标 i 的 `ColorStat`、j 因 count==0 省略；**(b)** *判别性*：另用**手构** `BeadPattern`（`cells==[i, j, i]`，
  直含两下标、绕过 matcher）→ 断言**两个独立** `ColorStat`（i count 2、j count 1），证明按*下标*计数、不按 RGB 合并
  （仅 (a) 不能排除 merge-by-RGB 误实现，须加 (b)）（spec「重复 RGB 按下标分别计数」两场景 / D9.1）
- [x] 4.8 `non_ascii_name_byte_faithful`——某用到色 `name` 含非 ASCII（如 `"碧蓝"`/`"Café"`），断言其在 `generate_summary`
  输出中逐字节原样出现（spec「非 ASCII 色名保真」/ D9.3）
- [x] 4.9 `statistics_is_deterministic` **+ 跨架构整数 golden**——(a) 同 grid+palette 两次 `count_colors`/`generate_summary`
  的结果 `PartialEq`/字符串相等；(b) 固定小 grid（含重复命中 + 等 count 平局）+ 固定小调色板，断言 `Vec<ColorStat> ==
  vec![...]` 与 `summary == "..."` **硬编码**期望（纯整数计数+整数键排序 → 跨 arm64/x86_64 一致，`// ponytail: 整数统计
  跨架构位精确，可硬编码 golden`，spec「统计确定性」/ D9.5）

## 5. 收尾验证 + 文档（D1 全套校正：ARCHITECTURE 三处 + INIT + ROADMAP + models doc-comment + color-matching delta）

- [x] 5.1 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（debug **与** `--release`）全绿
- [x] 5.2 确认 statistics **无新依赖、无 `rayon`、无 `f32`、无 `HashMap`/`HashSet`**（`cargo tree` 仍无 rayon；grep
  statistics 模块无浮点/哈希）；bead-core 仍无 fs/UI/平台依赖
- [x] 5.3 校正 `ARCHITECTURE.md` **三处**（D1）：① Data Model Layer 结构块——删 `BeadPattern.stats: Vec<ColorStat>` 字段
  与"filled from M4"行内注释，改述"统计是派生产物 `count_colors(&BeadPattern,&Palette)`，grid+stats 在 M6 pipeline 层打包"；
  ② 删/改写其后**独立 prose 段**「`stats` is a forward-looking field: it is populated starting in **M4** …」（约 254-256 行；
  **该段不含 `stats` struct 关键字却断言同一前向承诺，极易漏改**）→ "统计由 `count_colors` 从 `cells` 派生，永不存字段"；
  ③ statistics 模块段的 summary 示例 `S01 Black 1240` / `S02 White 980`（约 177-178 行，**无冒号**）→ INIT 口径
  `S01 Black: 1240` / `S02 White: 980`（D6 已声明 ARCH 示意 yields to INIT，须落地）。并确认 Rendering Strategy 段
  （约 280-289 行"M4 statistics count over cells / never from rendered images"）**已与 D1 一致、无需改动**（D1）
- [x] 5.4 校正 `INIT.md` Data Models 块：从 `BeadPattern` 删 `stats` 字段（与 ARCHITECTURE 同口径，D1）
- [x] 5.5 校正 `ROADMAP.md` M4：把「`ColorStat` model; `BeadPattern.stats` populated」改为「`ColorStat` model;
  statistics derived from `BeadPattern`（count_colors/total_beads/generate_summary）」（D1）
- [x] 5.6 收尾确认（**grep 须覆盖 prose + 示例，不止 struct 关键字**——否则漏 5.3② 的 prose 段与 ③ 的无冒号示例）：在
  `ARCHITECTURE.md`/`INIT.md`/`ROADMAP.md` 跑
  `grep -nE "\.stats|filled from M4|populated starting in|forward-looking field|S0[0-9]+ [A-Za-z]+ [0-9]"`——确认**无任何残留**
  断言 `BeadPattern.stats` 字段 / "populated in M4" 前向承诺 / 无冒号 summary 示例；`crates/bead-core/src/models/mod.rs`
  的 `BeadPattern` struct 无 `stats` 字段、doc-comment 已去 in-M3 时态；`openspec/specs/color-matching/spec.md` 经 5.7 delta
  应用后括注已为派生口径（**五处真理源（ARCH 三处 + INIT + ROADMAP + models doc）+ color-matching 规范**对"BeadPattern
  不含 stats、统计为派生"讲同一个故事）
- [x] 5.7 确认本 change 含 `openspec/changes/add-statistics/specs/color-matching/spec.md`（`## 修改需求` delta，重述
  「BeadPattern 输出形状」需求、仅把括注"（统计属 M4）"改为派生口径，规范性"不含 stats 字段"不变、被强化）；归档时该
  delta 应用到 `openspec/specs/color-matching/spec.md`（对应 proposal「修改功能」/ F2 / D1）
