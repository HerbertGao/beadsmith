## 1. 数据资产 + 署名

- [x] 1.1 从上游 MIT 源 `maxcleme/beadcolors` 的 `raw/artkal_s.csv`（199 行：159 标准 S + 40 扩展，`code,name,R,G,B`；
  apply 时拉取并把 GitHub URL + commit hash 固定进 `NOTICE`）转换生成 `palettes/artkal_s.json`：
  `{"brand":"Artkal S","colors":[{"code","name","rgb":"#RRGGBB"}…]}`，字段名用 `rgb`（非 `hex`），
  hex 由 `#%02X%02X%02X` 计算；`brand` 固定为系列标签 `"Artkal S"`（见 design D10）
- [x] 1.2 新增仓根 `NOTICE`：保留 `maxcleme/beadcolors` 的 MIT 署名
- [x] 1.3 新增 `palettes/README.md`（或 `palettes/DATA.md`）数据说明：5mm Midi S、社区近似 hex、
  非官方、含 199 色（159 标准 S + 40 扩展），官方现 ~200+
- [x] 1.4 校验生成文件：合法 JSON、`code` 全唯一、所有 `rgb` 匹配 `^#[0-9A-Fa-f]{6}$`
  （一次性 `jq`/脚本检查即可，不入库）— 对应 Done-when「附带的 Artkal 调色板可加载」

## 2. 依赖（Cargo）

- [x] 2.1 根 `Cargo.toml` `[workspace.dependencies]` 加
  `serde = { version = "1", features = ["derive"] }` 与 `serde_json = "1"`
- [x] 2.2 `crates/bead-core/Cargo.toml` 加 `serde.workspace = true` 与 `serde_json.workspace = true`

## 3. bead-core：错误模型

- [x] 3.1 `crates/bead-core/src/lib.rs`：给 `BeadError` 增加
  `PaletteParse(#[from] serde_json::Error)` 与 `InvalidPalette { reason: String }`
  两个变体（enum 已 `#[non_exhaustive]`），写 `thiserror` 文案

## 4. bead-core：palette 模块

- [x] 4.1 新建 `crates/bead-core/src/palette/mod.rs`，定义公开 `PaletteColor { code, name, rgb:[u8;3] }`
  与 `Palette { brand, colors:Vec<PaletteColor> }`（derive `Debug+Clone+PartialEq`，**不 derive `Eq`**，见 design D2）
- [x] 4.2 `palette/mod.rs`：私有 DTO `RawPalette`/`RawColor`（`#[derive(Deserialize)]`，`rgb: String`）；
  **不加** `#[serde(deny_unknown_fields)]`——多余字段静默忽略（宽松，见 design D2）
- [x] 4.3 `palette/mod.rs`：`parse_hex(&str)->Result<[u8;3],BeadError>`，严格 `#RRGGBB`、大小写不敏感，
  拒绝简写/无 `#`/错长度；返回**不含 code** 的错误，由 4.4 的 `load_palette` 补 code 上下文
- [x] 4.4 `palette/mod.rs`：`load_palette(&[u8])`——`from_slice::<RawPalette>` → 按固定顺序校验
  **① `colors` 非空 → ② 按序逐个 `parse_hex`（坏 hex 时 `load_palette` 包装成 `InvalidPalette` 并点名当前 `code`）
  → ③ 唯一 `code`**（fail-fast，**禁 HashMap**，错误点名单个 code）→ `Palette`
- [x] 4.5 `palette/mod.rs`：`validate_palette(&Palette)->Result<(),BeadError>`，只复查结构不变量
- [x] 4.6 `crates/bead-core/src/lib.rs`：`pub mod palette;` + 重导出
  `Palette / PaletteColor / load_palette / validate_palette`

## 5. 测试（映射 Done-when + 边界）

> 注：含 `#` 的 JSON 夹具须用 `br##"..."##`（多一个 `#`），否则 raw 字节串定界符与
> `#RRGGBB` 里的 `#` 冲突导致编译失败。
> 注：测重复 code 的夹具（5.4 / 5.9）所有颜色必须用合法 hex——否则按 D5 顺序会先在
> ② `parse_hex` 短路返回 hex 错，触达不到 ③ 唯一 code 检查。

- [x] 5.1 `palette/mod.rs` `#[cfg(test)]`：`load_valid_palette_parses_all_colors`
  （`#0A0B0C`→`[10,11,12]`，数量+顺序）— Done-when「合法调色板」
- [x] 5.2 `load_rejects_malformed_hex`（`#00GG00`→`InvalidPalette`，reason 点名 code）— Done-when「坏 hex」
- [x] 5.3 `load_rejects_empty_colors`（`[]`→`InvalidPalette`）— Done-when「空调色板」
- [x] 5.4 `load_rejects_duplicate_codes`（两个 `S01`→`InvalidPalette`，点名 code）
- [x] 5.5 边界：`load_rejects_wrong_hex_length`（`#FFF`）、`load_rejects_missing_hash`（`000000`）、
  `load_accepts_lowercase_hex`（`#aabbcc`→`[170,187,204]`）
- [x] 5.6 `load_rejects_malformed_json`（`b"{ not json"`→`PaletteParse`）、`load_rejects_missing_field`
  （缺 `rgb`→`PaletteParse`）；**只断言变体，不断言 serde_json Display 文案**
- [x] 5.7 `validate_passes_on_loaded_palette`（对已加载 `Palette` 返回 `Ok`）
- [x] 5.7b `validate_rejects_empty`（手工构造 `colors` 为空的 `Palette` → `Err(InvalidPalette)`）
- [x] 5.7c `validate_rejects_duplicate_codes`（手工构造含重复 `code` 的 `Palette` →
  `Err(InvalidPalette)`，点名 code）—— 防止 no-op `validate_palette` 漏过 spec 契约
- [x] 5.8 `bundled_artkal_palette_loads`：`include_bytes!("../../../../palettes/artkal_s.json")`
  → `Ok`、非空、`code` 唯一 — Done-when「附带 Artkal 调色板可加载」
- [x] 5.9 `load_is_deterministic`：同一字节两次 `load_palette` 结果 `PartialEq` 相等；
  同一坏 JSON（坏 hex / 重复 code）两次得到逐字节相同的 `InvalidPalette.reason`，且点名的
  code 为文档顺序中首个触发者（验证 D5 校验顺序确定）

## 6. 收尾验证

- [x] 6.1 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 全绿
- [x] 6.2 确认 `bead-core` 仍无文件系统/UI/平台依赖（core 只收字节；`include_bytes!` 为编译期）
