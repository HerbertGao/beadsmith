## 修改需求

### 需求:BeadPattern 输出形状
`BeadPattern` 必须含 `width`、`height` 与行优先的 `cells: Vec<u16>`（每个 `u16` 是调色板下标），
且 `cells.len() == width*height`、`cells[y*width+x]` 为第 (x, y) 格。长度与下标运算必须以 `usize`
进行。必须提供 `cell_at(x, y) -> Option<u16>`，越界返回 `None`。`BeadPattern` 不含 `stats` 字段
（统计在 M4 以派生函数 `count_colors`/`total_beads`/`generate_summary` 从 `cells` 现算提供，**永不**作为
`BeadPattern` 字段；见 M4-D1。本括注由 add-statistics 修正——原作"（统计属 M4）"易被读作"stats 字段将在 M4 到来"，
而 M4 经探索定为"派生而非存储"，故澄清；规范性要求「不含 `stats` 字段」不变、由此永久强化）。

#### 场景:cell_at 行优先取格与越界
- **当** 对一个 `w×h` 的 `BeadPattern` 调用 `cell_at(x, y)`
- **那么** `x<w && y<h` 时返回 `Some(cells[y*width+x])`，否则返回 `None`
