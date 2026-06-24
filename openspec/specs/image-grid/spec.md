# image-grid 规范

## 目的
定义从原始图像字节到 `PixelGrid`（行优先 RGB8 像素网格、配色前的真理源）的预处理：解码、颜色归一到 RGB8、
按目标宽高比中心裁剪、缩放到固定网格；不依 EXIF 自动旋转。`PixelGrid` 是过渡中间产物，M3 配色后由 `BeadPattern`
取代为真理源。确定性：同字节 + 同尺寸产出逐字节相同网格。纯库行为：字节进、`PixelGrid` 出，无文件系统/UI/平台依赖。

## 需求
### 需求:从字节解码图像
`decode_image` 与 `image_to_grid` 必须接受图像字节（`&[u8]`）并自动嗅探格式；必须支持
PNG、JPEG、WEBP。引擎禁止读取文件系统、禁止接受文件路径——文件读取由调用方负责。无法
识别或损坏的字节必须返回 `BeadError::ImageDecode`。多帧 / 动图输入（APNG、动图 WEBP）按
编解码器默认行为解码为其默认 / 首帧（确定性来自锁定的编解码器版本）；M2 **不把帧选择钉成
golden 契约**，也不专门为此加夹具 / 测试（确定性动图夹具的生成成本与该边缘输入不成比例，
YAGNI；动图成真实用例再补）。JPEG / WEBP 仅保证
「可解码 + 尺寸与源一致」，不对逐像素配色正确性作承诺（嵌入的 ICC 配置不被套用，得到原始
解码值）；其子编码（baseline / progressive JPEG、CMYK JPEG、有损 / 无损 WEBP）M2 不逐一钉
golden——某子编码若不被支持会显式 `ImageDecode`（不会静默错色），可接受。PNG 才是 golden
源（见 design D8）。

#### 场景:解码 PNG / JPEG / WEBP
- **当** 传入合法的 PNG、JPEG 或 WEBP 字节
- **那么** `decode_image` 返回 `Ok`，图像尺寸与源一致

#### 场景:拒绝损坏或未支持的字节
- **当** 传入非图像字节（如 `b"not an image"`），或未编入的格式（如 GIF / BMP / TIFF）
- **那么** 返回 `Err(ImageDecode)`

### 需求:颜色归一到 RGB8
解码后必须把任意颜色类型归一为 `[u8; 3]`：alpha 通道丢弃（不展平到背景）；Luma8 灰度
映射为 `r == g == b`；16 位降为 8 位；调色板 PNG 展开为 RGB。归一必须确定性。

#### 场景:丢弃 alpha
- **当** 解码一张含透明像素的 RGBA PNG
- **那么** 结果像素为其 RGB 通道（alpha 被丢弃），且多次运行一致

#### 场景:灰度归一
- **当** 解码一张 Luma8 灰度 PNG
- **那么** 每个像素的 `[u8; 3]` 满足 `r == g == b`

### 需求:中心裁剪到目标宽高比
`crop_center` 必须把源图中心裁剪到与目标 `width : height` 宽高比一致的最大子矩形。裁剪
偏移必须用整数 floor，多余的奇数像素分给右 / 下。源宽高比**恰好**等于目标比时必须返回不变
的裁剪（仅精确相等才 no-op；近似相等仍按 floor 正常裁剪）。`crop_center` 作为公开原语
（D3）可被直接调用，必须**自守卫**以下退化输入，一律返回 `BeadError::InvalidImage { reason }`
（`reason` 确定性且点名退化的维度），且禁止产生 0 维中间图、禁止 panic：
- 目标 `tw == 0` 或 `th == 0`（退化的目标比，会使裁剪宽 / 高 floor 到 0）；
- 源任一维度为 0；
- 极端宽高比使计算出的裁剪宽 / 高 floor 到 0。

宽高比比较与裁剪尺寸推导必须用整数交叉相乘 `src_w * th` vs `src_h * tw`（避免浮点不确定性，
也不依赖 `tw/th` 除零）。**所有交叉积与裁剪尺寸积必须先 widening 到 `u64` 再算**（两个 `u32`
之积最大 `≈1.84e19 < u64::MAX`，`u64` 足够）：否则像源 `100000×1`、目标 `1×100000` 这种**本身
很小但比例极端**的输入会让 `u32` 积 `100000*100000=1e10` 溢出——debug panic、release wrap 选错
裁剪路径，违反本需求「不 panic、确定性」契约。校验裁剪维度非 0 后再 checked-cast 回 `u32`。
（此溢出发生在分配之前，与已推迟的 `usize` 大网格分配溢出是两回事。）

#### 场景:横图裁成方形
- **当** 源是宽图、目标为方形
- **那么** 裁剪结果居中，且宽 == 高

#### 场景:奇数像素差偏向右/下
- **当** 中心裁剪存在奇数像素差
- **那么** 偏移为 `floor(差 / 2)`，多余像素落在右 / 下

#### 场景:极端宽高比裁剪退化为零维被拒
- **当** 源为极端比（如 `100×1`），目标比使裁剪宽或高 floor 到 0（如目标 `1:100`）
- **那么** 返回 `Err(InvalidImage)`，`reason` 点名退化的维度，且不产生全黑网格、不 panic

#### 场景:本身小但比例极端不溢出 u32
- **当** 源 `100000×1`（或镜像 `1×100000`），目标 `1:100000`（或 `100000:1`）——交叉积
  `100000*100000 = 1e10` 超 `u32::MAX`
- **那么** 用 `u64` 算交叉积后返回 `Err(InvalidImage)`，**debug 不 panic、release 不 wrap 选错路径**

### 需求:缩放到固定网格
`resize_image` 与 `image_to_grid` 必须把图像缩放到精确的 `width × height`（不保持宽高比——
调用方先裁剪）。在 `RgbImage` 层用 `::image::imageops::resize`（始终精确，无宽高比保持）——
注意 `DynamicImage::resize_exact` 不可用于 `RgbImage`（仅 `DynamicImage` 有），见 design D6′。
默认重采样滤镜为 `Lanczos3`，经 `ResizeOptions.filter` 可配。必须允许放大（目标大于源）。
`resize_image` 作为公开原语（D3）可被直接调用：源图任一维度为 0（如外部手构 `0×N` 的
`RgbImage`）时必须返回 `InvalidImage`（reason 点名退化维度），**不得**让 0 维源喂进
`imageops::resize`——实测它对 0 维源静默返回 `Ok` + 全黑网格、不 panic，是与裁剪退化同源的
false-green。

#### 场景:产出精确尺寸网格
- **当** `image_to_grid(bytes, 80, 100, &ResizeOptions::default())`
- **那么** 返回 `Ok(PixelGrid)`，`width == 80`、`height == 100`、`pixels.len() == 8000`

#### 场景:允许放大
- **当** 源很小（如 4×4），目标较大（如 50×50）
- **那么** 返回 `Ok`，`pixels.len() == 2500`

### 需求:PixelGrid 输出形状
`PixelGrid` 必须含 `width`、`height` 与行优先的 `pixels: Vec<[u8; 3]>`，且
`pixels.len() == width * height`，`pixels[y * width + x]` 为第 (x, y) 格。长度与下标运算
必须以 `usize` 进行（`width as usize * height as usize`），不得用 `u32` 乘加（防大网格
溢出）。该不变量由 `resize_image` 保证；公开字段构造的 `PixelGrid` 由构造方负责满足
`pixels.len() == width * height`，否则下游按下标取格逻辑错误（见 design D1）。

#### 场景:1×1 目标合法
- **当** `image_to_grid(.., 1, 1, ..)`
- **那么** 返回 `Ok`，`pixels.len() == 1`

### 需求:无效操作错误
**维度守卫总则：** `image` 模块的每个公开函数都必须**自守卫**自己的退化维度输入，返回
`BeadError::InvalidImage { reason }`，而不是依赖某条编排顺序——因为 `crop_center` /
`resize_image` 都是 D3 明确公开、可被独立调用的原语，任一直接收到 0 维就会静默产黑或 panic。
`reason` 必须确定性且点名出错的维度（同输入重复调用得到逐字符相同的 `reason`）。逐函数：
- `resize_image`：目标 `w==0||h==0` **或**源图任一维度为 0（任一即拒，非同时）→ `InvalidImage`；
- `crop_center`：目标 `tw==0||th==0`、源任一维度为 0、或裁剪宽/高 floor 到 0 → `InvalidImage`；
- `image_to_grid`：在 `decode → crop_center` **之前**先校验目标 `w/h`（`crop_center` 以目标算
  宽高比，目标为 0 会先除零绕过下游守卫），再依赖 `crop_center`/`resize_image` 的自守卫兜底。

故零维度的拒绝对 `resize_image`、`crop_center`、`image_to_grid` 三条入口都必须可达。

#### 场景:拒绝零宽
- **当** `resize_image(src, 0, 10, ..)`
- **那么** 返回 `Err(InvalidImage)`，`reason` 点名 width

#### 场景:拒绝零高
- **当** `resize_image(src, 10, 0, ..)`
- **那么** 返回 `Err(InvalidImage)`，`reason` 点名 height

#### 场景:image_to_grid 也拒绝零目标维度
- **当** `image_to_grid(bytes, 0, 10, ..)` 或 `image_to_grid(bytes, 10, 0, ..)`
- **那么** 返回 `Err(InvalidImage)`（在 `crop_center` 除零之前命中），不 panic

### 需求:确定性
同一字节输入与同一 `ResizeOptions` 必须产生逐字节相同的 `PixelGrid`（确定性是门，对齐
CLAUDE.md 硬规则 2 与 ROADMAP 的 M2+ 确定性门）。实现禁止引入非确定性来源（`rayon` 并行
重采样、随机、迭代顺序泄漏）；重采样滤镜与裁剪偏移规则必须固定，锁 `Cargo.lock` 冻结编解码器。
- **跨架构口径（不弱化项目门，按 ROADMAP 既有分工）：** `Lanczos3` 是 `f32` 重采样；
  IEEE-754 下逐操作（无 FMA 收缩 / 无 x87 扩展精度）是确定的，故跨架构逐字节一致是**预期**，
  但其**校验**落在 M8 的「CLI == FFI」检查（ROADMAP 既定：M2 立门、M7 golden、M8 跨平台验证）。
  M2 不预先硬编码一份「赌它跨架构相同」的 `Lanczos3` golden 值。
- **M2 测试如何证明（避免 CI 跨架构假阴）：** ①确定性由 `grid_is_deterministic` 的同进程
  重算比对证明（跨平台稳定，是真正的门）；②任何硬编码 golden 期望网格用**跨架构位精确**的
  方式断言——即对那一格用 `Nearest`（整数精确、无 `f32`）裁+缩，而非 `Lanczos3` 的 `f32` 值
  （见 tasks 6.2）。

#### 场景:重复产网格一致
- **当** 对同一字节、同一 `ResizeOptions` 多次调用 `image_to_grid`
- **那么** 每次返回的 `PixelGrid` 完全相等（含像素顺序）

### 需求:EXIF 朝向不自动旋转
解码必须按存储朝向进行，禁止自动套用 EXIF 朝向（M2 范围内）。该行为必须有测试钉住，
将来若改为自动旋转会使该测试失败。

#### 场景:带朝向标签的 JPEG 按存储解码
- **当** 解码一张带 EXIF 朝向标签的 JPEG
- **那么** 图像按存储（未旋转）解码

