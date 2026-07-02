# 设计：去孤立点 / 小连通域清理（despeckle）

## 上下文

配色（`match_pattern`）+ 可选珠色减色（`GreedyReducer`）后的 `BeadPattern` 常残留空间孤立的小斑块（单/双颗散珠）。`GreedyReducer` 只按**用量**全局合并颜色种类，不看空间连通性——散珠存活。despeckle 是 `BeadPattern` 级的**空间**清理，与 `GreedyReducer` 的**颜色数**缩减正交。约束：`bead-core` 无 UI/FS、确定性（同机逐字节 + 跨架构整数位精确 + CLI==FFI）、Phase 1 单线程、`generate_pattern` 唯一入口、永不发明色。

## 目标 / 非目标

**目标：**
- 减少孤立散珠 / 小碎块，让实体拼豆更干净、好摆。
- opt-in、默认关 → 默认输出逐字节不变、golden 稳定。
- 纯整数、确定性、跨架构位精确；永不发明中间色。
- 对 Staged 与未来 Gerstner 两路都生效。

**非目标：**
- 不改默认输出（默认 `None`）；不做平滑/抗锯齿/边缘增强；不引入抖动；不并行（Phase 1）；不扩 FFI/移动端入参；不追求「保住所有细节」（清理与保细节本质对立，交由阈值 + opt-in 让用户权衡）。

## 决策

**D1：opt-in `despeckle: Option<u32>`，默认 `None`。**
`None`=不清理，默认路径逐字节不变、golden 稳定；`Some(s)`=清理珠数 ≤ `s` 的同色连通分量。沿用 `max_colors: Option<u32>` 的 opt-in 风格。
- *替代*：默认开 + 保守阈值（如 1）。否决——改默认输出、动 golden、且「杀单颗」对某些图（点睛高光=单颗）有害；让用户显式开更稳。

**D2：pattern→pattern 独立函数，不新增 trait。**
`despeckle(pattern: &BeadPattern, min_region: u32) -> BeadPattern`（同形状，置于 `cleanup` 模块、crate 根重导出 `despeckle`）。pipeline 后段一个可选步骤调用它。命名统一为 **`despeckle`**（spec/pipeline/tasks 一致，不用 `clean`）。
- *替代*：套 `BeadReducer` trait 或新 `PatternCleaner` trait。否决——单一算法、无第二实现，object-safe 缝是投机（YAGNI）；与 `GreedyReducer` 语义不同（空间 vs 颜色数），硬塞一个 trait 反而混淆。

**D3：4-连通 + 多数邻色重映射，输入快照语义（order-independent）。**
行优先扫描找**同色 4-连通分量**；分量珠数 ≤ `min_region` 时，整块重映射到**多数邻色**——**逐边界邻接边计票**：分量内格 × 其 4-邻中的**异色**外侧格，每条跨界边给该外侧珠色一票，取票数最多者；平局取**调色板最小下标**。并入色必是**已存在的相邻珠色**（永不发明）。分量无异色邻居（仅当全图单色）→ no-op。
- **执行模型 = 输入快照**：分量发现与计票**全对输入快照**、重映射写**独立输出缓冲**、**单遍**——先前重映射不影响后续分量计票，故**与扫描顺序无关**、输出由输入拓扑唯一决定（这也与「并入色是输入中已存在的相邻珠色」自洽）。*替代*：就地 mutate（行优先，先扫者赢）。否决——序相关、难声明式推理，snapshot 更干净且 review 一致推荐。
- *替代*：8-连通 / 「最近感知色」替换。否决——4-连通更保守、边界更直觉；「多数邻色」比「最近感知色」更贴「融进周围」的目视目标，且纯整数（无 f32）。

**D4：单遍、确定性、纯整数。**
单遍扫描（v1 不迭代到稳定）+ 输入快照（见 D3）→ 输出序不变、由输入唯一决定；固定 4-连通、固定逐边计票、平局取小下标；无随机 / `rayon` / hash 顺序泄漏。全程整数（下标与计数）→ **跨架构位精确**，可钉整数 golden（区别于 Oklab/Triangle 的 f32 同机档）。**不声明幂等**：`despeckle∘despeckle` 未必等于 `despeckle`（单遍不迭代到稳定；合并后边界可能生成新小分量）——这是有意取舍，不作幂等承诺。

**D5：位置 = 减色之后、统计/渲染之前。**
`image_to_grid → match_pattern →（可选 GreedyReducer）→（可选 despeckle）→ count/summary/render`。理由：减色可能新产生小碎块，despeckle 收尾最干净；且统计/渲染基于**清理后**的 `BeadPattern`。despeckle 只会**减少或持平**不同珠色数（并入邻色），故 `max_colors` 的 ≤N 仍成立。

**D6：`min_region` 语义与守卫。**
`min_region` = 触发清理的连通域**最大珠数**（`Some(s)` → 清理 ≤ `s` 珠的分量）。`Some(0)` = 不清理任何分量（等价 no-op，**不**报错——0 珠的分量不存在）；亦可由 CLI 归一为「不传即 None」。`despeckle` 是全函数（合法 pattern 下不 panic，前置条件同 `match_pattern` 输出：`cells` 合法下标）。

## 风险 / 权衡

- [过大阈值吃掉想要的细节：眼神光 / 点睛色 / 首饰 = 小分量] → opt-in + 保守阈值 + 文档写明；验收目视（UncleGao 眼镜高光 / GAO 字）确认未被过度吞。
- [单遍不彻底：清理后边界可能生成新小分量] → v1 单遍（确定、够用）；迭代到稳定留 Open Q（要防振荡 + 定收敛）。
- [多数邻色平局 / 复杂边界] → 逐边计票 + 平局取最小下标固定，确定性有保证。
- [**全小分量拓扑**（不止 `min_region ≥ 图像总珠数`——如 2 色棋盘在 `min_region=1` 即每格都是合格小分量）→ 整体换色/反转，**不是**「塌成一色」（棋盘按输入快照多数邻色重映射后仍是棋盘、只是配色互换）] → 属去斑对「非孤立点、而是密集小分量」输入的固有行为；无内置上界，由 opt-in + 文档「阈值应保持很小（1~2）、去斑针对孤立散点而非纹理」缓解，不设硬 cap。
- [**forward-fragile**：`--despeckle` 的输出 FFI 当前无法复现（FFI 不暴露 despeckle）→ 削弱「CLI==FFI 单一 GenerateResult」对非默认输入的覆盖] → 可接受（FFI 是 M8、despeckle opt-in、CLI==FFI 门只比默认路径）；但未来 FFI 要么暴露 despeckle、要么对非默认输入永久分叉——留 flutter-ffi 需求明确时定。（narrative-drift/forward-fragility 属 prose loop 不保证覆盖的分布，此处显式记录。）

## 验收发现（§6.1 端到端目视，2026-07-02）

UncleGao（平涂卡通，80×80 + oklab + max-colors 24）实测：`--despeckle 1` 改 12.3% 格、`--despeckle 2` 改 17.3% 格，**两者都目视变差**——眼镜框 / 眼睛 / 文字等**细薄特征本身就是 ≤2 珠的小分量**，被算法按规格并入邻色，越清越糊。这**证实**了「阈值吃细节」风险，且印证定位：**despeckle 是照片噪点工具，不适合平涂卡通**（卡通的小分量是有意的细线，非噪点）。实现本身正确（确定性、纯整数、单测全绿、`0` 为 no-op）——问题是**适用场景**，非缺陷。结论：默认关（`None`）保护卡通用户；despeckle 的收益在**照片/噪声型缩减结果**上（其既定目标），edge-aware 精修留 Gerstner/后续。`--despeckle 0` 与不给逐字节一致（no-op 已验证）。

## 迁移计划

无历史用户。默认 `None` → 默认输出逐字节不变、golden 不动。回滚=还原提交。与 `gerstner-pixel-abstraction` 并行独立（despeckle 作用于最终 `BeadPattern`、两路通用）；**注意**：本变更与 Gerstner 都改 `GenerateOptions`（本加 `despeckle`、Gerstner 加 `generator`）——**despeckle 先归档**，Gerstner 归档时其 `GenerateOptions` MODIFIED 块需含 `despeckle` 字段（rebase 到含 despeckle 的主规范）。

## 待解问题

- 4- vs 8-连通（本 design 取 4）。
- 替换色：多数邻色（本 design 取）vs 最近感知色 vs 最大相邻同色块。
- 单遍 vs 迭代到稳定（本 design 取单遍；迭代需防振荡）。
- `min_region` 是否需要按目标尺寸自适应，还是纯绝对珠数。
- 是否给 Staged 默认开一个极保守阈值（本 design 全 opt-in）。
