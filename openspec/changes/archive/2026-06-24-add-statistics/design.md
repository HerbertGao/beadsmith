## 上下文

里程碑 M4。M1 建了 `palette`（`Palette { brand, colors: Vec<PaletteColor> }`、`PaletteColor { code, name,
rgb }`、有序 `Vec` 扫描、无 `HashMap`）。M3 建了 `matcher` + `BeadPattern { width, height, cells: Vec<u16> }`
（行优先调色板下标，配色后的真理源；`RgbMatcher` 自持调色板 RGB 保序快照，下标 `i` ≡ `palette.colors[i]`；
`match_pattern` 是 `PixelGrid → BeadPattern` 的唯一交接点）。M3-D4 **刻意不在 `BeadPattern` 放 `stats`**
（空 `stats:[]` 是会撒谎的字段），并把"M4 加统计"写为既定演进。M3-D6 冻结规则：统计**遍历 `BeadPattern.cells`
下标**，绝不碰 `PixelGrid` 原始 RGB、绝不从渲染图反推。

M4 交付三个原语（`count_colors` / `total_beads` / `generate_summary`）+ `ColorStat` 模型 + 逐字匹配 INIT 的
summary 文本。约束不变：纯库、确定性是门（每个排序/格式可 golden 钉死）、纯整数计数、无 `HashMap`/`HashSet`
（M1 先例）、单线程、`thiserror`。

设计经探索阶段拍板（Software Architect 提议 + 主 agent 复核纠正——核心是 D1 选了"派生而非存储"的 B 方案，
把 D4 的越界守卫定为"边界检查跳过 + 可观测 `Σ count < total` 信号"（**不**用会 panic 的 `debug_assert!`，见 D4）、
把 D2 的双模式定为 app 层零成本升级）。

## 目标 / 非目标

**目标：** `statistics` 模块（`count_colors` + `total_beads` + `generate_summary`）；`ColorStat` 模型；
确定性排序（count 降序、平局最低下标）；只列用到的色；逐字 INIT summary 契约；跨架构整数 golden。

**非目标：** `stats` 作为 `BeadPattern` 字段（D1 否，永不）；排序双模式参数（D2，app 层自排）；
`pipeline::generate_pattern` 与 grid+stats 打包类型 `GenerateResult`（M6）；preview/grid 渲染（M5）；
CSV/其它导出（非 MVP）；`rayon`（Phase 2）。

## 决策

**D1 — 统计放置：`BeadPattern` 保持纯净，统计是独立的派生产物（反转早期把 `stats` 画进 `BeadPattern` 的文档草图）。**

不给 `BeadPattern` 加 `stats` 字段，`BeadPattern { width, height, cells }` 与 M3 完全一致。统计由 `statistics`
模块的自由函数按需算：

```rust
pub fn count_colors(grid: &BeadPattern, palette: &Palette) -> Vec<ColorStat>;
```

`ColorStat { code, name, count: u32 }` 放 `models`（同 ARCH/INIT 定义）。grid+stats 的**打包**（供 `pattern.json`
序列化、FFI）发生在 **M6 pipeline 层**的独立结果类型（如 `GenerateResult { pattern, stats, summary }`），
**不**靠改 `BeadPattern`。M4 不建该类型，只交付派生函数。

- **理由**：① CLAUDE 规则 3 是治理法且原话是"derive"非"store"——"`BeadPattern` 是真理源；preview/statistics/
  exports 皆从它*派生*"。`stats` 字段紧挨 `pub cells` = 派生数据与其源并存的经典脱节隐患；`cells` `pub`、任何调用
  方（或 M5/M6）改了 `cells` 而不重算，`stats` 就成谎。派生函数*无法*脱节——每次从 `cells` 现算，构造即正确。
  这正是 M3-D4 拒绝空 `stats` 的同一隐患晚一里程碑——而**陈旧的 `stats` 比空的更危险**（看起来权威）。
  ② M3-D4 的精神（不存会撒谎的派生数据）优先于路线图措辞（"`BeadPattern.stats` populated"）的字面。
  ③ 确定性：无 `stats` 字段时 `BeadPattern` 的 `PartialEq` 与 M7 `pattern.json` golden 只覆盖 `{width,height,
  cells}`（纯整数、跨架构稳）；统计有自己的 `summary.txt` golden——两份窄 golden 胜过一份宽的（排序策略变了
  只炸 summary golden，不扰动 cells golden）。④ 序列化便利的诉求真实但落在 M6：`pattern.json` 是*管线输出文档*，
  由已同时持 pattern 与 palette 的编排层自然组装；数据模型不必背这个包，管线结果类型背即可（M8/FFI 只对接
  `generate_pattern`，不直接对接 `BeadPattern`）。
- 替代方案 (A)：照路线图字面给 `BeadPattern` 加 `stats: Vec<ColorStat>`。否决：三处真理源字面画了该字段、且 M3
  曾就此向 CodeRabbit 辩护——但脱节隐患非假想：`cells` `pub` 意味"`stats` 反映 `cells`"这个不变量**无强制点、无
  构造收口**（不像 `cells.len()==width*height` 至少由 `match_pattern` 产出），一个手改 `cells`+陈旧 `stats` 的
  `BeadPattern` 可构造且静默错——而统计喂的是*买豆清单*，"静默错的计数"正是最伤用户处。「别反转 M3」反而支持 B：
  M3 的*实质*决策（D4）就是"`BeadPattern` 无 `stats`"；被"反转"的只是 ARCH/INIT 那条"字段将在 M4 到来"的*前向
  注释*——反转一条尚未成真的注释很廉价，选 A 才是真反转 M3-D4 的设计原则。**关于 CodeRabbit 辩护**：M3 辩护的是
  *在文档保留注释*（"是的，stats M4 来"）、**并未 ship 字段**；选 B 不与该辩护冲突，是解决注释推迟的"怎么来"。
- 替代方案 (C)：`stats: Option<Vec<ColorStat>>`。否决：以新装束重引 M3-D4 的"撒谎字段"（现在谎是 `Some(stale)`），
  每个读者多一道 `match`，仍紧邻 `pub cells` 无脱节守卫。"算没算"的干净答案是"调函数"，不是"存可空派生态"。
- **文档影响（载重）**：本 change **必须**校正 `ARCHITECTURE.md`（Data Model Layer：从 `BeadPattern` 删 `stats`
  字段与"filled from M4"注释，改述"统计是派生产物 `count_colors`、M6 层打包"）、`INIT.md`（Data Models 同步）、
  `ROADMAP.md` M4（"`ColorStat` model; `BeadPattern.stats` populated" → "`ColorStat` model; statistics derived
  from `BeadPattern`"）、`models/mod.rs` 的 `BeadPattern` doc-comment（"no `stats` field in M3 (D4)" → "no
  `stats` field — statistics are derived, see M4-D1"，去 in-M3 时态）。`ColorStat` 结构本身不变。
  **校正点不止笼统的"四处"——逐处列死（见 tasks 5.3–5.7）**：`ARCHITECTURE.md` 实含**三处**——① 结构块 `stats` 字段、
  ② 其后独立 prose 段「`stats` is a forward-looking field … populated starting in **M4**」（不含 `stats` token、易漏）、
  ③ statistics 段无冒号 summary 示例 `S01 Black 1240`（D6 已声明 yields to INIT，须落地为带冒号）；外加 `INIT.md`、
  `ROADMAP.md`、`models/mod.rs` doc-comment；**再加** `color-matching` 规范「BeadPattern 输出形状」需求的括注
  「（统计属 M4）」——它原读作"stats 字段将在 M4 到来"，D1 反转后会误导，须经本 change 的 `## 修改需求` delta
  （`specs/color-matching/spec.md`）修正为派生口径（见 proposal「修改功能」）。`ARCHITECTURE.md` 的 Rendering Strategy 段
  （"M4 statistics count over cells / never from rendered images"）**已与 D1 一致、无需改**。

**D2 — `ColorStat` 排序：count 降序，平局取最低下标（确定性门，同 M3 最低下标规则 golden 钉死）。**

`count_colors` 返回的 `Vec<ColorStat>` 按 ①`count` 降序、②调色板下标升序（M3 声明序锚点）平局。实现 = **对按下标序
构建的计数做稳定排序**：先按下标 `0..n` 算 per-index 计数，再 `sort_by(|a,b| b.count.cmp(&a.count))`（**稳定排序**
保留等 count 组内的下标升序），天然得到 count 降序-then-下标升序，无需显式次键。

- 理由：① INIT 例子（S01 1240/S02 980/S13 760/S45 520）同时满足 count 降序*与* code 升序，**不能判别**，故按意图+先例
  定再 golden 钉死。② summary 的用途定 count 降序——这是可复制的购物/拼装辅助（INIT:"directly copyable"），用户要
  "最需要哪些色"在最上；纯下标升序会把主色埋在它在 JSON 里碰巧的位置。③ 平局必须复用 M3 锚点而非另立：M1（order
  matches JSON）/M2（裁剪偏移靠位置）/M3-D3（平局取最低下标）都把确定性锚到*声明序=下标升序*；等 count 平局按下标
  升序——同一锚点，全引擎*一条*确定性规则。用 *code 升序*（字符串比较）平局会是第二条独立规则 + locale 坑；下标升序
  是整数、跨架构稳、且已是房屋锚点。
- 替代方案：纯下标升序（M1/M2/M3"靠位置"的最纯延续，且不用排序）。否决：败坏 summary 用途（主色不在前），且与
  INIT 例子"最大在前"的隐读相悖。下标升序留在它该在的地方——*平局子序*——确定性不丢、有用性不弃。
- **双模式（ponytail 升级路径，不写进 M4 代码）**：M4 只产这一个规范序、不加任何 sort-mode 参数/配置。App（M9）
  要"按 code 排"那个模式：消费方直接对返回的 `Vec<ColorStat>` 调 `.sort_by(|a,b| a.code.cmp(&b.code))`——
  ColorStat 自带 `code`，**引擎零改动**。用 `ponytail:` 注释在代码记此升级路径。
- 确定性注记：count `u32`、下标 `usize`——整个排序键是整数 → 跨架构位精确、可 golden 钉死。用一个含等 count 平局
  的单元测试断言其下标序（同 M3 专门的平局测试）+ 一份固定 `Vec<ColorStat>` golden 钉住。

**D3 — `count_colors` 返回语义：只列用到的色（count>0）；summary 列出正是该集合。**

每个 count>0 的调色板色返回一个 `ColorStat`，count==0 的略去。计数用 `let mut counts = vec![0u32;
palette.colors.len()];` 单遍 `for &idx in &grid.cells { /* 见 D4 守卫 */ counts[idx] += 1; }`，再按下标序取非零项、
最后套 D2 稳定排序。

- 理由：① INIT summary 只列 4 行（Artkal S 有 199 色），契约是"本图案用到的色"非"整本目录配一墙 `: 0`"。② count>0 是
  自然诚实集——统计描述*图案*，零豆色不属图案。③ total 仍对全网格校验（D5），略零无损完整性（**调色板前置条件成立时**
  Σ 用到色 count 仍 == width×height，零色贡献 0；前置条件被违反时见 D4 退化侧 `Σ count < total`）。
- **计数结构（确定性、无 HashMap）**：稠密 index-keyed `Vec`（非 hash map），迭代序天生确定、计数纯整数——M1"有序
  `Vec` 扫描、无 `HashMap`"先例用到计数上。
- 替代方案：返回全调色板（含 count==0）。否决（对 summary）：噪声、悖 INIT 例子。将来真有消费方要全量（如库存视图，
  INIT 的 Future Feature）那是*另一个*函数或*后加*的 flag（非破坏，M4 YAGNI），不为投机读者现在付噪声成本。

**D4 — palette 一致性契约 + 越界守卫：全函数 + 文档化前置条件，越界下标确定性跳过、不 panic（非静默 UB）。**

这是 M3"matcher 持 palette 快照；index→color 要同一份未改 `Palette`"隐患的 M4 化身——统计是该不变量的*第二个*消费者。

- `count_colors(grid, palette)` 是**全函数**（返回 `Vec<ColorStat>`、**无 `Result`**）+ **文档化前置条件**：每个
  `cells[i]` 必须是 `palette.colors` 的合法下标——即此处传的 `palette` 必须是产出 `grid` 的 matcher 用的*同一份
  未改动* palette。镜像 `match_pattern` 的"调用方拥有 `PixelGrid` 不变量"（M3-D5）：管线内自然调用即满足，违反是
  调用方契约违约。
- **前置条件的两种违反 + 信号边界（自觉接受的 scope 边界）**：违反"同一份未改动 palette"有两类。① **更小/越界
  palette**（`max(cells) >= palette.colors.len()`）→ 越界跳过 → `Σ count < total` 是*可观测信号*。② **等长但内容不同的
  palette**（`colors.len()` 相同、`{code,name}` 被换）→ 无越界、无信号，`count_colors` 静默输出*错的* code/name。**②
  在 M4 无信号是有意接受的**：它是"调用方持同一份 palette"前置条件的固有部分（同 `match_pattern` 的同-palette 不变量，
  房屋风格），且*连 M6 的 `max(cells)` vs `len()` 校验也抓不到*等长替换——彻底诊断需 M6 持单一 `Palette` 同喂 matcher/
  统计/渲染（见下"管线不变量"），非 per-cell 计数能承担。M4 文档化此边界、不为它穿 `Result`（D7）。
- **但不同于 `match_pattern`，此处越界 `cells[i]` 会索引 `Vec` → panic**。core 不在可达输入上 panic（M3-D7/M2-D5′）。
  传比配色时更小的 palette 是可达的。**决策：边界检查访问（`counts.get_mut(idx)` / `if idx < counts.len()`），
  越界下标跳过该格的 per-color 计数、但仍计入 `total_beads`**（D5 独立 = `cells.len()`，故 total 仍对）。结果是对
  契约违约输入给出确定性、不 panic 的输出（退化时 Σ 用到色 count ≤ total），而 per-color/total 不一致本身就是"传错
  palette"的可见信号。**不加 `debug_assert!`**：早期草案曾设想 `debug_assert!(idx < counts.len())` 作开发期信号，但
  `cargo test`（debug 档，`debug_assertions` 默认开、本仓无 `[profile.*]` 覆盖）下它会对*正是* spec「越界不 panic」
  要求、且测试 4.6 在 debug 跑的那一格输入触发 panic——与「全函数/不 panic」契约及 4.6/5.1 自相矛盾（会 panic 的断言
  违背"core 在可达输入上不 panic"，而更小 palette 是可达的）。改为：仅靠边界检查跳过 + `Σ count < total_beads` 这一
  可观测信号暴露"传错 palette"，误用提示放 `// ponytail:` 代码注释、不放任何会 panic 的断言（core 亦不打印/不日志）。
- 理由：① 全函数+前置条件匹配房屋风格（`match_pattern`/`find_best_match` 热路径），优于给"正确调用永不失败"的计数
  原语穿 `Result`。② 与 `match_pattern` 的差别强制显式守卫：`match_pattern` 能全函数且朴素是因任意 `[u8;3]` 都是
  `find_best_match` 合法输入；这里按调用方值索引 `Vec`，"全函数"必须配非 panic 访问。③ 跳过-仍计 total 优于诸替代：
  无条件保 `total_beads==cells.len()` 不变量、保持全函数、让损坏*可观测*（Σ<total）而非崩溃或伪造错 total。
- 替代方案 (a)：裸 `counts[idx]`、靠前置条件。否决：可达的传错-palette 会 panic——违"core 不在可达输入 panic"。
- 替代方案 (b)：返 `Result`、任一越界即错。否决（主要）：把每个正确调用永不失败的原语污染成 fallible、给所有管线内
  调用方加 `?`，且与兄弟原语的全函数框架冲突。*留作 M6 选项*：若 M6 要硬"传错 palette"诊断，那属 `pipeline::
  generate_pattern` 的校验（一次比较 `max(cells)` vs `palette.colors.len()`），非 per-cell 计数循环。
- 替代方案 (c)：越界下标饱和到末色。否决：给真实色伪造一个*貌似合理的错* count——买豆清单最坏结局（买错色）。跳过
  诚实，饱和撒谎。
- **管线不变量（前向，同 M3 风险注记）**：M6 `pipeline::generate_pattern` 须持**单一 `Palette`**，把*同一*引用喂给
  ① matcher（`RgbMatcher::new`）② `count_colors` ③ M5 渲染。matcher 的 index→color、统计的 index→{code,name}、
  渲染的 index→rgb 都锚到这一份 palette。写进 risk 段，M6 落地时强制。

**D5 — `total_beads`：`-> u32`，定义为 `cells.len() as u32`；不变量 `total == width×height == Σcount`（调色板前置条件成立时）由构造成立。**

```rust
pub fn total_beads(grid: &BeadPattern) -> u32;   // = grid.cells.len() as u32
```

只取 `&BeadPattern`（数豆不需颜色，无需 palette）。

- 理由：① `cells.len()` 是真理源、Σstats 是派生的派生。路线图 done-when（"totals equal width × height"）与不变量
  链 `total == width×height == Σcount`（**前置条件：每个 `cells[i]` 对所传 palette 合法**）应**锚在 `cells`**、其余两条
  *对它校验*。把 `total_beads` 定义为 `cells.len()` 使它独立于 `count_colors`——于是一个断言
  `total_beads(p) == Σ count_colors(p,pal).count`（**palette 合法时**）是*真交叉校验*（两条独立计算相符），非 tautology；
  若 `total_beads` 是 Σstats 则该断言空真、抓不到计数 bug。（palette 越界时该相等不成立、退化为 `Σ < total`，见 D4。）② 用 `cells.len()` 而非
  `width*height`：前置条件下相等，但 `cells.len()` 是*物理存在*的数、无需 `usize` 乘/溢出推理，且对退化 `width==0`
  正确（`len()==0`）；断言 `total_beads==width*height` 遂成对布局不变量的另一条真校验。③ `u32` 宽度对：最大=
  width×height（INIT 基准上限 300×300=90000）；与 `ColorStat.count: u32`、`width/height: u32` 一致，整个统计面
  `u32` 齐整。**算作 `cells.len() as u32`**。**前置条件 `cells.len() <= u32::MAX`**：`cells` 字段 `pub`、可被手构
  破坏，`as u32` 在超限时*静默截断*；故与 `models/mod.rs` 对 `cells.len()==width*height` 的"调用方持有不变量"同口径，
  列为前置条件而非加 `Result`/`try_into`（后者属 scope creep、悖 D7 全函数框架）。（将来真有超 `u32` 的巨网格模式，是
  `count`+`total_beads` 协同加宽——此处不解、Phase-1 豆板（INIT 上限 300×300=90000）远不可达。）
- 替代方案：`total_beads = Σcount`。否决：使 total 依赖 stats 计算（耦合两原语、要个不需要的 `&Palette`），且把不变量
  交叉校验变 tautology。total 必须直接来自网格，三相等才有两条独立腿可比。

**D6 — `generate_summary`：签名、逐字 INIT 格式、末尾换行、brand 取自 `&Palette`、复用 `count_colors` 排序。**

```rust
pub fn generate_summary(grid: &BeadPattern, palette: &Palette) -> String;
```

**逐字**产出 INIT Summary Format：
```text
Bead Pattern Summary
Size: {width} x {height}
Total Beads: {total}
Palette: {brand}
                       ← 单空行
{code} {name}: {count}   ← 每用到色一行，D2 序
...
```
- 第 2 行 `Size: {w} x {h}`——`x` 两侧各一空格（同 INIT `80 x 100`）。第 3 行 `Total Beads: {n}`，`n=total_beads`
  （D5）。第 4 行 `Palette: {brand}`——取 `palette.brand` 原样（**已查证** `palettes/artkal_s.json` 的 `brand=="Artkal S"`，
  与 INIT `Palette: Artkal S` 完全一致；引擎不拼接 `" S"`）。然后单空行（`\n\n`）。
- 每色行格式 = `code 空格 name 冒号空格 count`（INIT `S01 Black: 1240`），**带冒号**——区别于 ARCH statistics 段草图
  `S01 Black 1240`（无冒号）：**INIT 为准**（它标"directly copyable"是契约），ARCH 草图是示意、yields to it。
- **末尾换行：是**——最后一色行以 `\n` 收尾。输出是 POSIX 文本（`summary.txt`，M6）+ 可复制块；末尾换行是常规、diff
  干净、避免 golden 里"no newline at end of file"噪声。golden 显式钉住、不让漂。
- **空网格（连 D9）**：`width==0`/`height==0`（无 cells）→ `count_colors` 返 `[]`、summary = 4 行头+空行+无色行、
  `Total Beads: 0`。**精确字节**（**两个**触发分支：`width==0`（任意 height）或 `height==0`（任意 width），故第 2 行用
  通式不写死 0）：以 4 行头 + 那个空行分隔符收尾，即
  `"Bead Pattern Summary\nSize: {width} x {height}\nTotal Beads: 0\nPalette: {brand}\n\n"`（如 `width==0,height==5` →
  `Size: 0 x 5`；`height==0,width==5` → `Size: 5 x 0`；结尾 `\n\n` = 第 4 行换行 + 空行换行；空行分隔符**无条件**保留、
  **不**因无色行而省——否则"末尾换行作用于最后产出行"会有两种合法解读、golden 二义）。每分支专门 golden 钉死该串。
- **summary 不做 `Σ==total` 交叉校验**：`Total Beads` 行取 `total_beads`（全网格），各色行取 `count_colors`（越界跳过）。
  传错/更小 palette 时二者会不一致（`Total Beads: N` 而色行之和 `< N`）——这是 D4 的可观测信号，但 `generate_summary`
  **只忠实渲染、不主动检测或标红**该退化。"传错 palette"诊断属 M6 `generate_pattern` 的校验，非 M4（避免误以为该信号是
  用户可见告警）。
- **逐字格式的前置条件（palette 字段无断行符）**：「4 行头 + 每用到色一行」的精确结构假设 `brand`/`code`/`name` 不含
  `\n`/`\r` 等断行控制符（`generate_summary` 原样写入、不转义/不过滤，同 D9.3 字节保真）。M1 `validate_palette` 当前不拒
  断行符，故这是 `generate_summary` 的**文档化前置条件**（同 D4 的 palette 前置条件口径）。在 `validate_palette` 增加
  "拒断行控制符"是 *palette 能力*的独立改动（修改 M1 已生效规范）→ **越界本 change 范围，留作后续 change**，M4 不实现。
- 理由：格式是复制粘贴契约 → 须逐字节精确并 golden 钉（含冒号、`x` 两侧空格、单空行、末尾换行）。`generate_summary`
  内部**复用** `count_colors`（同一排序/used 集）与 `total_beads`（同一 total）→ summary 与结构化 stats 永不打架。
  brand 取自 `&Palette` 保持 core 纯（不硬编码 brand），summary 是 (grid, palette) 的纯函数。
- 替代方案：不要末尾换行。否决：goldens/diff 里出"No newline at end of file"、且是较不常规的文本形；钉末尾换行只一字符。
- 替代方案：从另传的 `Vec<ColorStat>` 构建（`generate_summary(grid, &stats)`）。否决（对 M4 独立原语）：让调用方
  传*不匹配*的 stats（重引脱节）；自派生保持自洽。（M6 可一次算 stats 复用——那是这些全函数之上的 M6 优化，非 M4 签名。）

**D7 — 错误模型：三个函数全 total（`-> Vec<ColorStat>` / `-> u32` / `-> String`），无 `Result`，零新增 `BeadError` 变体。**

- 理由：① 正确（管线内）调用时计数/求和/格式化皆全运算、不会失败——按 M3 框架（`lib.rs`:"total APIs 直接返值；
  fallible 返 `Result`"）它们是 total API。② 唯一可达失败模式（传错/更小 palette）由 D4 跳过守卫确定性处理、非错误类。
  ③ 复用-不-加（M3-D7）：即便要信号"传错 palette"，`InvalidPalette { reason }` 语义已覆盖，且 `BeadError`
  `#[non_exhaustive]`，将来 M6 真要 pipeline 级诊断再非破坏地加。M4 不加。
- 替代方案：`count_colors -> Result` + 新 `PaletteMismatch` 变体。否决：见 D4(b)，fallible 属 M6 校验边界、新变体是
  过早表面扩张；`#[non_exhaustive]` 免费留门。

**D8 — 模块 / 可见性：新 `statistics` 模块；`pub` 三原语 + `ColorStat`；pipeline（M6）仍是唯一外部入口。**

- 新 `crates/bead-core/src/statistics/mod.rs`（对齐 ARCH 模块清单）。
- `ColorStat` 加进 `models/mod.rs`（同 `PixelGrid`/`BeadPattern`），`#[derive(Debug, Clone, PartialEq)]`——
  **`PartialEq` 非 `Eq`**，同 M3-D1（golden 比较用 `assert_eq!` 够，`Eq` 是公开承诺、YAGNI，真有哈希键需求再加）。
- `lib.rs` 重导出：
  ```rust
  pub use models::{BeadPattern, ColorStat, PixelGrid};
  pub use statistics::{count_colors, generate_summary, total_beads};
  ```
- 这三个是**库内/pipeline 复用原语、非 FFI 入口**（同 M3-D5）；`pipeline::generate_pattern`（M6）才是唯一外部/FFI
  入口、内部调用并打包它们。`statistics` 只依赖 `models`+`palette`（不依赖 `image`/`matcher`）——依赖方向干净
  （statistics 是真理源模型的叶子消费者）。
- 替代方案：把 `count_colors` 等做成 `BeadPattern` 的方法（`pattern.count_colors(&palette)`）。否决：给模型类型塞
  palette 相关行为、糊掉"模型是纯数据、统计是派生"这条 D1 立足的线。自由函数 + 专门模块保持真理源模型瘦、派生显式分离。

**D9 — M4 须钉死的边界（确定性门）。**

1. **重复 RGB / 重复 name、code 不同**：`validate_palette` 只保唯一 *code*、不保唯一 RGB/name。两个同 RGB 色得*不同*
   下标；M3 matcher 把精确命中送*最低*下标——故高下标的重复色可能 **count==0**（被 D3 略去）而低下标累计命中。
   `count_colors` 严格按*下标*计数（每调色板项一个 `counts[]` 槽），两个同 RGB 色是按各自 `{code,name}` 的两个独立
   候选——不合并、不歧义。钉**两个**测试：**(a)** palette 含同 RGB `("A",rgb)`,`("B",rgb)`；全 `rgb` 的 matcher 产出
   网格 → 单个 `A` 的 `ColorStat`（最低下标）带全 count、**无** `B` 行（M3 平局交互）。**(b)** *判别性*用例——
   **(a) 单独不能区分"按下标计数"与"按 RGB 合并后贴最低下标"**（两实现都只产 `A` 行）。故另钉一个**手构** `BeadPattern`，
   `cells` 直含*两个*下标（如 `[i, j, i]`，绕过 matcher）→ 断言**两个独立** `ColorStat`（`i` count 2、`j` count 1），
   证明严格按*下标*计数、绝不按 RGB 合并。把 D2/D3 与 M3 平局的交互显式 golden 钉。
2. **空网格（`width==0`/`height==0`、`cells==[]`）**：`count_colors`→`[]`；`total_beads`→`0`；`generate_summary`→
   `Size: {width} x {height}`（通式，两数原样——`width==0` 则首数 0，`height==0` 则次数 0；见 D6 精确字节）、
   `Total Beads: 0`、无色行。无 panic、无除法、无特例分支（空 `cells` ⇒ 空计数环 ⇒ 全零 counts ⇒
   无用到色）。专门 golden 钉（M3-D5 已立退化网格合法）。
3. **非 ASCII / 多字节色名**：name 是 `String`（UTF-8）。`generate_summary` 原样写入行；Rust `String` 格式化逐字节
   忠实，`"碧蓝"`/`"Café"` 进 `summary.txt` 不变且确定。无截断、无归一、无宽度假设（summary 无列对齐——是空格分隔的
   `code name: count`）。钉测试断言非 ASCII 名逐字节出现。（M9 真实调色板可能有非英文名，"可复制"契约须保真。）
   注："directly copyable"是*字节流*契约、非视觉列对齐；未来若 M9 展示层要列对齐（CJK 等宽），那属消费方，**不得改引擎
   summary 的字节**（否则破坏 golden 与"逐字"契约）。
4. **`count` 宽度 vs 网格 total**：per-color `count` 与 `total_beads` 都 `u32`；每 count ≤ total ≤ `cells.len()`，
   现实上限远低于 `u32::MAX`，故任一 per-color count 在 total 溢出前不会溢出——单一上限一致（仅前向注记）。
5. **确定性双层（同 M3-D8）**：(a) 同 `(grid, palette)` 两次 ⇒ `PartialEq` 相等的 `Vec<ColorStat>` + 逐字节相同
   summary；(b) 硬编码跨架构 golden（`Vec<ColorStat>` + 精确 `summary.txt` 字符串）。纯整数计数 + 整数键排序 ⇒
   跨架构位精确，可硬编码（同 M3 cells golden 的信心，不像 M2 `f32` Lanczos）。

## 风险 / 权衡

- [D1 反转早期 `BeadPattern.stats` 文档草图] → 本 change 内同步校正：`ARCHITECTURE.md` **三处**（结构 `stats` 字段 +
  「forward-looking field … populated starting in M4」prose 段 + 无冒号 summary 示例）、`INIT.md`、`ROADMAP.md`、
  `models/mod.rs` doc-comment，**并**为 `color-matching` 规范「BeadPattern 输出形状」需求加 `## 修改需求` delta 修正其
  「（统计属 M4）」括注——**校正点不止早期笼统所说的"四处"**（prose 段不含 `stats` token、易被 grep 漏，特别钉 tasks 5.6）。
  是*自觉的、经 review 的*反转（M3 只辩护保留注释、未 ship 字段），在 proposal「变更内容」+「修改功能」显式写明，免得后人
  以为 M3/M4 偶然打架。
- [统计依赖"同一份未改 palette"（D4）] → cells 是 `u16` 下标，出 code/name 必须传产出 grid 的 matcher 用的同一份
  palette；M4 文档化为前置条件 + 越界跳过守卫（不 panic），M6 pipeline 持单一 Palette 同喂 matcher/统计/渲染——
  M6 落地时作管线不变量强制。
- [排序是确定性门（D2）] → 误用不稳定排序或 `<=`/`>=` 会让等 count 平局漂；用稳定排序 over 下标序 + 专门的等 count
  平局测试钉"最低下标"。golden 之外必须有的单元锚点。
- [`u32` count/total 上限（D5/D9.4）] → 现实豆板远不及；真有巨网格模式是 count+total 协同加宽（前向，非本变更）。
- [越界守卫"跳过-仍计 total"（D4）] → 是对契约违约输入的诚实退化（Σ<total 即信号）；必须 golden 钉（一个故意更小的
  palette 断言不 panic + Σ<total），防后续重构静默退回裸索引 panic。

## Migration Plan

无运行时迁移：新增能力 `statistics` + 新增 `ColorStat`，**不改** palette/image-grid 的需求；对 color-matching 仅有一处
**非破坏**的措辞修正——经 `## 修改需求` delta 把「BeadPattern 输出形状」需求的括注「（统计属 M4）」改为派生口径（规范性
要求「不含 stats 字段」不变、被强化，见「修改功能」/ D1）。随本 change 提交的文档校正（D1，逐处见 tasks 5.3–5.7）：
`ARCHITECTURE.md` 三处 + `INIT.md` + `ROADMAP.md` + `models/mod.rs` doc-comment + color-matching delta。回滚 = 撤销本变更
（删 `statistics` 模块、回退 `ColorStat`、回退上述文档校正与 delta）。M6 的 `GenerateResult`（grid+stats 打包）是既定
演进（非本变更）。

## Open Questions

- **M6 结果类型形状**（grid+stats 怎么打包、`summary` 存还是重算）= M6 决策；M4 只承诺"不放 `BeadPattern` 字段"。
- **brand 字符串**：已查证 `palettes/artkal_s.json` 的 `brand=="Artkal S"`，与 INIT 一致——无 open 项。
