//! Per-color statistics derived from a [`BeadPattern`]. Pure integer counting —
//! no `f32`, no `HashMap`/`HashSet`, no `rayon`. Counts iterate `BeadPattern.cells`
//! (palette indices); raw `PixelGrid` RGB and rendered images are never touched
//! (CLAUDE rule 3 / design D1).
//!
//! Three total primitives (design D7 — no `Result`, no new `BeadError` variant):
//! [`count_colors`], [`total_beads`], [`generate_summary`]. They are library /
//! pipeline-reuse primitives, not FFI entry points (design D8); `pipeline::
//! generate_pattern` (M6) is the single external entry and packages them.

use crate::models::{BeadPattern, ColorStat};
use crate::palette::Palette;

/// Count, per palette color, how many cells of `grid` use it.
///
/// Walks `grid.cells` once, tallying each palette index into a dense
/// index-keyed `Vec<u32>` (no `HashMap`/`HashSet` — deterministic, integer
/// only, continuing the M1 ordered-`Vec` precedent, design D3). Returns one
/// [`ColorStat`] per **used** color (`count > 0`); colors absent from `cells`
/// are omitted (design D3). Each stat's `code` / `name` come from
/// `palette.colors[index]`.
///
/// Ordering (determinism gate, design D2): `count` descending; ties broken by
/// **lowest palette index** first. This is achieved by collecting non-zero
/// entries in index order, then a **stable** sort on `count` descending — the
/// stable sort preserves the index-ascending order within equal-`count` groups.
///
/// **Total function** (no `Result`) with a documented precondition (design D4):
/// every `cells[i]` must be a valid index into `palette.colors`, and `palette`
/// must be the same unmodified palette the matcher that produced `grid` used.
/// Violating it (e.g. a smaller palette with out-of-bounds indices) does **not**
/// panic: an out-of-bounds index is skipped for per-color counting but still
/// counts toward [`total_beads`], so `Σ count < total_beads` is an observable
/// "wrong palette" signal (design D4). An empty palette (`colors.len() == 0`) is
/// the degenerate case: every cell is out of bounds, all skipped → `[]`.
pub fn count_colors(grid: &BeadPattern, palette: &Palette) -> Vec<ColorStat> {
    // Dense index-keyed counts — no HashMap/HashSet (design D3): index order is
    // inherently deterministic and counting stays pure-integer.
    let mut counts = vec![0u32; palette.colors.len()];

    for &idx in &grid.cells {
        // ponytail: 越界=传错 palette（更小/不同的调色板）。靠边界检查跳过 per-color
        // 计数（仍计入 total_beads），由 Σ count < total 这一可观测信号暴露误用；
        // 不用会在 cargo test debug 下 panic 的 debug_assert!（违 spec「不 panic」, D4）。
        if (idx as usize) < counts.len() {
            counts[idx as usize] += 1;
        }
    }

    // Collect non-zero entries in index order (the determinism anchor), then a
    // STABLE sort by count descending. Stable -> equal-count groups keep their
    // index-ascending order (design D2 lowest-index tiebreak).
    let mut stats: Vec<ColorStat> = counts
        .iter()
        .enumerate()
        .filter(|(_, &count)| count > 0)
        .map(|(i, &count)| ColorStat {
            code: palette.colors[i].code.clone(),
            name: palette.colors[i].name.clone(),
            count,
        })
        .collect();

    // ponytail: 必须 sort_by（稳定）——sort_unstable_by 会破坏等 count 平局的最低下标序。
    // ponytail: 双模式（按 code 排）由 app 层对返回的 Vec<ColorStat> 自调 .sort_by(code)
    //           实现（ColorStat 自带 code），引擎只产这一个规范序、不留双模式代码（D2）。
    // clippy 建议 sort_by_key(Reverse(..))——同样稳定、不改语义，但 D2 把降序比较器
    // 钉为这一确切形式；显式 allow 而非改写，保留「稳定排序」意图的可读性。
    #[allow(clippy::unnecessary_sort_by)]
    stats.sort_by(|a, b| b.count.cmp(&a.count));

    stats
}

/// The total bead count of `grid` = `cells.len()`.
///
/// Defined as `grid.cells.len() as u32` — independent of any palette and of
/// [`count_colors`] (design D5). When the palette precondition holds (every
/// `cells[i]` is valid) this equals both `width * height` and the sum of
/// `count_colors(..).count`; when it is violated (smaller palette, out-of-bounds
/// indices) the latter degrades to `Σ count < total_beads` (design D4).
///
/// **Precondition** `cells.len() <= u32::MAX` (same caller-held-invariant
/// posture as `models`' `cells.len() == width * height`; reachable bead grids
/// are far below this — `as u32` would truncate beyond it, design D5).
pub fn total_beads(grid: &BeadPattern) -> u32 {
    grid.cells.len() as u32
}

/// Produce the directly-copyable INIT "Summary Format" text for `grid` +
/// `palette` (design D6), reusing [`count_colors`] and [`total_beads`] so the
/// summary and the structured stats never disagree.
///
/// Byte-exact layout:
/// ```text
/// Bead Pattern Summary
/// Size: {width} x {height}
/// Total Beads: {total}
/// Palette: {brand}
///                       <- single blank line
/// {code} {name}: {count}   <- one line per used color, count-desc / lowest-index
/// ...
/// ```
/// The `x` in the size line has a space on each side; each color line carries a
/// colon (`{code} {name}: {count}`); the output ends with a trailing newline.
///
/// Empty grid (`width == 0` or `height == 0`, empty `cells`): the 4-line header
/// (with `Total Beads: 0`) + the blank-line separator, no color lines, ending
/// `\n\n`. No panic.
///
/// **Precondition**: `brand` / `code` / `name` contain no line-break control
/// characters (`\n` / `\r`); they are written verbatim, byte-faithful (design
/// D6 / D9.3). The "wrong palette" degradation (`Total Beads` vs Σ of color
/// lines) is rendered faithfully but not detected/flagged here — diagnosing it
/// is M6's job (design D6).
pub fn generate_summary(grid: &BeadPattern, palette: &Palette) -> String {
    use std::fmt::Write;

    let stats = count_colors(grid, palette);
    let total = total_beads(grid);

    // ponytail: 一个 format! 拼 4 行头 + 空行分隔符，再 writeln! 把每色行写进同一 buffer
    //           （std::fmt::Write，写 String 永不失败，故弃 Result）——省掉 4 个一次性 format!
    //           分配，字节输出与逐 push_str 完全一致（4.4/4.9 golden 钉死）。
    let mut out = format!(
        "Bead Pattern Summary\nSize: {} x {}\nTotal Beads: {total}\nPalette: {}\n\n",
        grid.width, grid.height, palette.brand
    );
    for stat in &stats {
        let _ = writeln!(out, "{} {}: {}", stat.code, stat.name, stat.count);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::{match_pattern, RgbMatcher};
    use crate::models::PixelGrid;
    use crate::palette::PaletteColor;

    // Build a Palette from (code, name, rgb) triples. Names ARE load-bearing for
    // statistics (they go into ColorStat / summary), so they are explicit here.
    fn palette_from(colors: &[(&str, &str, [u8; 3])]) -> Palette {
        Palette {
            brand: "Test".to_string(),
            colors: colors
                .iter()
                .map(|(code, name, rgb)| PaletteColor {
                    code: code.to_string(),
                    name: name.to_string(),
                    rgb: *rgb,
                })
                .collect(),
        }
    }

    // 4.1 — count_colors: each used color's count/code/name is correct; colors
    // absent from cells are NOT in the result.
    #[test]
    fn count_colors_counts_used_only() {
        // index 0 RED, 1 GREEN, 2 BLUE. cells use only 0 and 2.
        let palette = palette_from(&[
            ("R", "Red", [255, 0, 0]),
            ("G", "Green", [0, 255, 0]),
            ("B", "Blue", [0, 0, 255]),
        ]);
        // cells: 0 appears 3x, 2 appears 1x, 1 (GREEN) never.
        let grid = BeadPattern {
            width: 2,
            height: 2,
            cells: vec![0, 0, 2, 0],
        };

        let stats = count_colors(&grid, &palette);

        // Only used colors -> two entries (R and B), GREEN omitted.
        assert_eq!(stats.len(), 2);
        // R appears 3x -> first (count desc).
        assert_eq!(
            stats[0],
            ColorStat {
                code: "R".to_string(),
                name: "Red".to_string(),
                count: 3
            }
        );
        assert_eq!(
            stats[1],
            ColorStat {
                code: "B".to_string(),
                name: "Blue".to_string(),
                count: 1
            }
        );
        // GREEN (index 1) must not appear.
        assert!(!stats.iter().any(|s| s.code == "G"));
    }

    // 4.2 — sort: count descending; equal count -> lowest index first; repeated
    // calls identical (determinism gate, design D2).
    #[test]
    fn sort_count_desc_tiebreak_lowest_index() {
        // index 0 A, 1 B, 2 C, 3 D.
        let palette = palette_from(&[
            ("A", "Aa", [1, 0, 0]),
            ("B", "Bb", [2, 0, 0]),
            ("C", "Cc", [3, 0, 0]),
            ("D", "Dd", [4, 0, 0]),
        ]);
        // counts: A=1, B=1 (tie at lowest count), C=3, D=2.
        let grid = BeadPattern {
            width: 7,
            height: 1,
            cells: vec![0, 1, 2, 2, 2, 3, 3],
        };

        let stats = count_colors(&grid, &palette);

        // count desc: C(3), D(2), then tie A(1) before B(1) by lowest index.
        assert_eq!(stats.len(), 4);
        assert_eq!(stats[0].code, "C"); // count 3
        assert_eq!(stats[1].code, "D"); // count 2
                                        // tie at count 1: index 0 (A) before index 1 (B).
        assert_eq!(stats[2].code, "A");
        assert_eq!(stats[3].code, "B");
        assert_eq!(stats[2].count, 1);
        assert_eq!(stats[3].count, 1);

        // descending invariant across all neighbors.
        for w in stats.windows(2) {
            assert!(w[0].count >= w[1].count);
        }

        // repeated calls are identical.
        let again = count_colors(&grid, &palette);
        assert_eq!(stats, again);
    }

    // 4.3 — total_beads cross-check: total == cells.len() == width*height ==
    // Σ count_colors(..).count, with a CORRECT (in-bounds) palette (design D5).
    #[test]
    fn total_beads_cross_check() {
        let palette = palette_from(&[
            ("A", "Aa", [1, 0, 0]),
            ("B", "Bb", [2, 0, 0]),
            ("C", "Cc", [3, 0, 0]),
        ]);
        // 3x2 = 6 cells, every index in 0..3 -> all in-bounds.
        let grid = BeadPattern {
            width: 3,
            height: 2,
            cells: vec![0, 1, 2, 0, 1, 0],
        };

        let total = total_beads(&grid);
        let sum: u32 = count_colors(&grid, &palette).iter().map(|s| s.count).sum();
        let wh = grid.width * grid.height;

        assert_eq!(total, grid.cells.len() as u32);
        assert_eq!(total, wh);
        assert_eq!(total, sum);
        // four-way equality holds (correct palette, no out-of-bounds).
        assert_eq!(grid.cells.len() as u32, 6);
    }

    // 4.4 — generate_summary byte-exact format (design D6). Golden pinned from
    // the deterministic output.
    #[test]
    fn summary_exact_format() {
        let palette = palette_from(&[
            ("S01", "Black", [0, 0, 0]),
            ("S02", "White", [255, 255, 255]),
        ]);
        // 80 cells laid out as 2 rows of 40 is overkill; use a small grid whose
        // counts and order are easy to reason about. counts: S01=3, S02=2.
        let grid = BeadPattern {
            width: 5,
            height: 1,
            cells: vec![0, 0, 0, 1, 1],
        };

        let summary = generate_summary(&grid, &palette);

        let expected = "Bead Pattern Summary\n\
                        Size: 5 x 1\n\
                        Total Beads: 5\n\
                        Palette: Test\n\
                        \n\
                        S01 Black: 3\n\
                        S02 White: 2\n";
        assert_eq!(summary, expected);
        // Palette line equals palette.brand.
        assert!(summary.contains(&format!("Palette: {}\n", palette.brand)));
        // Total Beads equals total_beads.
        assert!(summary.contains(&format!("Total Beads: {}\n", total_beads(&grid))));
    }

    // 4.5 — empty grid summary: two instances covering the generic
    // `Size: {width} x {height}` (design D6 exact bytes / D9.2).
    #[test]
    fn summary_empty_grid() {
        let palette = palette_from(&[("S01", "Black", [0, 0, 0])]);

        // (a) width == 0, height == 5.
        let grid_a = BeadPattern {
            width: 0,
            height: 5,
            cells: vec![],
        };
        assert_eq!(count_colors(&grid_a, &palette), vec![]);
        assert_eq!(total_beads(&grid_a), 0);
        assert_eq!(
            generate_summary(&grid_a, &palette),
            "Bead Pattern Summary\nSize: 0 x 5\nTotal Beads: 0\nPalette: Test\n\n"
        );

        // (b) height == 0, width == 5.
        let grid_b = BeadPattern {
            width: 5,
            height: 0,
            cells: vec![],
        };
        assert_eq!(count_colors(&grid_b, &palette), vec![]);
        assert_eq!(total_beads(&grid_b), 0);
        assert_eq!(
            generate_summary(&grid_b, &palette),
            "Bead Pattern Summary\nSize: 5 x 0\nTotal Beads: 0\nPalette: Test\n\n"
        );
    }

    // 4.6 — smaller palette: out-of-bounds indices skipped, no panic in debug or
    // release; out-of-bounds cells excluded from ColorStat but counted in
    // total_beads (Σ count < total). Plus empty-palette sub-assertion (design D4).
    #[test]
    fn smaller_palette_skips_not_panic() {
        // Pattern produced as if from a 3-color palette: indices 0, 1, 2.
        let grid = BeadPattern {
            width: 4,
            height: 1,
            cells: vec![0, 1, 2, 0],
        };
        // A SMALLER palette with only indices 0 and 1 -> index 2 is out of bounds.
        let smaller = palette_from(&[("A", "Aa", [1, 0, 0]), ("B", "Bb", [2, 0, 0])]);

        let stats = count_colors(&grid, &smaller); // must not panic
        let sum: u32 = stats.iter().map(|s| s.count).sum();
        let total = total_beads(&grid);

        // index 0 appears 2x, index 1 once; index 2 (out of bounds) skipped.
        assert_eq!(sum, 3);
        assert_eq!(total, 4);
        assert!(sum < total, "Σ count must be < total when indices overflow");
        // The out-of-bounds index never produced a ColorStat.
        assert_eq!(stats.len(), 2);

        // generate_summary must also not panic on the degenerate input.
        let _ = generate_summary(&grid, &smaller);

        // Empty-palette special case: every cell out of bounds -> [], no panic.
        let empty_palette = Palette {
            brand: "Empty".to_string(),
            colors: vec![],
        };
        assert_eq!(count_colors(&grid, &empty_palette), vec![]);
        assert_eq!(total_beads(&grid), 4); // total unaffected by palette
        let _ = generate_summary(&grid, &empty_palette); // no panic
    }

    // 4.7 — duplicate RGB, different code (index i < j). (a) full-hit matcher
    // grid -> only index i; (b) discriminating hand-built grid -> two stats
    // (design D9.1).
    #[test]
    fn duplicate_rgb_counts_by_index() {
        // index 0 = ("A", rgb), index 1 = ("B", same rgb).
        let palette = palette_from(&[("A", "Aa", [42, 42, 42]), ("B", "Bb", [42, 42, 42])]);

        // (a) matcher produces a fully-hitting grid: every pixel == the dup RGB.
        // The matcher sends exact hits to the LOWEST index (0), so index 1 never
        // appears -> only A is reported, B omitted (count == 0).
        let matcher = RgbMatcher::new(&palette).expect("valid palette");
        let px_grid = PixelGrid {
            width: 3,
            height: 1,
            pixels: vec![[42, 42, 42], [42, 42, 42], [42, 42, 42]],
        };
        let pattern = match_pattern(&px_grid, &matcher);
        assert_eq!(pattern.cells, vec![0, 0, 0]); // all to lowest index
        let stats_a = count_colors(&pattern, &palette);
        assert_eq!(stats_a.len(), 1);
        assert_eq!(stats_a[0].code, "A");
        assert_eq!(stats_a[0].count, 3);
        assert!(!stats_a.iter().any(|s| s.code == "B"));

        // (b) discriminating: hand-build cells containing BOTH indices, bypassing
        // the matcher. Counting is by INDEX, not merged by RGB -> two stats.
        let hand = BeadPattern {
            width: 3,
            height: 1,
            cells: vec![0, 1, 0], // i=0 twice, j=1 once
        };
        let stats_b = count_colors(&hand, &palette);
        assert_eq!(
            stats_b.len(),
            2,
            "must be two independent stats, not merged"
        );
        // index 0 (count 2) before index 1 (count 1) by count desc.
        assert_eq!(stats_b[0].code, "A");
        assert_eq!(stats_b[0].count, 2);
        assert_eq!(stats_b[1].code, "B");
        assert_eq!(stats_b[1].count, 1);
    }

    // 4.8 — non-ASCII color name appears byte-faithfully in summary (design D9.3).
    #[test]
    fn non_ascii_name_byte_faithful() {
        let palette = palette_from(&[("C1", "碧蓝", [0, 0, 255]), ("C2", "Café", [120, 80, 40])]);
        let grid = BeadPattern {
            width: 3,
            height: 1,
            cells: vec![0, 0, 1],
        };

        let summary = generate_summary(&grid, &palette);

        // Each non-ASCII name's full color line appears byte-for-byte.
        assert!(summary.contains("C1 碧蓝: 2"));
        assert!(summary.contains("C2 Café: 1"));
        // And the raw multi-byte sequences are present unaltered.
        assert!(summary.contains("碧蓝"));
        assert!(summary.contains("Café"));
    }

    // 4.9 — determinism (a) repeated calls equal; (b) hardcoded cross-arch golden
    // for a fixed small grid (repeat hits + equal-count tie) (design D9.5).
    // ponytail: 整数统计跨架构位精确，可硬编码 golden
    #[test]
    fn statistics_is_deterministic() {
        // index 0 A, 1 B, 2 C. counts: A=3, B=1, C=1 (B/C tie at count 1).
        let palette = palette_from(&[
            ("A", "Alpha", [1, 0, 0]),
            ("B", "Beta", [2, 0, 0]),
            ("C", "Gamma", [3, 0, 0]),
        ]);
        let grid = BeadPattern {
            width: 5,
            height: 1,
            cells: vec![0, 0, 0, 1, 2],
        };

        // (a) repeated calls equal.
        let s1 = count_colors(&grid, &palette);
        let s2 = count_colors(&grid, &palette);
        assert_eq!(s1, s2);
        assert_eq!(
            generate_summary(&grid, &palette),
            generate_summary(&grid, &palette)
        );

        // (b) hardcoded golden: A first (count 3), then tie B(1) before C(1) by
        // lowest index.
        assert_eq!(
            s1,
            vec![
                ColorStat {
                    code: "A".to_string(),
                    name: "Alpha".to_string(),
                    count: 3
                },
                ColorStat {
                    code: "B".to_string(),
                    name: "Beta".to_string(),
                    count: 1
                },
                ColorStat {
                    code: "C".to_string(),
                    name: "Gamma".to_string(),
                    count: 1
                },
            ]
        );
        assert_eq!(
            generate_summary(&grid, &palette),
            "Bead Pattern Summary\n\
             Size: 5 x 1\n\
             Total Beads: 5\n\
             Palette: Test\n\
             \n\
             A Alpha: 3\n\
             B Beta: 1\n\
             C Gamma: 1\n"
        );
    }
}
