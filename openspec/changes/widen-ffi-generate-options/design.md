## 上下文

`bead-ffi` 是 `bead-core` 到 Dart 的零逻辑薄桥,M8 刻意把 `generate` 边界收窄为「仅 `width`/`height`」,其余选项取引擎 `Default`。Post-M9 引擎已落地 `max_colors`(`GreedyReducer`)、`despeckle`、`generator`(`GeneratorKind::{Staged,Gerstner}`),`GenerateOptions` 三字段与 CLI 旗标(`--max-colors`/`--despeckle`/`--generator`)均就绪,唯独 FFI 边界没开,导致这些能力对 App 不可达。

本设计只解决「把这三项透传过桥」这一件事:边界签名、`generator` 的跨边界表示、以及如何在放宽后守住「未设置即与旧默认逐字节一致」的既有闸门。不涉及任何引擎算法,也不涉及设置屏 UI(属 ROADMAP 另一工作流)。

约束:ARCHITECTURE 五条硬规则;桥必须保持零逻辑(只调 `load_palette` / `generate_pattern` / `pattern_json`);`bead-core` 零改动、不被 FFI 污染;确定性是闸门。

## 目标 / 非目标

**目标:**
- `generate` 在 `width`/`height` 外新增 `max_colors` / `despeckle` / `generator` 三个可选项并原样透传进 `GenerateOptions`。
- 三项未设置时,输出与放宽前的 FFI、及不带旗标的 CLI 默认路径**逐字节相同**。
- 设置后与 CLI 同旗标输出对齐(host 同 libm),并有 Dart 测试证明三项确被转发。

**非目标:**
- 不改引擎算法;不暴露 `filter`/`cell_size`/`shape`/`matcher`;不做设置屏 UI;不碰移动打包/签名。

## 决策

**决策 1:放宽单一 `generate`,而非新增第二个入口。**
在既有 `generate` 上加三个可选参数,`generate_inner` 相应加参并填入 `GenerateOptions`。
- 替代方案:新增 `generate_advanced`。否决理由:两个入口重复 marshalling 与错误扁平化逻辑、违背「单一生成入口」精神;FRB 支持可选/默认参数,一个函数即可干净表达,YAGNI。

**决策 2:`generator` 以 FFI 镜像枚举跨边界,而非 bool / String。**
在 `bead-ffi` 侧定义 `GeneratorKind` 的镜像(`staged`|`gerstner`),映射到 `bead_core::GeneratorKind`——与 CLI 的 `From<CliGenerator> for GeneratorKind` 同性质的平凡 marshalling。
- 替代方案 A:`bool use_gerstner`。否决理由:第三种生成器落地即成破坏性改动,且语义不如具名枚举自解释。
- 替代方案 B:`String`。否决理由:需运行时校验非法字符串、多一条错误路径,失去类型安全;枚举在编译期封闭取值。
- 说明:镜像→core 映射是纯值转换,不构成「桥层业务逻辑」,不违反零逻辑薄桥(镜像在 `bead-ffi` 侧完成,`bead-core` 零改动)。

**决策 3:`max_colors` / `despeckle` 用 `Option<u32>` 忠实镜像 core,不用 sentinel。**
- 替代方案:以 `0` 表示「关闭」。否决理由:`despeckle` 的 `Some(0)` 是**合法空操作**,与 `None`(跳过该阶段)在引擎语义上不同;用 `0` 当哨兵会抹掉这一区分。`Option` 是对 core 字段的诚实镜像,`None` 直接对应「不设旗标」。
- **不对称提醒**:`max_colors=Some(0)` 与 `despeckle=Some(0)` 语义相反——前者被 `GreedyReducer::new` 拒为 `InvalidImage`(经既有边界扁平化抛 Dart 异常,与 CLI `--max-colors 0` 一致),后者是合法空操作。UI 侧若用 `0..N` 滑杆需知此差异(属后续设置屏工作流);本变更只需保证越界值走既有错误扁平化、桥层不新增校验。

**决策 4:边界只开放「CLI 可表达 ∩ UI 需要」的三项。**
`filter`/`cell_size`/`shape` 继续不暴露——CLI 无法表达其非默认值,一旦暴露则「CLI == FFI」对非默认输入不可测;`matcher` 本轮移动端仍只承诺默认 Oklab。
- 替代方案:一次性开放所有 `GenerateOptions` 字段。否决理由:破坏可测性闸门、无对应 UI 需求,是投机式扩张边界。

**决策 5:确定性守法靠「结构体逐字段等价」而非新分支。**
构造 `GenerateOptions { width, height, max_colors, despeckle, generator, ..Default::default() }`。因 core `Default` 恰为 `max_colors=None`/`despeckle=None`/`generator=Staged`,当调用方「未设置」传入这三个默认值时,结果结构体与旧的 `{ width, height, ..Default::default() }` 逐字段相同——无需在桥内写 if/else。
- 验证:保留现有「未设置」的 byte-exact 测试不变,另加「已设置」用例。

## 风险 / 权衡

- **FRB 镜像枚举 marshalling 出错** → 缓解:沿用已用于 `BeadPattern`/`ColorStat` 的 FRB 镜像模式;`bead-ffi` in-crate `#[test]` 断言镜像→core 映射与「已设置选项确实改变输出」。
- **f32 路径仅同机 canonical** → 澄清 + 缓解:决定性档位上,只有 `despeckle` 是纯整数;`max_colors` 在默认 Oklab 下走 matcher 的 f32 感知度量(`ColorSnapshot::Perceptual`),与 `generator=gerstner` 同属 f32。但「已设置」byte-exact 闸门是**同机** FFI-vs-CLI 比较,host 同 libm 下 f32 与整数都逐字节稳,故 `max_colors`+`despeckle`(staged)承担严格字节对账、`generator=gerstner` 做同机 CLI 对齐 + iOS 结构不变量,均不承诺跨目标 byte-exact——与 spec 既有决定性边界一致。
- **Dart 签名破坏(现有唯一调用点)** → 缓解:本变更内同步更新 App 唯一调用点为 `None`/`None`/`staged`(行为不变);参数取可选,未来再加选项非破坏。
- **误被当成「顺手把 UI 也做了」** → 缓解:非目标显式排除设置屏;调用点暂传默认值,控件属后续工作流。

## Migration Plan

1. `crates/bead-ffi`:改签名 + 加镜像枚举 + 填 `GenerateOptions`;补 in-crate 测试。
2. 重新生成 FRB 绑定(`apps/mobile` 的 Dart 侧)。
3. 更新 App 唯一调用点传三默认值(行为零变化)。
4. 扩展「CLI == FFI」Dart 测试:保留两默认尺寸用例 + 新增一个「已设置」用例。
5. 回滚:还原 `bead-ffi` crate 与重生成的绑定即可;`bead-core` 未动,无迁移数据。

## Open Questions

- `despeckle` 的 `Some(0)` 与 `None` 差异是否需要在 UI 上体现,留给「设置屏」工作流决定,不属本变更。
