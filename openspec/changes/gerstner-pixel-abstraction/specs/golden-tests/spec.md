# golden-tests 规范（增量）

## 新增需求

### 需求:Gerstner 路径的合成夹具确定性 golden
golden **必须**为 `Gerstner` 生成路径提供一份**合成小夹具** golden：手构一张极小（如 `8×8`）双色 / 渐变 `RgbImage`（**禁止** committed 二进制照片夹具）、固定 `palette`、固定设置（`width`/`height`/`generator == Gerstner`/其余默认），把产出冻结为守卫。**canonical 平台（arm64 Linux，同既有 golden 的 gate）**断言 `cells`（或四产物字节）**逐一相等**；**非 canonical 平台**断言**浮点无关的结构不变量**（`cells.len()==w*h`、每个下标 `< palette.colors.len()`、若干**已知 cell 下标**、不同珠色数）。此 golden **守住 Gerstner 的确定性机制**（实数 per-axis 步长 / round-0 质心 / 原始网格锚定候选 / 快照式更新 / 固定累加序 / 空簇保留），因为「同机重算逐字节」单测只证**自洽**、证不了**结构回归**（如累加序被悄改、平局翻转仍逐轮自洽却改了输出）。默认 `Staged` 路径 golden **不受影响、逐字节不变**。

#### 场景:Gerstner 合成夹具产确定 golden
- **当** 用固定合成极小夹具 + 固定 `Gerstner` 设置运行 golden 测试
- **那么** canonical 平台产物**逐字节等于** committed 合成 golden；非 canonical 平台**结构不变量**（形状 / 下标合法 / 若干已知 cell / 珠色数）全过；**改变任一 Gerstner 确定性设置**（`T`/`m`/`width`/`height`/`palette`）即与 golden 不符
