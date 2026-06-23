## 为什么

里程碑 **M2 — Resize + Pattern Grid**。引擎要先把源图变成一个固定尺寸的拼豆网格
（1 像素 = 1 颗豆，尚不做配色），M3 的匹配器才能在此基础上把每格映射到真实豆色。
M1 给了调色板；M2 给出 M3 要消费的「原始像素网格」，并把图像 I/O 边界和第一批
数据模型（`models`）定下来。

## 变更内容

- `bead-core` 新增 `image` 模块：从图像字节解码 → 归一 → 中心裁剪 → 缩放 → 网格。
  - `decode_image(&[u8]) -> Result<RgbImage, BeadError>`：自动嗅探 PNG/JPEG/WEBP，**解码并
    归一**到 RGB8（丢 alpha；Luma8 / Rgb16 / palette-PNG 经 `to_rgb8` 归一为 `[u8;3]`）。
  - `crop_center(&RgbImage, tw, th) -> Result<RgbImage, BeadError>`：中心裁剪到目标宽高比，
    奇数像素差给右/下；自守卫退化维度（目标 `tw==0||th==0`、源维度为 0、裁剪 floor 到 0 维）
    → `InvalidImage`，不除零 / 不产 0 维（见 design D5′）。
  - `resize_image(&RgbImage, w, h, &ResizeOptions) -> Result<PixelGrid, BeadError>`：
    `::image::imageops::resize` 到精确 `w×h`（默认 Lanczos3；`resize_exact` 仅 `DynamicImage`
    有，见 D6′）；`w==0||h==0` 或 0 维源 → `InvalidImage`。
  - `image_to_grid(&[u8], w, h, &ResizeOptions)`：先校验目标维度 → decode→crop→resize 的
    **库内 / pipeline 复用原语**（**非** FFI 入口——FFI/CLI 走 `pipeline::generate_pattern`，
    CLAUDE.md 规则 4，见 design D3）。
- 新增 `models` 模块与 `PixelGrid { width, height, pixels: Vec<[u8;3]> }`（行优先原始
  RGB 网格，`pixels.len() == width*height`）。
- `BeadError` 增加 `ImageDecode(#[from] ::image::ImageError)` 与 `InvalidImage { reason }`
  （前导 `::`：本地模块与 crate 同名 `image`，见 design D6）。
- 新增依赖 `image`（特性裁剪，见「影响」）。
- 校正 ROADMAP M2 措辞：「produces a `BeadPattern`」→「produces a `PixelGrid`」
  （`width×height` cell 数量不变量保留），并在 M3 处补充「M3 引入 `BeadCell`/
  `BeadPattern` 并把 `PixelGrid` 映射进去」。

> 契约说明：`BeadCell.color_index` 是调色板下标，`BeadPattern` 需要调色板/统计 ——
> M2 都还没有。故 M2 的输出是 typed 的原始 RGB `PixelGrid`，`BeadCell`/`BeadPattern`
> 推迟到 M3。这与 ARCHITECTURE 的 Core Workflow（Pattern Grid 在配色之后）一致。

## 功能 (Capabilities)

### 新增功能
- `image-grid`: 从图像字节（PNG/JPEG/WEBP）解码、归一、中心裁剪、缩放为固定 `w×h`
  的原始 RGB 网格 `PixelGrid`；约定确定性的解码/裁剪/缩放行为。

### 修改功能
<!-- 无：M2 不改动已生效的 palette 规范。 -->

## 非目标（Non Goals）

按 YAGNI 推迟 / 不做：

- `BeadCell` / `BeadPattern` / `ColorStat` → M3 / M4（尚无调色板、匹配器、统计）。
- 颜色量化 / 配色匹配 → M3 / Phase 2（算法档位）。
- EXIF 朝向自动旋转 → 推迟（未证明必要）。措辞校正：`image 0.25` **能**暴露朝向元数据
  （`ImageDecoder::orientation`），但 `load_from_memory` **不自动套用**；M2 即依赖这一默认，
  解码即存储朝向，留 `ponytail:` 注释。
- alpha 展平到背景色 → 推迟（M2 直接丢 alpha）；将来确有需求再加 `background` 选项。
- 可配置的放大策略（拒绝 / 警告）→ M2 静默允许放大。
- 完整 golden-file 测试框架 → M7；M2 用脚本生成的小 PNG + 内联期望网格即可。
- 手动 / 交互式裁剪 → 前端职责；core 只做中心裁剪。
- `rayon` 并行 → Phase 2（ARCHITECTURE 性能策略）。
- 解压炸弹 / 内存上限（`image::Limits`）→ 后续加固里程碑；M2 用默认。

## 影响

- **代码**：
  - `crates/bead-core/src/lib.rs`（改：`pub mod image; pub mod models;` + 重导出 + 两个 `BeadError` 变体）
  - `crates/bead-core/src/image/mod.rs`（新）、`crates/bead-core/src/models/mod.rs`（新）
  - `crates/bead-core/Cargo.toml`、根 `Cargo.toml`（加依赖）
- **依赖（新增，需理由）**：
  - `image = { version = "0.25", default-features = false, features = ["png","jpeg","webp"] }`，
    加到 `[workspace.dependencies]`，bead-core 以 `.workspace=true` 引用。
  - 理由：唯一在单一 `load_from_memory` API 后同时解码 PNG+JPEG+WEBP 并自带高质量重采样
    （`imageops`）的 crate。单编解码器 crate 无法覆盖三种格式。`default-features=false`
    砍掉 avif/gif/tiff/bmp/… **与 `rayon`**，保证单线程确定性。
- **确定性**：无 `rayon`（单线程重采样）、固定 `Lanczos3`、固定裁剪偏移规则、统一
  `to_rgb8` 归一、解码即存储（无 EXIF 非确定性）；锁定并提交 `Cargo.lock` 冻结编解码器
  补丁版本。同字节输入 → 逐字节相同 `PixelGrid`（确定性门，对齐 CLAUDE 规则 2 / ROADMAP）。
  `Lanczos3` 的 `f32` 重采样跨架构一致是**预期**（IEEE-754 逐操作确定），其校验按 ROADMAP
  既有分工落在 M8「CLI == FFI」——M2 不弱化项目门，也不硬编码跨架构 `Lanczos3` golden
  （golden 用 `Nearest` 位精确断言，见 design D8 / tasks 6.2）。
- **里程碑 / Phase**：里程碑 M2；不涉及算法 Phase（尚无配色匹配）。
- **文档**：ROADMAP M2 的 `BeadPattern`→`PixelGrid` 措辞校正（见「变更内容」）；
  `ARCHITECTURE.md` 同步（`image` 模块加 `image_to_grid`；Data Model Layer 加 `PixelGrid`
  说明；Rendering Strategy 注明配色前 `PixelGrid` 是真理源）——勿只改 ROADMAP 留 drift（见 tasks 7.4）。
