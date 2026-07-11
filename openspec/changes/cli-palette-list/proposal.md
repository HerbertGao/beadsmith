## 为什么

`bead-cli palette list` 从 M6 起一直是显式「coming soon」桩（非零退出）。M10 引擎侧
已落地 13 个 MIT-clean 品牌色卡（连同既有 `artkal_s` 共 14 份），但 CLI 用户无从在命令行
发现有哪些内置色卡、各有多少色。这是 M10「更多色卡」工作线**引擎侧的最后一块**——补齐后
CLI 即与 App 的选色卡能力对等（App 已能从 14 份内置色卡里选，见已归档的
`app-palette-selection`）。

现状还使 `cli` 规范与实现相矛盾：规范要求 `palette list` 为桩，本变更让它真实列出色卡，
故必须同步修订规范。

## 变更内容

- **`palette list` 由桩改为真实实现**：列出随二进制**编译期内置**的 14 份色卡，每份一行含
  id、品牌名（`brand`）、色数，退出码 0。
- **内置方式 = `include_str!` 嵌入**（不扫文件系统）：`bead-core` 保持无 fs（规则 1），
  二进制装到任何位置都能列；内置集合与顺序对齐 App 的 `palette_registry.dart`，排除 AGPL
  受阻的 `palettes/_unlicensed/`。
- **品牌名/色数由 `load_palette` 解析得出**，不硬编码——JSON 文件为唯一真相源，防与实际
  色卡漂移。
- **`inspect` 仍为桩**：本变更只摘出 `palette list`，`inspect <path>` 继续是「coming
  soon」非零退出桩。
- **确定性/引擎零影响**：不碰 `pipeline`、不改任何 pattern/统计/渲染输出、不动 golden；
  纯新增只读列举命令。

## 功能 (Capabilities)

### 新增功能

- **cli**：`palette list 列出内置色卡`——列出编译期内置的 14 份色卡（id/brand/色数），
  退出码 0。

### 修改功能

- **cli**：`未实现子命令显式报错而非静默或 panic`——从中摘出 `palette list`（已实现），
  该桩需求收窄为仅覆盖 `inspect <path>`。
