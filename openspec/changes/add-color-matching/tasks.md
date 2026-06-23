## 1. models：BeadPattern

- [x] 1.1 扩 `crates/bead-core/src/models/mod.rs`，定义公开
  `BeadPattern { width:u32, height:u32, cells:Vec<u16> }`（derive `Debug+Clone+PartialEq`，
  **不 derive `Eq`**，同 `PixelGrid`，见 design D1）；文档注明行优先 `cells[y*width+x]` = 调色板
  下标、`cells.len()==width*height`（长度/下标用 `usize`）、无 `stats` 字段（M4）。加
  `pub fn cell_at(&self, x:u32, y:u32) -> Option<u16>`（越界 `None`，索引用 `usize`）

## 2. matcher：trait + RgbMatcher

- [x] 2.1 新建 `crates/bead-core/src/matcher/mod.rs`，定义公开
  `pub trait ColorMatcher { fn find_best_match(&self, target: [u8;3]) -> u16; }`（见 D2）
- [x] 2.2 `pub struct RgbMatcher` + `RgbMatcher::new(palette: &Palette) -> Result<RgbMatcher, BeadError>`：
  构造时把 `palette.colors` 的 RGB **按顺序**拍平成 `Vec<[u8;3]>` 快照（性能接缝 + 保序下标，D2/D3）。
  **守卫**：`colors` 为空 → `InvalidPalette`（reason 含 "no colors"）；`colors.len() > 65536` →
  `InvalidPalette`（reason 含 "more than"，防 `as u16` 静默截断；`len==65536` 合法，下标 0..=65535 可表示，
  见 D7）；**不 panic**
- [x] 2.3 `impl ColorMatcher for RgbMatcher`：`find_best_match` = RGB 平方欧氏距离
  （`d = (Δr)²+(Δg)²+(Δb)²`，**无 sqrt**；分量差先 `a as i32 - b as i32` 再平方防 `u8` 下溢；
  距离累加 `u32`），**平局取最低下标**（用严格 `<` 更新最优、相等不更新，D3）；返回 `u16` 下标

## 3. matcher：match_pattern 入口

- [x] 3.1 `matcher/mod.rs`：`pub fn match_pattern(grid: &PixelGrid, matcher: &dyn ColorMatcher) -> BeadPattern`——
  逐格 `grid.pixels[i] → matcher.find_best_match(*px) → cells[i]`（同 `i=y*width+x`，零坐标转换），
  `grid.width/height` 原样搬进 `BeadPattern`，遍历 `grid.pixels` 产 `cells`（故 `cells.len()==grid.pixels.len()`）。
  **前置条件**（doc 注明，同 M2 `PixelGrid` 口径）：`grid.pixels.len() == grid.width as usize * grid.height as usize`
  （长度运算用 `usize`，不 `u32` 乘）——成立时即 `cells.len()==w*h`；全函数不返 `Result`、不复检，违约属
  调用方契约违约（见 D5）

## 4. lib.rs：重导出

- [x] 4.1 `crates/bead-core/src/lib.rs`：`pub mod matcher;` + 重导出
  `ColorMatcher / RgbMatcher / match_pattern`（matcher 模块）与 `BeadPattern`（models）；
  **不新增 `BeadError` 变体**（复用 `InvalidPalette`，D7）

## 5. 测试（映射 spec 需求 + 确定性门）

- [x] 5.1 `matcher/mod.rs` `#[cfg(test)]`：`exact_hit_maps_to_zero_distance`——像素 RGB 恰等于某调色板
  色 → `find_best_match` 返回该下标（spec「精确命中」/ ROADMAP Done-when）
- [x] 5.2 `off_palette_maps_to_nearest`——离色像素 → 返回平方距离最小色的下标（手算一个已知最近案例）
- [x] 5.2b `distance_guards_widening_and_accumulator`——**钉死 i32 widening + u32 累加两个守卫**（否则
  `u8` 减法 / `u16` 累加的错误实现仍能蒙混过 5.1/5.2/5.3）：① 至少一个 target 分量**严格小于**命中
  palette 色的对应分量（负差——`u8` 减法实现会在 debug panic / release wrap）；② 一个近最大距离对
  （如 `[0,0,0]` vs `[255,255,255]` = `3×255² = 195075`——`u16` 累加会 wrap/截断，`u32` 才对），断言
  返回正确最近下标
- [x] 5.3 `tie_break_returns_lowest_index`——构造两个到 target 等距的调色板色（target 在二者正中），
  断言返回**较小下标**、重复调用一致（**确定性门**，D3.3）
- [x] 5.3b `exact_hit_duplicate_rgb_returns_lowest_index`——构造两个 **RGB 完全相同**的调色板色（仅 code
  不同；validate 只保证 code 唯一、不保证 RGB 唯一），target == 该 RGB（二者距离都为 0）→ 断言返回
  **较小下标**（spec「精确命中」的重复-RGB 子句，D3.3；与 5.3 同为 strict-`<` 机制，但钉 distance==0 这一类）
- [x] 5.4 `match_pattern_shape_and_rowmajor`——满足前置条件（`pixels.len()==w*h`）的 `w×h` PixelGrid →
  `width==w/height==h/cells.len()==w*h`，且抽查某格 `cells[i]` 对应 `pixels[i]` 的最近色（行优先一一
  对应）；附一例退化网格 `width==0`（`pixels` 空）→ `cells.len()==0`，不 panic（前置条件下的边界）
- [x] 5.5 `match_pattern_is_deterministic`（同 PixelGrid+同 Palette 两次 `match_pattern` 的 `BeadPattern`
  `PartialEq` 相等）**+ 跨架构整数 golden**：固定小 PixelGrid（含精确命中 / 等距平局 / 离色取最近
  三类格）+ 固定小调色板，断言 `cells == vec![...u16...]`**硬编码**期望（纯整数 → 跨 arm64/x86_64
  一致，`// ponytail: 整数匹配跨架构位精确，可硬编码 golden；M2 Lanczos3 f32 才不敢`）— Done-when
- [x] 5.6 `cell_at_rowmajor_and_oob`——`x<w&&y<h` → `Some(cells[y*w+x])`，越界 → `None`
- [x] 5.7 `empty_palette_rejected`——`RgbMatcher::new(空 Palette)` → `Err(InvalidPalette)`，reason 含
  "no colors"，**不 panic**（只断言变体 + 关键字，不断言完整 Display 文案，同 M1/M2）
- [x] 5.8 `oversized_palette_rejected`——**钉死真边界**：`RgbMatcher::new(65537 色)` → `Err(InvalidPalette)`，
  reason 含 "more than"；**且** `RgbMatcher::new(65536 色)` 必须 **`Ok`**（下标 0..=65535 全可表示，
  `u16::MAX==65535`）。测试内用 `(0..N).map(|i| PaletteColor{code:i.to_string(), ...})` 生成 dummy 调色板
  （`// ponytail: 6.5 万个 dummy String 仅此测试一次性分配，可接受`）
- [x] 5.9 `single_color_palette_ok`——单色调色板：任意像素都匹配到下标 0，不 panic（边界）

## 6. 收尾验证 + 文档

- [x] 6.1 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（debug **与**
  `--release`）全绿
- [x] 6.2 确认 matcher **无新依赖、无 `rayon`、无 `f32`**（纯整数，`cargo tree` 仍无 rayon；grep
  matcher 模块无浮点）；bead-core 仍无 fs/UI/平台依赖
- [x] 6.3 校正 `ARCHITECTURE.md`：①Data Model Layer 的 `BeadCell { x, y, color_index }` →
  `BeadPattern { width, height, cells: Vec<u16> }`（行优先，坐标可推；删/不引入 `BeadCell` struct）；
  并把 `BeadPattern` 的 `stats: Vec<ColorStat>` 字段**标注「M4 起填充，M3 不含」**（同 6.3b 对 INIT 的口径，
  使 ARCHITECTURE / INIT / M3 代码对 `stats` 讲同一个故事——M3 `BeadPattern` 无 `stats`，ARCHITECTURE/INIT
  作为前向设计文档保留该字段但注明 M4 才有）；②matcher 段确认 `ColorMatcher`/`find_best_match` 与实现一致；
  ③Rendering Strategy 注明配色后 `BeadPattern` 是真理源、`PixelGrid` 是配色前中间体（M4 从 cells 计数、
  M5 从 cells 查 palette 上色）
- [x] 6.3a 校正 `crates/bead-core/src/models/mod.rs` **模块级 doc-comment**（line 2，现「M3 adds
  `BeadCell` / `BeadPattern`」）→ 描述 `BeadPattern { cells: Vec<u16> }`，删 `BeadCell` 提法。**与 task 1.1
  在同一文件编辑里完成**，否则注释一落地即成谎
- [x] 6.3b 校正 `INIT.md` Data Models 块（`BeadCell { x, y, color_index }` + `BeadPattern.cells:
  Vec<BeadCell>` + `stats`）：删 `BeadCell` struct、`BeadPattern.cells` → `Vec<u16>`，注明 `stats` 随 M4
  （INIT 与 ARCHITECTURE 保持同一数据模型故事）
- [x] 6.4 校正 `ROADMAP.md` M3：**强制**把「Introduce `BeadCell`/`BeadPattern`」「`BeadCell.color_index`
  now points into the palette」与 L56 的 `BeadCell` 提法改为 `BeadPattern { cells: Vec<u16> }`（**非「如需」
  ——这三处直接矛盾本变更，必须改**）；同时核对 M3 Done-when（已知色→精确条目、离色→最近、确定性）与本
  变更一致
- [x] 6.5 收尾确认：`grep -rn "BeadCell" ARCHITECTURE.md ROADMAP.md INIT.md crates/` **零命中**（主 agent 已跑，确认 0 命中）——
  四处真理源（含 `crates/bead-core/src/models/mod.rs` doc-comment）全部反映 `Vec<u16>`。**注意只扫这四类
  目标**，不扫整仓：本变更目录 `openspec/changes/add-color-matching/`（proposal/design/tasks 正讨论「移除
  `BeadCell`」）与归档 `openspec/changes/archive/` 都**合法保留** `BeadCell` 字样，扫全仓会永远非零、变成
  扫不过的假门
