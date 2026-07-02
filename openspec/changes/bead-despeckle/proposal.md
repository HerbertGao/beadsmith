# 提案：去孤立点 / 小连通域清理（despeckle）

## 为什么

实体拼豆的观感与「好摆」不只看颜色准，更看**图案是否碎**：配色/减色后常残留**空间上孤立的单颗/双颗散珠**（脸上一粒杂色、背景里零星异色点）——远看脏、近看碎、按图摆珠时极易摆错漏摆。现管线的 `GreedyReducer` 减的是**颜色种类数**（全局按用量合并），**不处理**空间孤立的小斑块：一个用量不低但散成许多孤点的颜色会整存下来。两方算法讨论都把「去孤立点 / 小连通域清理」放进**首选默认组合**，且它比 Gerstner 便宜得多、对 Staged 与 Gerstner 两条生成路都受益——是当前 ROI 最高、风险最低的一步。

## 变更内容

- 新增**可选**的 pattern→pattern **连通域去斑**后处理：把**同色连通分量中珠数 ≤ 阈值**的小斑块，整块**重映射到相邻珠色中边界接触最多者**（多数邻色），减少孤立散珠。
- **opt-in、默认关**：`GenerateOptions` 增 `despeckle: Option<u32>`（默认 `None`=不清理，**默认输出逐字节不变、golden 稳定**）；给 `Some(s)` 时清理珠数 ≤ `s` 的连通域。
- **永不发明中间色**：并入的是**邻域已存在的珠色**（延续 palette-aware-reduction 原则）；不新增珠板外颜色。
- **纯整数、确定性**：固定行优先扫描 + 4-连通 + 边界邻色计数 + 平局取最小下标；无随机、无 `rayon`、无 hash 顺序泄漏——**跨架构位精确**（可钉整数 golden，不同于 Oklab/Triangle 的 f32 同机档）。
- 位于 pipeline **后段**：`match_pattern`（+ 可选 `GreedyReducer`）**之后**、统计/渲染**之前**，作用于最终 `BeadPattern`；对 Staged 与未来 Gerstner 两路都生效。

## 功能 (Capabilities)

### 新增功能
- `pattern-cleanup`: `BeadPattern` 级的**连通域去斑**——定义「小同色连通分量 → 并入多数邻色」的确定性、纯整数、永不发明色的清理算法，及其阈值语义与形状不变量（同形状、`cells` 均合法下标）。

### 修改功能
- `pipeline`: `generate_pattern` 后段增加**可选 despeckle 步骤**（`max_colors` 减色之后、统计/渲染之前）；`GenerateOptions` 增 `despeckle: Option<u32>`，`Default` 为 `None`（默认路径逐字节不变）；仍是唯一编排入口、不内联算法。
- `cli`: `generate` 新增可选 flag `--despeckle <N>`（默认不清理）。

## 影响

- **bead-core**：新增 `pattern-cleanup` / 连通域去斑模块（pattern→pattern 独立函数，不新增 trait——单一算法，YAGNI）；`pipeline` 后段加可选步骤；`GenerateOptions` 增字段。
- **CLI**：`generate` 增 `--despeckle <N>`（`u32`；`Some(0)` 语义与拒绝策略见 spec）。
- **门**：`despeckle=None` 默认路径**逐字节不变**（golden 稳定）；`Some(s)` 路径纯整数、可钉**跨架构位精确** golden；CLI==FFI 门只比默认路径（FFI 不暴露 despeckle）。
- **权衡**：过大阈值会**吃掉想要的细节**（眼神光、点睛色、首饰）——故 opt-in + 保守阈值 + 文档说明（与 least-used 合并同类取舍）。
- **依赖**：无新增。
