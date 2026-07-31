# Windows 玻璃材质：判别实验与稳定管线

> **实验状态（`codex/windows-true-alpha-glass`）**
>
> 本文记录的是测试分支已经复现并验证的 Windows 方案，还没有替代
> `docs/PRODUCT-CONSTRAINTS.md` 中的正式产品约束。合并采用前需要由维护者确认，
> 然后同步更新正式约束；如果实验被放弃，则以 `PRODUCT-CONSTRAINTS.md` 为准。

这份记录覆盖 Metrik 在 Windows 11 上对卡片、横向胶囊和纵向胶囊的玻璃实验。
2026-07-31 的冷启动对照实验推翻了昨天“WebView2 天生不透明，只能贴壁纸图”
的结论。旧壁纸取景实现仍暂存在后端，方便实验分支回退，但当前前端不再调用。

当前代码的完整分层、状态机、CSS 配方、流光实现和验收矩阵见
[`WINDOWS-GLASS-IMPLEMENTATION.md`](./WINDOWS-GLASS-IMPLEMENTATION.md)。

实测环境：Windows 11 26200、2560×1440、系统缩放 125%、Tauri 2 / WebView2。

## 1. 真正的故障点

Tauri 以 `transparent: true` 创建的窗口，冷启动时具有真实逐像素 Alpha。导致白色
底板的不是 CSS 透明度不够，而是运行期改变合成管线：

| 对照 | 结果 |
|---|---|
| 冷启动，仅使用创建期 `transparent: true` | **真实桌面、图标和后方窗口可见** |
| 运行期调用 `getCurrentWebview().setBackgroundColor([0, 0, 0, 0])` 后再使用 `backdrop-filter` | **出现不透明白色采样面** |
| 先启用 HostBackdrop / Acrylic，再切回 clear | **当前进程持续出现白色重定向底板** |
| 重新设置 DWM BlurBehind、`DWMSBT_NONE` / `AUTO`、隐藏再显示窗口 | 无法稳定恢复 |
| 完全不调用运行期背景重置，也不切 DWM 材质，只改 CSS tint | clear → off → dark → light → clear 全部稳定 |

因此，之前“把 WebView2 底色重新设成透明”的操作并不是无害的初始化；在当前
WebView2/Tauri 组合中，它会改变外部背景的采样结果。DWM 材质切换也不是可逆的
视觉开关，至少不能作为卡片/胶囊和三种 tint 的日常状态机。

## 2. 当前稳定架构

Windows 小组件在整个窗口生命周期中只使用一条合成管线：

1. `tauri.conf.json` 在创建窗口时声明 `transparent: true`。
2. 运行期不调用 WebView 背景重置，不启用或关闭 HostBackdrop、Acrylic、Mica。
3. `#root` 是唯一的大面积玻璃层，负责一个冷白/深色 tint 以及
   `backdrop-filter: blur(...) saturate(...)`。
4. 卡片和胶囊沿用 DWM 默认小圆角，不使用自定义 HWND region；Windows 桌面端
   `#root` 不再画第二套圆角或静态边框。浏览器没有原生窗口，只在
   `html[data-runtime="browser"]` 下用 `#root` 模拟外框；shell 始终透明且不画
   第二层边框。
5. 大面积子面板不再重复铺半透明白底，只保留分隔线、选中态和 hover。
6. clear / light / dark / off 仅改变 CSS，卡片与胶囊变形也不触碰 HWND 合成策略。

这条规则比“每种皮肤对应一种 Windows 原生材质”更重要：同一窗口一旦在运行期
换过合成后端，即使 API 返回成功，后续 clear 也可能只看到白板。

## 3. 边缘流光

Pogget 边缘的轻微流动感不需要 React 每帧重渲染。当前实现直接在指针事件中更新：

- `--glass-pointer-x`
- `--glass-pointer-y`
- `--glass-edge-opacity`

一个挂在最外层 `#root`、只占 1px 边环的径向渐变伪元素读取这些变量。它不参与布局、不接收命中，
不会影响 Tauri 拖动区和胶囊尺寸测量；离开窗口后淡出。系统要求减少动画时关闭
过渡，强制色彩和减少透明度模式下关闭该效果。

## 4. 两个参考项目能证明什么

### PoggetCore

`EnderMo/PoggetCore` 是核心/引擎仓库，不包含截图中完整的小组件 UI。其公开信息和
已有符号证据能说明 Pogget/VinaUI 有 D2D、Gaussian Blur、壁纸缓存和材质渲染能力，
但仅凭该仓库不能断言截图中的每个像素究竟来自桌面抓取、壁纸缓存还是窗口合成。
昨天把“存在壁纸缓存”直接等同为“Pogget 只贴壁纸”属于过度推断。

### Coffee-CLI

Coffee-CLI 的结构更接近本次成功实验：窗口从创建时透明，根节点承担 tint 与
`backdrop-filter`，大面积内层保持透明，而且不会在皮肤切换时反复重置 WebView
背景。这也是 Metrik 当前采用的架构方向。

## 5. 为什么停用壁纸取景方案

旧方案是“读壁纸文件 → 裁剪 → 模糊 → 按窗口位置负偏移”。它可以模拟透明，但有
四个结构性缺陷：

- 只能显示壁纸，无法透出位于 Metrik 后方的窗口和桌面图标。
- Windows 的真实壁纸布局不等于简单居中裁剪，实测有约 27 物理像素的偏差。
- 窗口移动、DPI、多显示器、幻灯片换图都需要额外同步和缓存失效逻辑。
- 多层 tint 很容易叠成乳白板，恰好掩盖真实 Alpha 是否成功。

它仍可作为极老系统或透明合成失效时的独立降级方案研究，但不应再作为 Windows 11
主路径。

## 6. 验收边界

Windows 原生验收至少包含：

- 冷启动 clear 卡片能看到真实桌面/图标。
- 卡片 ↔ 横向胶囊 ↔ 纵向胶囊不出现白底板。
- clear → off → dark → light → clear 往返后仍保持真实 Alpha。
- 指针靠近边缘时高光位置跟随，离开后淡出。
- “减少透明度”、减少动画和强制色彩模式仍可读。

这是 Windows shell 专属结论。macOS 继续使用系统 vibrancy，不复制 Windows 的
窗口合成和圆角策略；浏览器预览只能验证布局与 CSS 回落层，不能证明 HWND Alpha。
