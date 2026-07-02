# cli 规范（增量）

## 新增需求

### 需求:generate --generator 选生成模式
`bead-cli generate` MUST 新增可选 flag `--generator staged|gerstner`（`clap::ValueEnum`，CLI 侧手写 `match` 映射到 core 的 `GeneratorKind`，core **不**依赖 clap），**未给时默认 `staged`**（与引擎默认 `GeneratorKind::Staged` 一致）。`--generator gerstner` 启用 Gerstner 生成模式（照片路径）。非法值由 clap 在参数解析阶段拒绝、以**参数错误退出码 2** 结束、不 panic。此 flag 与既有 `--matcher`/`--max-colors` 并列为可选；`cell_size`/`filter`/`shape` **仍**不暴露。

#### 场景:--generator 各值成功、非法值退出码 2
- **当** 以 `--generator staged`、`--generator gerstner` 分别运行有效 `generate`
- **那么** 两者均退出码 0、写出四个文件，分别用 `Staged` / `Gerstner` 生成模式
- **且** 以 `--generator <非二值>`（如 `--generator slic`）运行时，进程以**退出码 2**（参数错误）结束、stderr 含可选值提示、**不 panic**、不写出文件

#### 场景:不给 --generator 默认 staged
- **当** 不给 `--generator` 运行 `generate`
- **那么** 用默认 `staged`（现分段路径），行为与未引入该 flag 前一致
