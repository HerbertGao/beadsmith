# 任务：去孤立点 / 小连通域清理（despeckle）

## 1. GenerateOptions 字段

- [x] 1.1 `GenerateOptions` 增 `despeckle: Option<u32>`，`Default` 填 `None`；更新「默认选项填充」单测断言 `despeckle==None`（`crates/bead-core/src/pipeline/mod.rs`）

## 2. pattern-cleanup 模块：连通域去斑

- [x] 2.1 新建 `crates/bead-core/src/cleanup/mod.rs`（或 pattern-cleanup 模块），入口 `fn despeckle(pattern: &BeadPattern, min_region: u32) -> BeadPattern`（同形状、pattern→pattern、不新增 trait）
- [x] 2.2 实现（**输入快照语义**）：分量发现 + 计票**读输入快照**、重映射写**独立输出缓冲**、**单遍**（序不变、与扫描顺序无关）；行优先扫描找**同色 4-连通分量**（整数）；分量珠数 ≤ `min_region` 时，**逐边界邻接边**对异色外侧格计票、并入**票数最多者**（平局取最小下标）；并入色必为已存在相邻珠色（**永不发明**）；全图单色 / 空 pattern → 原样返回；合法输入不 panic
- [x] 2.3 从 crate 根 `pub use` 导出 despeckle 入口（`crates/bead-core/src/lib.rs`）

## 3. pipeline 后段步骤

- [x] 3.1 `generate_pattern` 后段加可选 despeckle（`crates/bead-core/src/pipeline/mod.rs`）：顺序 `… →（可选 GreedyReducer）→（当 despeckle==Some(s)）despeckle(&pattern, s) → count/summary/render`；`None` 跳过、pattern 原样
- [x] 3.2 确认统计/摘要/两张 PNG 基于**去斑后** pattern；不新增 `BeadError` 变体；不内联算法

## 4. CLI

- [x] 4.1 `generate` 加 `--despeckle <N>`（`u32`，可选）+ 映射到 `opts.despeckle`（未给=None）；`--despeckle 0` no-op、非 u32 退出码 2（`crates/bead-cli/src/main.rs`）
- [x] 4.2 集成测试：`--despeckle N` 退出 0 写四文件；不给时 None；非 u32 退出 2 不 panic（`crates/bead-cli/tests/cli.rs`）

## 5. 单测（语义 / 确定性）

- [x] 5.1 单颗被同色 4-邻包围的异色 + `min_region>=1` → 并入背景色；同形状、合法下标、无板外色
- [x] 5.2 分量珠数 > `min_region` → 保留不动
- [x] 5.3 目标平局取最小下标（构造两侧邻色等量）
- [x] 5.4 全图单色 → 原样返回；空 pattern（`cells.len()==0`）原样返回不 panic；`min_region==0` no-op；**两相邻异色单点（输入快照）序不变**（行优先与列优先扫描输出相同）
- [x] 5.5 确定性 + **跨架构位精确整数 golden**：固定小 `BeadPattern` + `min_region` → 硬编码期望 `cells`（整数、arm64/x86_64 都过）
- [x] 5.6 pipeline：`despeckle==None` 逐字节不变；`Some(s)` 时统计/渲染来自去斑后 pattern；与 `max_colors==Some(n)` 同用时最终不同珠色数仍 ≤ n

## 6. 端到端 + 文档

- [x] 6.1 端到端跑 UncleGao（`--despeckle 1~2`）目视：背景/脸上零星散珠减少、更干净；**并确认眼镜高光 / GAO 字 / 点睛细节未被过度吞**（若明显，调阈值 / 记为取舍）
- [x] 6.2 `cargo build` / `cargo test` / `cargo clippy --all-targets` 全绿
- [x] 6.3 同步 `ARCHITECTURE.md`（后段增可选 despeckle：连通域去斑、pattern→pattern、纯整数跨架构、永不发明色、opt-in 默认关）；**说明为何 despeckle 是自由函数而 reduce 是 `BeadReducer` trait**（axis 差异：despeckle 不碰色坐标 / 无 palette / 空间拓扑 vs 颜色数、`min_region==0` 是有意义 no-op；单实现不抽 trait，第三个空间步骤出现再考虑共享缝）；**despeckle 归为「库/复用原语、非生成入口」**（Rule 4）；`pattern-cleanup` 定位为**空间后处理能力族**（未来最小岛屿/平滑归入）
- [x] 6.4 golden：确认默认（`despeckle==None`）golden **不变**；`Some(s)` 路径纯整数，可加一份跨架构位精确整数单元 golden（非端到端 PNG）
