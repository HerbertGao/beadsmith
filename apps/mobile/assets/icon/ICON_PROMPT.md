# 拼豆匠 App 图标 — 文生图 Prompt

> 用途：把下面的 prompt 喂给文生图 AI（Midjourney / DALL·E 3 / Flux / SD），生成多个**正式图标候选**来挑方向。
> 视觉概念来自 UI Designer 评审（母题=拼豆拼成的心，鲜艳多彩扁平，受控同心圈配色）。
>
> **⚠ 诚实前提（先读）**：文生图模型**不会精确遵守**「恰 9 列 / 精确 hex / 透明图层 / 无 alpha / 双平台分层」这类硬约束——
> 它们擅长出**视觉方向/氛围**，不擅长像素级几何与规范。所以：
> - **本 prompt 用来「选方向、找感觉」**（配色、密度、豆孔质感、构图）。
> - **真正可上架的精确图标**（自适应 foreground 透明层、iOS 满幅无 alpha、9 列对齐、品牌 hex）建议由
>   `apps/mobile/tool/gen_placeholder_icon.py`（程序化、确定性）改造产出，或用矢量工具（Figma/Illustrator）按下方
>   「精确规格」重绘——那才能保证 flutter_launcher_icons 需要的干净图层与色值。
> - 挑中某个 AI 候选后，把它当**参考图**，再用脚本/矢量落地精确版。

---

## 核心 Prompt（英文主 + 中文注）

```
App icon, flat vector illustration, a heart shape built out of fuse beads
(perler beads) arranged on a square peg grid — about 9 beads wide and 8 tall,
roughly 48 round beads total, each bead a solid-color ring (donut) with a clear
hole in the center, beads separated by thin even gaps that reveal the warm cream
background between them. Colors follow concentric contour bands from the heart's
outline inward: outer ring coral red (#FF4D5E), next tangerine orange (#FF8A1E),
then sunflower yellow (#FFC42E) only in the interior, a teal (#21C7D6) core, and
a single violet (#9B5DE5) bead at the very center as a focal point. Warm cream
background (#FFF3E0) fills the whole square. Completely flat: no gradient, no
gloss, no drop shadow, no 3D. The heart's outer edge is stair-stepped / pixelated
by the bead grid (do NOT smooth it into a clean vector heart). Cheerful, playful,
crafty, clean. Centered composition, generous margin, not touching the edges.
Modern mobile app icon, high contrast, reads clearly at small sizes.
--ar 1:1
```

中文速记：奶白 `#FFF3E0` 满底；~9 列、约 48 颗**带透明中心孔的圆环拼豆**拼成对称爱心；珠间露奶白缝
（像拼豆板）；同心圈上色 外→内 珊瑚红→暖橙→明黄→湖青核心 + 正中 1 颗紫罗兰；**纯平涂、无渐变无阴影**；
心的外轮廓保留网格阶梯感（别抹成平滑矢量心）；居中留边、不贴边。

## Negative Prompt（SD/Flux 用；Midjourney 用 `--no`）

```
gradient, gloss, glossy, 3D, realistic plastic, bevel, drop shadow, inner shadow,
photorealistic, text, letters, words, watermark, wordmark, smooth vector heart,
outline stroke, dark background, low contrast, beads touching edges, cropped,
busy background, pattern background, random muddy colors, over 12 columns dense grid
```

Midjourney 追加：`--no text, gradient, gloss, 3d, shadow, smooth heart, dense grid`

---

## 两个平台变体（生成时分别出，或先出主图再据此重绘图层）

### ① Android 自适应 · foreground（前景层）
在核心 prompt 基础上，替换背景描述为：
```
… the beaded heart ONLY, centered and occupying about 66% of the frame with
transparent margin all around, the bead center-holes and the gaps between beads
also fully transparent (transparent PNG), NO background fill.
```
中文：只画心、居中限中心 ~66%、四周 + 豆孔 + 缝隙全透明、**无底色**（透明 PNG）。背景层单独用纯 `#FFF3E0`。
（注：多数文生图模型出不了干净透明通道——此变体基本得靠脚本/矢量实现，AI 图仅作配色参考。）

### ② iOS · 满幅方图（无 alpha）
```
… fills the entire square with the cream (#FFF3E0) background, NO transparency /
NO alpha channel anywhere, the beaded heart centered at about 74% of the frame,
bead holes filled with the same cream color, heart kept well inside a safe margin
(≥12% from edges) so Apple's rounded-corner mask never clips it.
```
中文：满幅奶白铺满、**整图无 alpha**；心居中占 ~74%；豆孔填奶白；留边 ≥12% 避开系统圆角。

---

## 平台参数建议

- **Midjourney**：`--ar 1:1 --style raw`（raw 更贴 flat 设计、少 MJ 默认摄影味）；`--v 6+`。
- **DALL·E 3**：把核心 prompt 直接口述，强调 "flat vector app icon, no gradient, no 3D"。
- **Flux / SD**：核心 prompt + 上方 Negative Prompt；SD 可加 `flat design, vector, icon` LoRA/风格词。
- 生成 1024×1024，挑中后放大到 1024² 作源图。

---

## 精确规格（落地正式版用，非文生图能保证）

| 项 | 值 |
|---|---|
| 底色 | `#FFF3E0`（备用略深 `#FFEBCC`） |
| 网格 | 心宽 ~9 珠 × 高 ~8 珠，总 46–52 颗；**上限 <12 列** |
| 珠外径 / 豆孔 | 珠 = 0.90×格；孔 = 0.40×格；珠间露 ~10% 奶白缝 |
| 同心圈配色（外→内） | 珊瑚红 `#FF4D5E` → 暖橙 `#FF8A1E` → 明黄 `#FFC42E`（仅内部）→ 湖青 `#21C7D6` 核 + 正中 1 颗 紫罗兰 `#9B5DE5`；玫粉 `#FF5DA2` 机动过渡 |
| 对比硬规则 | 黄 `#FFC42E` 不上最外圈/心尖（贴奶白会消失）；最外圈只用高对比色 |
| 风格 | 纯平涂、无渐变/高光/阴影；保留豆孔与奶白缝隙网格；心轮廓保留阶梯、不平滑 |
| Android 前景 | 心占 66%、四周+孔+缝透明；背景层纯 `#FFF3E0` |
| iOS | 满幅无 alpha、心占 74%、孔填奶白、留边 ≥12% |

**负面清单**：❌渐变/高光/3D/写实 · ❌文字/wordmark · ❌贴边/溢出安全区 · ❌随机逐珠上色 · ❌≥12 列过密 ·
❌低对比珠贴底 · ❌细描边/细网格线 · ❌iOS 带 alpha · ❌平滑矢量心 · ❌背景加图案。

> 现占位图（`assets/icon/app_icon*.png`，16 列随机色）为可用临时图；正式上架前用本文件 + 上表落地清晰版替换。
