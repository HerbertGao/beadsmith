## 上下文

里程碑 M2。M1 已建 `palette`。M2 在 `bead-core` 新增 `image` 模块与第一批 `models`：
把源图字节解码、归一、中心裁剪、缩放成固定 `w×h` 的原始 RGB 网格 `PixelGrid`，供 M3
匹配器消费。约束：纯库、确定性、`thiserror + Result<T, BeadError>`、字节进数据出。

设计在探索阶段收敛并冻结（基于实测 `image 0.25.10` API）。公开 API 表面采用「架构师版
（更灵活）」：三原语 + 便捷入口全公开，接受 `image::RgbImage` / `FilterType` 进公开签名。

## 目标 / 非目标

**目标：** `decode_image` / `crop_center` / `resize_image` / `image_to_grid` + `ResizeOptions`；
`PixelGrid` 输出；PNG/JPEG/WEBP；确定性。

**非目标：** `BeadCell`/`BeadPattern`/`ColorStat`（M3/M4）、配色匹配（M3）、EXIF 旋转、
alpha 展平、放大策略、完整 golden 框架（M7）、rayon（Phase 2）、内存上限。

## 决策

**D1 — 输出类型：`PixelGrid`，不建 `BeadCell`/`BeadPattern`。** 公开
`PixelGrid { width:u32, height:u32, pixels:Vec<[u8;3]> }`（行优先 `pixels[y*width+x]`，
derive `Debug+Clone+PartialEq`，**不 derive `Eq`**）。
- 替代方案：撑大 `BeadPattern` 在 M2 装原始 RGB。否决理由：`BeadCell.color_index:u16`
  是调色板下标，M2 无调色板；ARCHITECTURE Core Workflow 把 Pattern Grid 置于配色之后。
  原始 RGB 中间体是诚实模型，避免「在 index 字段里塞原始色」的 hack。
- **`PixelGrid` 是过渡 / 内部中间体，不是最终结果。** M3 由匹配器消费 `PixelGrid` 产出
  `BeadPattern`（结构不同：`Vec<[u8;3]>` vs `Vec<BeadCell{x,y,color_index}>`），`PixelGrid`
  不再返回给外部调用方；稳定的对外结果仍是 `BeadPattern`（对齐 ARCHITECTURE「BeadPattern
  是真理源」）。M2 阶段它是配色前的真理源。故 `image_to_grid → PixelGrid` 是 M2 便捷面，
  不是「永久保留」的承诺（见 D3）。
- **不 derive `Eq` 的理由**（同 palette `PaletteColor` 的取舍）：`Eq` 是一项公开 API 承诺，
  YAGNI 推迟到真有 `HashMap` 键需求时再加（加 `Eq` 非破坏，去掉才破坏）。`PixelGrid` 字段
  虽全 `Eq`，仍保持与 `PaletteColor` 一致的最小派生面。
- **`pixels.len() == width*height` 不变量的归属：** 由 `resize_image` 保证；但字段是 `pub`、
  D3 又鼓励原语独立复用，故外部可构造违例 `PixelGrid`，而 M3 匹配器会按
  `pixels[y*width+x]` 信任该不变量。M2 采**惰性方案 (a)**：保留 `pub` 字段 + 文档声明
  「不变量由 `resize_image` 维护；外部构造方须自证 `len==width*height`，否则下游取格逻辑
  错误」。`ponytail:` 升级路径——若复用面变广，改私有字段 +
  `pub fn new(..) -> Result<PixelGrid, BeadError>` 校验不变量。下标 / 长度运算一律 `usize`
  （`width as usize * height as usize`），不用 `u32` 乘加以防大网格溢出。

**D2 — 缩放滤镜：`Lanczos3` 默认，经 `ResizeOptions { filter: FilterType }` 可配。**
`Default for ResizeOptions` = `Lanczos3`。
- 替代方案 a：固定 Lanczos3、不要 options 结构。否决理由（按用户选定的「更灵活」）：
  options 结构是无破坏扩展的接缝，且 Done-when 需要可配置入口。
- 替代方案 b：默认 Nearest。否决理由：下采样质量差（细线丢失）；M3 反正最近色匹配，
  Lanczos3 的中间色不是问题。

**D3 — 公开 API 表面：三原语 + 便捷入口全 `pub`。** `decode_image`/`crop_center` 在签名
里出现 `image::RgbImage`，`ResizeOptions.filter` 是重导出的 `image::imageops::FilterType`。
- 替代方案：原语降为 `pub(crate)`、只公开 `image_to_grid`+`PixelGrid`+`ResizeOptions`，
  甚至用内部 `enum BeadFilter` 替代 `FilterType` 杜绝泄漏。否决理由：用户明确选「更灵活」
  版以换取原语可独立复用/测试；接受 `image` 类型泄漏为内部细节耦合。
- 记录权衡：公开签名耦合 `image` crate 版本；FFI（M8）暴露滤镜时需把 `FilterType` 映射成
  Dart 侧表示 —— 届时再处理，非 M2 问题。
- **与 CLAUDE.md 规则 4（`pipeline::generate_pattern` 是外部调用方唯一入口）的关系：**
  这四个 `pub` 原语（含 `image_to_grid`）是**库内 / pipeline 复用原语**，不是被认可的外部 /
  FFI 入口。`pipeline::generate_pattern`（M6 落地）一旦存在即为 CLI / FFI 的规范入口；
  `image_to_grid → PixelGrid` 仍是低层便捷面，M3 后**降级于** `generate_pattern` 之下，不承诺
  作为 FFI 的对接点。proposal 里「pipeline/FFI 用」的表述按此口径修正，避免 M3 误继承「永久
  保留这条 FFI 入口」的义务。

**D4 — 归一：一律 `to_rgb8()`。** alpha 丢弃（非展平）；Luma8→灰度 RGB；Rgb16→8bit；
palette-PNG→展开 RGB。单一确定性漏斗。
- 替代方案：alpha 展平到背景色。否决理由：需要背景色策略，M2 未证明必要（YAGNI）；
  豆子不透明，丢弃后的 RGB 仍是 M3 可匹配的真实色。将来加 `ResizeOptions.background` 即可。
- **`decode_image` = 解码 **+** 归一**，名字略低估了它做的有损变换（丢 alpha、16→8 降位）。
  M2 不改名（避免 churn），但其文档注释必须首句点明「Decodes **and normalizes to RGB8**
  (drops alpha, downsamples 16-bit)」，让 CLI / FFI 绑定时不被「decode」一词误导。诚实改名
  `decode_to_rgb8` 记为 deferred 选项，M2 不做。
- `to_rgb8()` 对 `image::ColorType` 是穷尽的——上面列的 4 类只是代表，实际还会面对
  `LumaA8`/`Luma16`/`Rgba16` 等，都走同一漏斗归一，行为正确（声明集 ⊊ 有效集但安全）。
  **嵌入的 ICC 配置不被套用**：得到原始解码值（确定性），与色彩管理查看器可能有肉眼色差——
  这与「JPEG/WEBP 仅冒烟、PNG 才是 golden 源」的取舍一致，非 bug。

**D5 — 裁剪 / 边界规则。**（a）中心裁剪偏移用整数 floor，多余像素给右/下；（b）放大静默
允许（`imageops::resize` 支持，core 无日志通道不能 warn，拒绝会挡正当用法）；（c）目标
`w==0||h==0` 拒为 `InvalidImage`，`1×1` 合法；源宽高比 == 目标比 → 裁剪为 no-op。
- 替代方案：放大时拒绝/警告。否决理由：core 无 UI/日志；小图放大是正当场景。

**D5′ — 退化维度：拒绝（不静默产黑网格）。**（review 发现的核心漏洞）实测 `image 0.25`：
源 `100×1` 朝 `1:100` 目标裁剪 → 中间图 `0×1`，喂给 `imageops::resize(.., 80, 100, ..)` 返回 `Ok` +
**全 `[0,0,0]` 网格、不 panic、通过所有断言**——合法输入静默产出全黑结果，是最坏的 false-green。
决策：**任一退化维度必须确定性返回 `InvalidImage { reason }`（reason 点名退化维度），禁止
0 维中间图、禁止 panic。** 选「拒绝」而非「clamp 到 1px」：与既有 `w==0||h==0` 目标拒绝对称、
诚实；`100×1` 强配成 1px 宽网格同样无意义。
- **维度守卫总则——每个公开原语自守卫，不靠编排顺序兜底**（review 发现：原语 D3 是公开、
  可独立调用的，只在 `image_to_grid` 里排顺序挡不住直接调用原语的人）：
  - `crop_center` 自守卫：目标 `tw==0||th==0`（退化目标比）、源任一维度为 0、裁剪宽/高
    floor 到 0——算出裁剪矩形后、`to_image()` 之前校验。比值与裁剪尺寸用整数交叉相乘
    `src_w*th` vs `src_h*tw`（不靠 `tw/th` 除零），且**所有交叉积/尺寸积先 widening 到 `u64`**
    （两 `u32` 之积 `≈1.84e19 < u64::MAX`）：否则源 `100000×1`、目标 `1×100000`（本身小、比例
    极端）会让 `u32` 积 `1e10` 溢出 → debug panic / release wrap 选错路径，违反「不 panic、确定」
    契约（此溢出在分配之前，与已推迟的大网格 `usize` 溢出无关）。校验非 0 后 checked-cast 回 `u32`。
  - `resize_image` 自守卫：目标 `w==0||h==0` **或源图任一维度为 0**（任一即拒，非同时）（实测
    `imageops::resize` 对 0 维源静默 `Ok` + 全黑，必须在调用它之前挡掉）。
  - `image_to_grid` = 先校验目标 `w/h` → decode → `crop_center`（自守卫）→ `resize_image`
    （自守卫）。目标维度校验放最前，因 `crop_center` 以目标算比、目标为 0 会先除零。
- 三条入口（`crop_center` / `resize_image` / `image_to_grid`）直接调用时，零维度拒绝都必须可达。

**D5″ — 裁剪策略是未来接缝（非 M2 代码）。** `crop_center` 把「居中 + 右/下偏置」写成函数
本体而非选项；M3 / M9（ARCHITECTURE 的 Flutter `CropPage`）会要顶部裁剪 / 智能裁剪。M2 不实现，
但在此记录：裁剪**策略**（非仅偏移取整）是将来 `ResizeOptions` / 独立函数的接缝，M3 不得把
「只有中心裁剪」烤进 `image_to_grid` 的契约。一句话，零 M2 代码。

**D6 — 错误模型：两个变体。** `ImageDecode(#[from] ::image::ImageError)`（损坏/截断/未编入
的格式）+ `InvalidImage { reason: String }`（自身语义错，如 0 维，reason 确定性点名）。
`BeadError` 已 `#[non_exhaustive]`，新增非破坏。
- 与 palette 对称：`#[from]` 包装库错 + 语义错变体。测试只断言变体，不断言
  `image::ImageError` 的 Display 文案（同 serde_json 的处理）。
- **⚠️ 模块名 vs crate 名冲突（编译陷阱）：** 本地模块叫 `image`（ARCHITECTURE 规定），与外部
  crate `image` 同名。在 `lib.rs`（含 `pub mod image;`）里写 `image::ImageError` 会解析到
  **本地** `crate::image` 模块（无 `ImageError`）→ 编译失败 / 指错。引用外部 crate 必须用
  **前导 `::`**：`::image::ImageError`、`::image::imageops::FilterType`（或文件顶部
  `use ::image::{ImageError, imageops::FilterType, RgbImage};` 再用裸名）。`image/mod.rs` 内同理
  （`image::imageops` 可能经 crate 根解析到本地模块），统一走 `::image::` 或 `use ::image::…`。

**D6′ — `RgbImage` 层用的是 `imageops` 自由函数，不是 `DynamicImage` 方法（review 实测核对）。**
全程载体是 `RgbImage`（`decode_image`→`RgbImage`、`crop_center(&RgbImage)→RgbImage`、
`resize_image(&RgbImage,..)`）。关键：`resize_exact` **只在 `DynamicImage` 上**
（`image::images::dynimage`），`RgbImage`（= `ImageBuffer<Rgb<u8>,Vec<u8>>`）**没有** `resize`/
`resize_exact` 方法——按 `resize_exact` 直写**编译不过**。`RgbImage` 层的等价操作是 `imageops`
自由函数：
- 解码 + 归一：`::image::load_from_memory(bytes)?.to_rgb8()`；
- 中心裁剪：`::image::imageops::crop_imm(&img, x, y, w, h).to_image()`（`crop_imm` 返回
  `SubImage` 视图，`.to_image()` 物化为新 `RgbImage`）；
- 精确缩放：`::image::imageops::resize(&img, w, h, filter)`（始终精确、不保持宽高比，返回
  `RgbImage`）——它对 0 维源的静默全黑短路与 `DynamicImage::resize_exact` 相同，故 D5′ 守卫
  逻辑不变，只是落到这个函数名上。

**D7 — 依赖：`image 0.25`，特性裁剪。** `default-features=false, features=["png","jpeg","webp"]`。
- 替代方案：单编解码器 crate（如仅 `png`）。否决理由：无法覆盖 JPEG/WEBP，要自接三个
  解码器 + 自写重采样。`image` 是唯一单 API 覆盖三格式 + 自带 `imageops` 重采样的 crate。
  特性裁剪回应「太重」的反对，并砍掉 `rayon`。

**D8 — 确定性。** 无 `rayon`（特性裁剪后 `image` 单线程）、固定 `Lanczos3`、固定裁剪偏移、
归一 RGB8、解码即存储（不自动 EXIF 旋转）、提交 `Cargo.lock` 冻结编解码器补丁版本。
- **跨架构口径（不弱化项目门）：** 确定性是门（CLAUDE.md 硬规则 2 / ROADMAP「M2+ 同输入同
  输出」），M2 不narrow它。`Lanczos3` 是 `f32` 重采样；IEEE-754 下逐操作（Rust 默认无 FMA 收缩、
  现代目标用 SSE 无 x87 扩展精度）是确定的，故跨架构逐字节一致是**预期**——其**校验**按 ROADMAP
  既有分工落在 M8「CLI == FFI」（M2 立门 / M7 golden / M8 跨平台验证）。因此 M2 **不预先硬编码**
  一份「赌它跨架构相同」的 `Lanczos3` golden 值（dev=arm64 生成的 `f32` 值若与 x86_64 CI 偶有
  末位差，硬编码 golden 会假阴）。
- **golden 策略（避免 CI 跨架构假阴）：** ①确定性靠 `grid_is_deterministic` 的**同进程重算
  比对**证明（跨平台稳定，是真正的门）；②另需的小 golden 断言用**跨架构位精确**的方式——对
  那格用 `Nearest`（整数精确、无 `f32`）裁+缩，而非 `Lanczos3` 的 `f32` 值；`Lanczos3` 由
  `default_filter_is_lanczos3` 单独钉默认值。详见 tasks 6.2。
- golden 源用**脚本生成的小 PNG**（无损、稳定），不用真照片（避免源文件成为不可控依赖）；
  JPEG/WEBP（`zune-jpeg`/`image-webp`）仅做解码冒烟测，不作 golden 源。

**D9 — EXIF：推迟。** 解码即存储朝向，不自动旋转。措辞校正：`image 0.25` **能**暴露朝向元
数据（`ImageDecoder::orientation`、JPEG 的 EXIF 朝向解析），但 `load_from_memory` /
`DynamicImage::from_decoder` **不自动套用**它（已核对 0.25.9 源码：解码路径不调
`apply_orientation`）。M2 即依赖这一「读到但不套用」的默认行为，不引入自动旋转。
- 在 `image/mod.rs` 留 `ponytail:` 注释；加 `exif_orientation_not_applied` 测试钉住当前
  行为，将来若改自动旋转该测试会响亮失败。
- **EXIF 夹具如何确定性构造：** `image` 的 JPEG 编码器只有在调用 `set_exif_metadata` 并传入
  手写的 EXIF APP1 朝向字节块时才会写出朝向标签——普通「脚本生成 PNG」式路径不会产生 EXIF。
  故一次性生成脚本须用 `set_exif_metadata` 写一个带朝向 tag 的 APP1 块，产出的 JPEG **字节**
  入库（与「脚本不入库、夹具字节入库」一致，不违反「不用真照片」原则）。

**D10 — 文件布局：`image/mod.rs` + `models/mod.rs`（目录模块）。** 对齐 ARCHITECTURE 的
`image/`+`models/` 目录与 `palette/mod.rs` 先例；`models/` 用目录给 M3 的
`BeadCell`/`BeadPattern`/`ColorStat` 预留位。M2 不拆分 `image/`（YAGNI）。

## 风险 / 权衡

- [`image`/`FilterType` 泄漏进公开签名] → 用户已选「更灵活」版接受之；FFI 暴露滤镜时再映射。
- [WEBP/JPEG 解码器随补丁版本漂移] → golden 源用 PNG（无损）；锁 `Cargo.lock`；JPEG/WEBP
  仅解码冒烟测。
- [极端宽高比裁剪 floor 到 0 宽] → **已定**（D5′）：`crop_center` 校验退化维度并返回
  `InvalidImage`，不产 0 维中间图；加守卫测试（tasks 6.7）。实测已复现该全黑 false-green。
- [解压炸弹（巨图 OOM）] → M2 用 `image` 默认 `Limits`（近无限）；core 取自可信 CLON 字节，
  推迟到后续加固里程碑，此处仅记录。
- [`Lanczos3` 在锐边可能振铃/光晕] → 若 golden 暴露，改 `Triangle`/`CatmullRom` 仅一处常量
  （或经 `ResizeOptions` 切换），非破坏。

## Migration Plan

无运行时迁移：纯新增能力 `image-grid`，不改已生效的 `palette` 规范。回滚为撤销本变更
（删 `image`/`models` 模块、回退两个 `BeadError` 变体与 `image` 依赖）。

## Open Questions

无 —— 探索阶段决策已全部冻结（含公开 API 表面取「架构师版」）。
