## 1. bead-cli 实现

- [x] 1.1 `crates/bead-cli/src/main.rs`：新增 `const BUILTIN_PALETTES: &[(&str, &str)]`，
  用 `include_str!("../../../palettes/<name>.json")` 内置 14 份色卡，id 与顺序对齐 App
  `palette_registry.dart`（MARD → Artkal S/A/C/M/R → Hama Midi/Maxi/Mini →
  Perler/Caps/Mini → Nabbi → Yant），**不含** `_unlicensed/` 任何文件
- [x] 1.2 `crates/bead-cli/src/main.rs`：`PaletteCmd::List` 由 `anyhow::bail!("coming soon")`
  改为调 `palette_list()`；更新 `List` 变体 doc 注释
- [x] 1.3 `crates/bead-cli/src/main.rs`：新增 `fn palette_list()`——遍历 `BUILTIN_PALETTES`，
  每份经 `load_palette` 解析，打印 `id  brand  N colors`，退出码 0（brand/色数不硬编码）

## 2. 验证

- [x] 2.1 `crates/bead-cli/src/main.rs`：`#[cfg(test)]` 单测断言 `BUILTIN_PALETTES.len() == 14`
  且每份 `load_palette` 成功、色数 > 0（防误嵌坏/缺文件）
- [x] 2.2 `crates/bead-cli/tests/cli.rs`：把旧「`palette list` 桩必须非零退出 + 含 coming
  soon」断言改为「退出 0、stdout 恰 14 行、含 `mard`/`MARD`」；`inspect` 桩断言保留
- [x] 2.3 `cargo test` 全绿；`cargo run -p bead-cli -- palette list` 人工核对输出 14 行
- [x] 2.4 `crates/bead-cli/src/main.rs`：新增单测 `builtin_palettes_match_source_dir` 防漂移——读盘
  `palettes/*.json`（除 `_unlicensed/` 子目录与 README）stem 集，断言 == `BUILTIN_PALETTES` id 集（双向），
  并逐份断言内置 JSON 与 `palettes/<id>.json` **逐字节相等**（锁 id↔文件对应、防未来新增色卡漏改 CLI；
  对齐 App `palette_assets_test` 设计 D8）

## 3. 归档时同步主规范（未实现子命令需求已收窄，目的段须手改）

- [ ] 3.1 归档 `cli-palette-list` 后，手动更新主 spec `openspec/specs/cli/spec.md` 的**目的段**
  （约第 6-7 行）：`palette list` 由「未实现桩」改为「列出内置色卡」，仅 `inspect` 保持桩——OpenSpec
  的需求增量合并**不覆盖目的散文段**，不手改会残留自相矛盾描述（Codex/RC round-1 finding）
