## 修改需求

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
