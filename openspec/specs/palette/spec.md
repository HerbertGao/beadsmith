# palette 规范

## 目的
待定 - 由归档变更 add-palette-loader 创建。归档后请更新目的。
## 需求
### 需求:从 JSON 字节加载调色板
`load_palette` 必须接受 JSON 字节（`&[u8]`）并返回内存中的 `Palette`。引擎禁止读取
文件系统、禁止接受文件路径——文件读取由调用方（CLI）负责。加载成功时，`colors`
的顺序必须与 JSON 中出现的顺序一致。

#### 场景:加载合法调色板
- **当** 传入一段含合法 `brand` 与至少一个合法颜色的 JSON 字节
- **那么** 返回 `Ok(Palette)`，`colors` 数量与顺序与输入一致，且每个 `rgb` 解析为
  `[u8;3]`（例如 `#0A0B0C` → `[10, 11, 12]`）

### 需求:hex 颜色解析
JSON 中每个颜色的 `rgb` 字段是 `"#RRGGBB"` 字符串，加载时必须解析为 `[u8;3]`。
解析必须严格：必须以 `#` 开头、正好 6 位 ASCII 十六进制、大小写不敏感；禁止接受
`#RGB` 简写、禁止缺少 `#`、禁止其它长度。

#### 场景:接受小写 hex
- **当** 某颜色 `rgb` 为 `"#aabbcc"`
- **那么** 解析为 `[170, 187, 204]`

#### 场景:拒绝非十六进制字符
- **当** 某颜色 `rgb` 含非十六进制字符（例如 `"#00GG00"`）
- **那么** `load_palette` 返回 `Err(InvalidPalette)`，且 `reason` 点名出错的 `code`

#### 场景:拒绝简写与缺少井号
- **当** 某颜色 `rgb` 为 `"#FFF"`（简写）或 `"000000"`（缺 `#`）
- **那么** `load_palette` 返回 `Err(InvalidPalette)`

### 需求:调色板结构校验
加载时必须校验：`colors` 禁止为空；所有 `code` 必须唯一。校验必须 fail-fast——遇到
第一个违规即返回错误。

#### 场景:拒绝空颜色表
- **当** JSON 的 `colors` 为 `[]`
- **那么** 返回 `Err(InvalidPalette)`，`reason` 说明颜色表为空

#### 场景:拒绝重复 code
- **当** 两个颜色具有相同的 `code`（例如两个 `"S01"`）
- **那么** 返回 `Err(InvalidPalette)`，`reason` 点名重复的 `code`

### 需求:错误模型
JSON 语法错误或结构不符（缺字段、类型错）必须返回 `BeadError::PaletteParse`；通过
解析但违反语义不变量（空表、重复 code、坏 hex）必须返回
`BeadError::InvalidPalette { reason }`。`reason` 必须确定性，且点名单个出错对象。

#### 场景:语法错误的 JSON
- **当** 传入不是合法 JSON 的字节（例如 `b"{ not json"`）
- **那么** 返回 `Err(PaletteParse)`

#### 场景:缺少必需字段
- **当** 某颜色缺少 `rgb` 字段
- **那么** 返回 `Err(PaletteParse)`

### 需求:校验已构造的调色板
`validate_palette(&Palette)` 必须复查结构不变量（`colors` 非空、`code` 唯一），并在违反
时返回 `Err(InvalidPalette)`。由于 hex 在加载阶段已解析为 `[u8;3]`，已构造的 `Palette`
按类型即 hex 合法，因此 `validate_palette` 不复查、也无法复查 hex。

#### 场景:对已加载的调色板校验通过
- **当** 对一个由 `load_palette` 成功返回的 `Palette` 调用 `validate_palette`
- **那么** 返回 `Ok(())`

#### 场景:校验拒绝空颜色表
- **当** 对一个手工构造、`colors` 为空的 `Palette` 调用 `validate_palette`
- **那么** 返回 `Err(InvalidPalette)`

#### 场景:校验拒绝重复 code
- **当** 对一个手工构造、含两个相同 `code` 的 `Palette` 调用 `validate_palette`
- **那么** 返回 `Err(InvalidPalette)`，`reason` 点名重复的 `code`

### 需求:确定性加载
调色板加载必须确定性：同一字节输入必须产生逐字节相同的 `Palette` 或相同的错误。实现
禁止在校验或错误信息中使用会泄漏迭代顺序的结构（例如 `HashMap`）。多个错误并存时，
报告的"第一个错"必须由固定校验顺序决定：① `colors` 非空 → ② 按 `colors` 顺序逐个解析
hex → ③ 唯一 `code` 检查。

#### 场景:重复加载结果一致
- **当** 对同一 JSON 字节多次调用 `load_palette`
- **那么** 每次返回的 `Palette` 完全相等（包含颜色顺序）

#### 场景:错误信息确定性
- **当** 对同一段会触发语义错（坏 hex 或重复 code）的 JSON 字节多次调用 `load_palette`
- **那么** 每次返回相同的 `Err(InvalidPalette)`，且 `reason` 字符串逐字节相同（点名同一 `code`）

### 需求:调色板 JSON 文件格式与附带调色板
调色板文件格式必须为：顶层对象含 `brand`（字符串）与 `colors`（数组），每个颜色含
`code`、`name`、`rgb`（`"#RRGGBB"` 字符串）。项目必须附带一个真实调色板
`palettes/artkal_s.json`（Artkal 5mm Midi S 系列）。该数据的 hex 为社区近似值（非
官方）必须如实注明，且必须保留来源数据集的 MIT 署名（仓根 `NOTICE`）。

#### 场景:加载附带的 Artkal S 调色板
- **当** 加载 `palettes/artkal_s.json` 的字节
- **那么** 返回 `Ok(Palette)`，`colors` 非空且所有 `code` 唯一

