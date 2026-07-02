# image-grid 规范（增量）

## MODIFIED Requirements

### 需求:缩放到固定网格
`resize_image` 与 `image_to_grid` 必须把图像缩放到精确的 `width × height`（不保持宽高比——调用方先裁剪）。在 `RgbImage` 层用 `::image::imageops::resize`（始终精确，无宽高比保持）——注意 `DynamicImage::resize_exact` 不可用于 `RgbImage`（仅 `DynamicImage` 有），见 design D6′。**默认重采样滤镜为 `Triangle`**（低瓣/近面积平均：`image` crate 对降采样按缩放比放大滤波支持域，`Triangle` 无负瓣，天然吃掉源图细噪声、且不产生 `Lanczos3` 负瓣在锐边的振铃过冲）；`ResizeOptions.filter` 仍**可配**，可显式指定 `Lanczos3` 或其它滤镜。必须允许放大（目标大于源）。`resize_image` 作为公开原语（D3）可被直接调用：源图任一维度为 0（如外部手构 `0×N` 的 `RgbImage`）时必须返回 `InvalidImage`（reason 点名退化维度），**不得**让 0 维源喂进 `imageops::resize`——实测它对 0 维源静默返回 `Ok` + 全黑网格、不 panic，是与裁剪退化同源的 false-green。

#### 场景:产出精确尺寸网格
- **当** `image_to_grid(bytes, 80, 100, &ResizeOptions::default())`
- **那么** 返回 `Ok(PixelGrid)`，`width == 80`、`height == 100`、`pixels.len() == 8000`

#### 场景:允许放大
- **当** 源很小（如 4×4），目标较大（如 50×50）
- **那么** 返回 `Ok`，`pixels.len() == 2500`

#### 场景:默认滤镜为 Triangle 且可配回 Lanczos3
- **当** 用 `ResizeOptions::default()` 缩放
- **那么** 采用 `Triangle` 重采样；而当调用方显式设 `ResizeOptions.filter = Lanczos3` 时，采用 `Lanczos3`（默认值可被覆盖）

### 需求:确定性
同一字节输入与同一 `ResizeOptions` 必须产生逐字节相同的 `PixelGrid`（确定性是门，对齐 CLAUDE.md 硬规则 2 与 ROADMAP 的 M2+ 确定性门）。实现禁止引入非确定性来源（`rayon` 并行重采样、随机、迭代顺序泄漏）；重采样滤镜与裁剪偏移规则必须固定，锁 `Cargo.lock` 冻结编解码器。
- **跨架构口径（不弱化项目门，按 ROADMAP 既有分工）：** **默认 `Triangle` 是 `f32` 重采样**（取代原 `Lanczos3`，仍为 `f32`；显式 `Lanczos3` 亦然）；IEEE-754 下逐操作（无 FMA 收缩 / 无 x87 扩展精度）是确定的，故跨架构逐字节一致是**预期**，但其**校验**落在 M8 的「CLI == FFI」检查（ROADMAP 既定：M2 立门、M7 golden、M8 跨平台验证）。M2 不预先硬编码一份「赌它跨架构相同」的 `f32` 重采样 golden 值。
- **M2 测试如何证明（避免 CI 跨架构假阴）：** ①确定性由 `grid_is_deterministic` 的同进程重算比对证明（跨平台稳定，是真正的门）；②任何硬编码 golden 期望网格用**跨架构位精确**的方式断言——即对那一格用 `Nearest`（整数精确、无 `f32`）裁+缩，而非默认 `Triangle`（或 `Lanczos3`）的 `f32` 值。

#### 场景:重复产网格一致
- **当** 对同一字节、同一 `ResizeOptions` 多次调用 `image_to_grid`
- **那么** 每次返回的 `PixelGrid` 完全相等（含像素顺序）
