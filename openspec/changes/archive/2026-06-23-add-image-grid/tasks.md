## 1. 依赖（Cargo）

- [x] 1.1 根 `Cargo.toml` `[workspace.dependencies]` 加
  `image = { version = "0.25", default-features = false, features = ["png", "jpeg", "webp"] }`
- [x] 1.2 `crates/bead-core/Cargo.toml` 加 `image.workspace = true`

## 2. bead-core：错误模型

- [x] 2.1 `crates/bead-core/src/lib.rs`：给 `BeadError` 增加
  `ImageDecode(#[from] ::image::ImageError)` 与 `InvalidImage { reason: String }`
  两个变体（enum 已 `#[non_exhaustive]`），写 `thiserror` 文案。
  **必须用前导 `::image::`**（或 `use ::image::ImageError;`）——`lib.rs` 含 `pub mod image;`，
  裸 `image::ImageError` 会解析到本地模块而编译失败（见 design D6 编译陷阱）

## 3. bead-core：models 模块

- [x] 3.1 新建 `crates/bead-core/src/models/mod.rs`，定义公开
  `PixelGrid { width:u32, height:u32, pixels:Vec<[u8;3]> }`（derive `Debug+Clone+PartialEq`，
  **不 derive `Eq`**，见 design D1）；文档注明行优先 `pixels[y*width+x]`、`pixels.len()==w*h`
- [x] 3.2 `lib.rs`：`pub mod models;` + 重导出 `PixelGrid`

## 4. bead-core：image 模块

- [x] 4.1 新建 `crates/bead-core/src/image/mod.rs`，定义公开
  `ResizeOptions { filter: ::image::imageops::FilterType }` + `impl Default`（`Lanczos3`），
  重导出/使用 `FilterType`（接受 image 类型进公开签名，见 design D3）。
  **`image/mod.rs` 内引用外部 crate 一律用 `::image::` 或顶部 `use ::image::{...}`**——本地
  模块同名 `image`，裸路径可能经 crate 根解析到本地模块（见 design D6）
- [x] 4.2 `image/mod.rs`：`decode_image(&[u8])->Result<RgbImage,BeadError>`——
  `::image::load_from_memory`（自动嗅探，前导 `::` 见 D6）→ `to_rgb8` 归一（丢 alpha；
  Luma8/Rgb16/palette 统一 `[u8;3]`，见 D4）；解码失败经 `#[from]` → `ImageDecode`
- [x] 4.3 `image/mod.rs`：`crop_center(&RgbImage, tw, th)->Result<RgbImage,BeadError>`——
  中心裁剪用 `::image::imageops::crop_imm(..).to_image()`（见 D6′），偏移 `floor(差/2)`、多余
  给右/下；源比**恰**==目标比 → no-op。**自守卫（公开原语，可被直接调用，见 D5′ 维度守卫总则）：**
  ①函数开头先校验目标 `tw==0||th==0` → `InvalidImage`（退化目标比）；②算出裁剪矩形后校验源
  任一维度为 0 或裁剪宽/高 floor 到 0 → `InvalidImage`（reason 点名退化维度），不产 0 维中间图、
  不 panic（实测已复现全黑 false-green）。比值与裁剪尺寸用整数交叉相乘 `src_w*th` vs `src_h*tw`
  （不靠 `tw/th` 除零），**所有交叉积/尺寸积先 `as u64` widening**（防 `u32` 溢出：源 `100000×1`、
  目标 `1×100000` 会让 `u32` 积 `1e10` 溢出 → debug panic/release wrap，违反不-panic 契约），
  校验非 0 后 checked-cast 回 `u32`
- [x] 4.4 `image/mod.rs`：`resize_image(&RgbImage, w, h, &ResizeOptions)->Result<PixelGrid,BeadError>`——
  缩放用 `::image::imageops::resize`（始终精确，**注意 `resize_exact` 仅 `DynamicImage` 有、
  `RgbImage` 无**，见 D6′）。**自守卫（公开原语，见 D5′）：目标 `w==0||h==0` **或**源图任一维度
  为 0（`src.width()==0 || src.height()==0`，任一即拒、非同时）→ `InvalidImage`（reason 点名
  维度）**，必须在调 `imageops::resize` 之前挡掉（实测它对 0 维源静默 `Ok`+全黑）；缩到精确
  `w×h`（用 `options.filter`）；放大允许；产 `PixelGrid`（`pixels.len()==w*h` 用 `usize` 算，行优先）
- [x] 4.5 `image/mod.rs`：`image_to_grid(&[u8], w, h, &ResizeOptions)->Result<PixelGrid,BeadError>`——
  **先校验目标 `w/h`（`w==0||h==0` → `InvalidImage`，须在 `crop_center` 以目标算比除零之前）**
  → decode → crop_center → resize_image 一条龙（守卫顺序见 D5′）
- [x] 4.6 `lib.rs`：`pub mod image;` + 重导出
  `decode_image / crop_center / resize_image / image_to_grid / ResizeOptions`；
  在 `image/mod.rs` 留 `ponytail:` 注释说明 EXIF 朝向不自动套用（见 D9）

## 5. 测试夹具（确定性来源）

- [x] 5.1 用一次性脚本生成并提交小尺寸**确定性**夹具到
  `crates/bead-core/src/image/fixtures/`（供 `include_bytes!`）：①脚本生成的小 PNG 渐变（golden 源，无损）
  ②透明 RGBA PNG ③Luma8 灰度 PNG ④Rgb16 16 位 PNG ⑤调色板 PNG ⑥小 JPEG ⑦小 WEBP
  ⑧带 EXIF 朝向标签的 JPEG。脚本不入库；夹具字节入库。**golden 源用 PNG（无损稳定），
  JPEG/WEBP 仅作解码冒烟**。
  **⑧ 注意：普通编码不写 EXIF——生成脚本须调 `image` 的 `set_exif_metadata` 写一个带朝向
  tag 的 EXIF APP1 字节块（见 D9）；产出的 JPEG 字节入库（不引入真照片）**

## 6. 测试（映射 Done-when + 边界）

- [x] 6.1 `image/mod.rs` `#[cfg(test)]`：`grid_has_exact_cell_count`
  （`image_to_grid(png,80,100,&default)` → `len()==8000`、`width==80`、`height==100`）— Done-when
- [x] 6.2 `grid_is_deterministic`（同字节+同选项两次 `PixelGrid` 相等）= **确定性的真正门**
  （同进程重算比对，跨平台稳定）。**inline golden 期望网格须跨架构位精确**：对那一格用
  `Nearest`（整数精确、无 `f32`）裁+缩并断言内联 `vec![[r,g,b],…]`——**不要**硬编码
  `Lanczos3` 的 `f32` 输出（dev=arm64 生成的值可能在 x86_64 CI 失败，见 D8）；`Lanczos3`
  默认值另由 6.9 单独钉（`// ponytail: 完整 golden-file 框架推迟 M7；确定性靠重算比对 +
  Nearest 位精确 golden，跨架构稳定`）— Done-when
- [x] 6.3 `decode_png` / `decode_jpeg` / `decode_webp`（各 `Ok`、尺寸对）— 格式覆盖
- [x] 6.4 `decode_rejects_garbage`（`b"not an image"`→`ImageDecode`）、`decode_rejects_unsupported_format`
  （GIF/BMP/TIFF 字节→`ImageDecode`，证明只编入 png/jpeg/webp）；**只断言变体，不断言 image Display 文案**
- [x] 6.5 归一：`alpha_is_dropped_deterministically`、`luma8_normalized_to_rgb`（`r==g==b`）、
  `rgb16_normalized_to_rgb8`（不 panic）、`palette_png_expanded`（已知像素）
- [x] 6.6 裁剪：`crop_center_odd_split_biases_right_bottom`（哨兵像素验偏移）、
  `crop_center_landscape_to_square`（宽==高）、`crop_center_already_matching_ratio_is_noop`
- [x] 6.7 边界：`upscaling_is_allowed`（4×4→50×50=2500）、`zero_width_rejected`、`zero_height_rejected`
  （`resize_image` 目标 0；`InvalidImage`，**且断言 `reason` 含 width/height 字样、重复调用
  `reason` 逐字符相同**——spec「无效操作错误」要求 reason 确定性点名维度）、`one_by_one_target_ok`
  （1 格）、`image_to_grid_zero_target_rejected`（`image_to_grid(bytes,0,10,..)`/`(..,10,0,..)` →
  `InvalidImage`，在 `crop_center` 除零之前命中，不 panic）。
  **以下四个直接调用公开原语（不经 wrapper），钉「每原语自守卫」（D5′；CR/Codex/RC round-2~4）：**
  - `crop_center_zero_target_rejected`：**直接** `crop_center(&img, 0, 10)` / `(&img, 10, 0)` →
    `InvalidImage`，不除零 panic；
  - `crop_center_extreme_ratio_rejected`：**直接**对解码出的 `100×1` `RgbImage` 调
    `crop_center(.., 1, 100)`，**及** `1×100` 源调 `crop_center(.., 100, 1)`（镜像方向）→
    各 `InvalidImage` 点名退化维度，不产全黑、不 panic；
  - `resize_image_zero_source_rejected`：**直接**对手构 `0×5` / `5×0` 的 `RgbImage` 调
    `resize_image(&z, 80, 100, ..)` → `InvalidImage`，不静默产全黑网格；
  - `crop_center_extreme_aspect_no_overflow`：**直接**对 `100000×1` 源调 `crop_center(.., 1, 100000)`
    **及**镜像 `1×100000` 源调 `crop_center(.., 100000, 1)`（交叉积 `1e10` 会溢出 `u32`）→ 各
    `InvalidImage`，**debug 不 panic、release 不 wrap 选错路径**（钉 `u64` widening；Codex round-4）
- [x] 6.8 `exif_orientation_not_applied`（带朝向 JPEG 按存储解码，钉住当前行为，见 D9）
- [x] 6.9 `default_filter_is_lanczos3`（`ResizeOptions::default().filter == FilterType::Lanczos3`）

## 7. 收尾验证 + 文档

- [x] 7.1 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 全绿
- [x] 7.2 确认 `bead-core` 仍无文件系统/UI/平台依赖（`image` 为图像处理依赖，非平台/fs）；
  确认 `image` 未引入 `rayon`（`cargo tree` 核对）；提交更新后的 `Cargo.lock`
- [x] 7.3 校正 `ROADMAP.md` M2：`models` 行与 Done-when 的 `BeadPattern` → `PixelGrid`
  （`width×height` cell 不变量保留）；M3 处补「M3 引入 BeadCell/BeadPattern 并把 PixelGrid 映射进去」
- [x] 7.4 校正 `ARCHITECTURE.md`（命名的内部真理源，勿只改 ROADMAP 留 drift）：
  ①`image` 模块块加 `pub fn image_to_grid(...)`；②Data Model Layer 加 `PixelGrid` 说明
  （M2 过渡性原始 RGB 中间体，M3 由匹配器映射进 `BeadPattern`）；③Rendering Strategy 注明
  配色前 `PixelGrid` 是真理源、配色后才是 `BeadPattern`。proposal「影响→文档」同步加
  `ARCHITECTURE.md`
