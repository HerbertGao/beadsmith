# statistics 规范

## 目的
定义从 `BeadPattern`（配色后真理源）按需派生的每色统计：`count_colors` 逐格按调色板下标计数、只列用到的色、按
count 降序-最低下标平局排序；`total_beads` 取网格豆总数；`generate_summary` 产出逐字匹配 INIT「Summary Format」的
可复制文本。统计是派生产物，`ColorStat` 永不作为 `BeadPattern` 字段。纯整数计数（无 `HashMap`/`f32`/`rayon`）→ 同
输入逐字节相同、跨架构位精确、可 golden 钉死；越界下标确定性跳过、不 panic。纯库行为，无文件系统/UI/平台依赖。
## 需求
### 需求:从 BeadPattern 统计每色豆数
`count_colors` 必须接受一个 `BeadPattern` 与一个 `&Palette`，遍历 `BeadPattern.cells`（调色板下标）逐格计数，
产出 `Vec<ColorStat>`。计数**必须只来自 `cells` 下标**，禁止读取 `PixelGrid` 原始 RGB、禁止从渲染图反推、
禁止读取文件系统。每个 `ColorStat` 的 `code`/`name` 来自 `palette.colors[下标]`。结果**只包含用到的色**
（`count > 0`）；在 `cells` 中出现 0 次的调色板色必须省略。计数必须用整数（豆数，非像素色），禁止浮点。

#### 场景:逐格按下标计数且只列用到的色
- **当** 对一个 `BeadPattern` 调用 `count_colors(&grid, &palette)`
- **那么** 返回的 `Vec<ColorStat>` 中，每个出现过的下标对应一项，其 `count` 等于该下标在 `cells` 中的出现次数、
  `code`/`name` 取自 `palette.colors[下标]`；`cells` 中未出现的调色板色不在结果内

### 需求:统计排序确定性（count 降序、平局取最低下标）
`count_colors` 返回的 `Vec<ColorStat>` 必须按 **`count` 降序**排序；`count` 相等时必须按**调色板下标升序**
（即遍历中较低的下标在前）平局。该顺序必须固定且确定，禁止依赖哈希或迭代顺序的偶然性（计数禁止使用
`HashMap`/`HashSet`，须用按下标的有序结构）。排序键全为整数（`count: u32`、下标），因此跨架构必须一致。

#### 场景:等 count 平局取最低下标且重复一致
- **当** 两个调色板色在 `cells` 中出现次数相同（如各出现一次）
- **那么** `count_colors` 必须把**下标较小**者排在前，且对同一输入重复调用结果完全一致

### 需求:total_beads 等于网格豆数
`total_beads` 必须接受一个 `BeadPattern` 并返回豆总数 `u32`，其值**等于 `cells` 的长度**（`cells.len() as u32`），
不依赖调色板、不依赖统计结果。**在调色板前置条件成立时**（每个 `cells[i]` 都是 `palette.colors` 的合法下标，见
「统计对非法调色板输入容错」需求），该值必须同时满足 `total_beads == width*height` 与
`total_beads == Σ count_colors(...).count`（每项 count 之和）——这三者相等是可测试的交叉校验；**前置条件被违反时**
（传入更小调色板，存在越界下标）则 `Σ count < total_beads`（越界格跳过 per-color 计数但仍计入 total），二者**不矛盾**、
是同一契约的正常侧与退化侧。长度运算用 `usize`；`total_beads` 以 `cells.len() <= u32::MAX` 为前置条件（可达豆板远低于
此；`as u32` 在超限时会截断——与 `models/mod.rs` 对 `cells.len()==width*height` 的"调用方持有不变量"同口径，不引入
`Result`/`try_into`）。

#### 场景:total 等于 width×height 等于各色计数之和
- **当** 对一个满足 `cells.len()==width*height` 的 `BeadPattern` 调用 `total_beads`
- **那么** 返回值等于 `cells.len()`、等于 `width*height`、且等于 `count_colors(&grid,&palette)` 各项 `count` 之和

### 需求:generate_summary 逐字匹配 INIT Summary Format
`generate_summary` 必须接受一个 `BeadPattern` 与一个 `&Palette`，返回一段**可直接复制**的 `String`，逐字匹配
INIT「Summary Format」：第 1 行 `Bead Pattern Summary`；第 2 行 `Size: {width} x {height}`（`x` 两侧各一空格）；
第 3 行 `Total Beads: {total}`（取 `total_beads`）；第 4 行 `Palette: {brand}`（取 `palette.brand` 原样）；
随后一个空行；随后每**用到的色**一行 `{code} {name}: {count}`（`code` 空格 `name` 冒号空格 `count`），顺序与
`count_colors` 相同（count 降序、平局最低下标）。末尾必须以换行符结束。summary 必须与 `count_colors`/`total_beads`
同源（内部复用），禁止与结构化统计产生分歧；禁止读取文件系统。

#### 场景:产出精确的可复制 summary 文本
- **当** 对一个 `BeadPattern` + `Palette` 调用 `generate_summary`
- **那么** 返回字符串逐字节等于 4 行头 + 空行 + 各用到色一行 `{code} {name}: {count}`（带冒号、按 count 降序-
  最低下标平局序）+ 末尾换行；其中 `Total Beads` 等于 `total_beads`、`Palette` 等于 `palette.brand`

#### 场景:空网格的 summary
- **当** 对一个 `width==0` 或 `height==0`（`cells` 为空）的 `BeadPattern` 调用 `generate_summary`
- **那么** 返回 4 行头（`Total Beads: 0`）+ 那个空行分隔符、无任何色行，且不 panic；逐字节为
  `"Bead Pattern Summary\nSize: {width} x {height}\nTotal Beads: 0\nPalette: {brand}\n\n"`（第 2 行两数**原样代入**——
  `width==0` 则首数 0、`height==0` 则次数 0，不写死任一为 0；结尾 `\n\n`：空行分隔符无条件保留）

### 需求:BeadPattern 保持纯净，统计为派生产物
`BeadPattern` **必须**只持 `{ width, height, cells }`，**禁止**含 `stats` 字段——统计是从 `cells` **按需派生**的
独立产物（`count_colors`/`total_beads`/`generate_summary`），绝不作为 `BeadPattern` 的存储字段（避免与 `cells`
脱节的撒谎数据）。下游（M5 渲染、M6 pipeline、导出）必须从 `BeadPattern` 派生统计，grid 与 stats 的打包由 M6
pipeline 层的结果类型承担，而非 `BeadPattern`。

#### 场景:统计来自 cells 而非存储字段或原始像素
- **当** 需要每色豆数
- **那么** 必须调用 `count_colors`（从 `BeadPattern.cells` 现算），不得读取 `BeadPattern` 的某个 `stats` 字段
  （该字段不存在），也不得遍历 `PixelGrid` 原始 RGB

### 需求:统计对非法调色板输入容错（越界下标不 panic）
`count_colors` 必须是全函数（返回 `Vec<ColorStat>`，**不返回 `Result`**），并有文档化前置条件：每个 `cells[i]`
必须是 `palette.colors` 的合法下标，且 `palette` 必须是产出该 `BeadPattern` 的匹配器所用的**同一份未改动**调色板。
当违反前置条件（如传入比配色时更小的调色板，使某 `cells[i]` 越界）时，`count_colors` **禁止 panic**：必须跳过越界
那一格的 per-color 计数，但该格仍计入 `total_beads`（故 `Σ count < total_beads` 成为"传错调色板"的可见信号）。
matcher 禁止 panic 的契约延伸至此。空调色板（`colors.len()==0`）是该越界情形的退化特例：每格皆越界、全部跳过 →
返回 `[]`、不 panic、`total_beads` 仍为 `cells.len()`。

#### 场景:传入更小调色板时越界跳过而非 panic
- **当** 用一个比产出 `cells` 时更小的 `Palette`（存在 `cells[i]` 越界）调用 `count_colors`
- **那么** 不 panic，越界格不计入任何 `ColorStat` 的 `count`，但仍计入 `total_beads`（结果满足 `Σ count < total_beads`）

### 需求:ColorStat 输出形状
`ColorStat` 必须含 `code: String`、`name: String`、`count: u32`，derive `Debug + Clone + PartialEq + Serialize`
（`Serialize` 由 M6/add-cli-pipeline 追加，使 `ColorStat` 可直接进入 `pattern.json` 的 `stats`——序列化真相源本身、不另立会漂移的 DTO，见 M6-D5），
**不 derive `Eq`**（与 `PixelGrid`/`BeadPattern` 一致；`assert_eq!`/golden 比较只需 `PartialEq`）。**不 derive `Deserialize`**（M6 只写不读；
未来「读回 pattern」时再非破坏地追加）。`count` 用 `u32`（最大为 `width*height`）。

#### 场景:ColorStat 携带 code、name 与整数 count
- **当** `count_colors` 为某用到的色产出一个 `ColorStat`
- **那么** 它的 `code`/`name` 等于 `palette.colors[该下标]` 的对应字段，`count`（`u32`）等于该下标在 `cells` 中的出现次数

#### 场景:ColorStat 可序列化进 pattern.json 的 stats
- **当** 把一个 `ColorStat` 用 `serde_json` 序列化
- **那么** 产出含 `code`、`name`、`count` 三字段的 JSON 对象，可作为 `pattern.json` 中 `stats` 数组的元素；且序列化是确定性的（同值同字节）

### 需求:重复 RGB 的调色板色按下标分别计数
`count_colors` **必须**按调色板下标分别计数、**禁止**按 RGB 合并：当调色板含多个 RGB 相同但 `code` 不同的色时
（`validate_palette` 只保证 `code` 唯一、不保证 RGB 唯一），每个下标各自独立计数。由于匹配器把精确命中送往最低
下标，较高下标的重复色可能 `count==0` 而被省略（只列用到的色）。

#### 场景:同 RGB 双色只有最低下标累计命中
- **当** 调色板含两个 RGB 完全相同、`code` 不同的色（下标 i < j），且某 `BeadPattern` 的所有格都命中该 RGB
- **那么** `count_colors` 只为下标 `i` 产出一个 `ColorStat`（带全部命中数），下标 `j` 因 `count==0` 被省略

#### 场景:cells 直含两个重复下标时分别计数（不按 RGB 合并）
- **当** 一个手构 `BeadPattern` 的 `cells` 同时含下标 `i` 与 `j`（两色 RGB 相同、`code` 不同，如 `cells==[i, j, i]`）
- **那么** `count_colors` 必须产出**两个独立** `ColorStat`（下标 `i` 与 `j` 各一项，`count` 分别等于各自在 `cells` 中的
  出现次数：`i`→2、`j`→1），**绝不**按 RGB 合并为一项（上一场景的"全命中→仅最低下标"用例不能区分此实现差异，故需此场景）

### 需求:非 ASCII 色名在 summary 中逐字节保真
`generate_summary` 必须把色名（UTF-8 `String`）原样写入输出，禁止截断、归一化或做宽度假设。含非 ASCII 字符
（如中文、带重音字母）的色名必须在 summary 中逐字节保真出现。

#### 场景:中文/带重音色名原样出现
- **当** 调色板某用到的色 `name` 含非 ASCII 字符（如 `"碧蓝"`、`"Café"`）
- **那么** 该色名在 `generate_summary` 的输出行中逐字节原样出现

### 需求:统计确定性（含跨架构整数一致）
同一 `BeadPattern` 与同一 `Palette` 必须产生逐字节相同的 `Vec<ColorStat>` 与 summary。实现禁止引入非确定性来源
（`rayon` 并行、随机、浮点、`HashMap`/`HashSet` 迭代顺序泄漏）。计数与排序全程为**整数运算**（整数计数、整数排序键），
因此跨架构（arm64 / x86_64）必须逐字节一致——据此可钉硬编码 golden。

#### 场景:重复统计一致
- **当** 对同一 `BeadPattern` + 同一 `Palette` 多次调用 `count_colors` / `generate_summary`
- **那么** 每次返回的 `Vec<ColorStat>` 与 summary 字符串完全相等

#### 场景:跨架构位精确 golden
- **当** 对一个固定小 `BeadPattern`（含重复命中与等 count 平局）+ 固定小调色板做统计
- **那么** `Vec<ColorStat>` 等于硬编码期望、summary 等于硬编码期望字符串，且断言在 arm64 与 x86_64 上都通过

