# cli 规范增量

## 修改需求

### 需求:未实现子命令显式报错而非静默或 panic
`bead-cli inspect <path>` 必须是**显式的「尚未实现」桩**：打印一条「coming soon」类消息到 stderr 并以非零退出码（1）结束，**禁止**静默成功、**禁止** `panic!`/`unimplemented!`。（`palette list` 已实现，见「palette list 列出内置色卡」需求，不再属本桩。）

#### 场景:桩命令以非零退出并提示未实现
- **当** 运行 `bead-cli inspect <path>`
- **那么** 进程退出码为 1，stderr 含「coming soon / 尚未实现」类提示，且不 panic

## 新增需求

### 需求:palette list 列出内置色卡
`bead-cli palette list` MUST 列出**随二进制内置的**色卡清单，每份一行含 **id、品牌名（`brand`）、色数**，以**退出码 0** 结束、不 panic、不读取任何外部文件。

内置色卡在 CLI 层用 `include_str!` **编译期嵌入**（**不**扫文件系统——`bead-core` 保持无 fs（规则 1），且二进制装到任何位置都可列、结果确定）。`brand` 与色数一律**由 `load_palette` 解析各内置 JSON 得出**，**不硬编码**（JSON 文件为唯一真相源，防漂移）。内置**集合**对齐 App 的 `palette_registry.dart`（当前 14 份：MARD → Artkal S/A/C/M/R → Hama Midi/Maxi/Mini → Perler/Caps/Mini → Nabbi → Yant）——集合由 `builtin_palettes_match_source_dir` 测试与源 `palettes/` 目录锁定；**数组顺序**（= `palette list` 展示序）按 App 展示序**手工保持、非测试强锁**（列表顺序属外观）。**排除** AGPL 受阻的 `palettes/_unlicensed/` 各牌。

#### 场景:列出内置色卡各一行
- **当** 运行 `bead-cli palette list`
- **那么** 退出码 0，stdout **每个内置色卡恰一行**（当前 14 行），每行含该色卡的 id、`brand` 与色数（如 `mard  MARD  221 colors`）
- **且** 不读取任何外部文件、不 panic

#### 场景:内置色卡表全部可解析
- **当** `palette list` 遍历 CLI 内置色卡表
- **那么** 每份经 `load_palette` 解析成功且色数 > 0（内置桩测试断言，防误嵌坏或缺文件）

#### 场景:内置集合与源色卡目录一致（防漂移）
- **当** 把 CLI 内置色卡表与源 `palettes/*.json`（排除 `_unlicensed/` 子目录与 README）比对
- **那么** 二者 id / 文件名 stem 集合**双向相等**，且每份内置 JSON 与 `palettes/<id>.json` **逐字节一致**（测试断言）——磁盘新增/删除色卡而 CLI 漏改、或 `include_str!` 写错 basename 时立即失败，不静默发布（对齐 App `palette_assets_test` 设计 D8）
