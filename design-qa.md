# Windows True-Alpha Glass — Design QA

Scope: Windows shell experiment on `codex/windows-true-alpha-glass`.

## Latest comparison target

- Source visual truth:
  `C:/Users/keros68/AppData/Local/Temp/codex-clipboard-301759e2-e906-48e9-9a05-05afa56ba713.png`
- Rejected implementation:
  `C:/Users/keros68/AppData/Local/Temp/codex-clipboard-009d1c14-9cf4-472c-bb8e-4c3d59ef0919.png`
- Written target adjustment: 按用户要求完整回退到自定义 HWND region 之前的
  DWM 小圆角版本，不再继续修补后加裁剪方案。
- Browser implementation screenshot:
  `.maintainer/qa/clear-vertical-small-radius-browser.png`
- Browser capture pixels / CSS viewport: `240 × 420` at device scale `1`; in-app
  browser enforces a `240px` minimum width, so this is structural evidence rather than a
  like-for-like native composition comparison.
- Windows native implementation: restarted HWND `921752`; an in-turn Windows capture was
  visually inspected, but the capture surface did not provide a filesystem path for a
  normalized side-by-side artifact.
- State: clear vertical strip；实时额度和桌面取景允许变化。

## Full-view comparison

把自定义半径缩小到 `6px` 仍然失败：旧 region 在卡片/胶囊变形后保留错误裁剪，
造成标题栏残片、左右白条和底部异常。当前已完整删除自定义 region、相关命令和所有
尺寸同步调用，重启后的原生截图恢复为连续玻璃背景和 DWM 小圆角。

## Focused comparison

重点检查失败截图中的标题栏残片、两侧白边和大圆头。重启后的原生捕获中三项均已
消失，窗口重新呈现此前的小系统圆角。由于当前捕获没有可保存路径，本轮没有伪造
等尺寸并排文件，最终证据状态仍按流程标记为 blocked。

## Required fidelity surfaces

- Fonts and typography: 字体家族、字号、字重和布局未改；clear 只改变前景色与
  text-shadow。大号数据和小号辅助文字均未发生换行或裁切。
- Spacing and layout rhythm: Windows 使用唯一 DWM 外轮廓；桌面 `#root` 和 shell
  均为零圆角，不再参与窗口裁剪。浏览器模拟值保持 compact `18px`、horizontal
  `20px`、vertical `16px`。
- Colors and visual tokens: clear 前景为 `rgba(255,255,255,0.94)`；文字阴影为深蓝灰；
  root 冷白 tint、品牌色、告警色和额度条颜色保持原方案。
- Image quality and asset fidelity: 使用现有 ChatGPT/Claude 品牌图标，没有新增、
  替换或模拟图片资产。桌面背景是真实 Windows 合成结果。
- Copy and content: 文案未改；实时统计数字不同属于正常数据变化。

## Interaction and structural checks

- Windows native clear vertical strip: 重启后捕获显示残片、白条和大圆头已消失。
- Browser vertical computed style:
  browser runtime 单独保留模拟圆角；shell `border 0`, `radius 0`, `shadow none`.
- Desktop CSS contract:
  clear root 与 shell 均为零 border/radius/shadow；仓库中不存在 `SetWindowRgn`、
  `set_window_shape` 或窗口形状同步调用。
- 指针移到胶囊外缘后，`#root --glass-edge-opacity` 从 `0` 变为 `1`。
- 卡片 ↔ 胶囊切换仍可用。
- Browser console errors: none.

## Comparison history

### Iteration 0 — blocked

- [P2] Clear 前景仍是深色，和用户选定的白色方向不符。
- [P2] root 与 shell 同时画圆角边框，四角出现明显双框。

Fixes:

- clear 不再挂 `--glass-light`，改为继承透明 HUD 的白色前景体系；
- 添加暗色 text-shadow 和 SVG drop-shadow；
- 静态描边与动态流光迁移到 `#root`；
- clear shell 改为无 border、无 radius、无 shadow；
- 指针 CSS 变量改写到 `#root`。

### Iteration 1 — failed

- [P2] shell 双框已消失，但 Windows HWND 与 root 仍分别裁切圆角，在透明卡片和
  竖胶囊顶部形成第二条内缩圆弧。

Fixes:

- desktop clear root 改为零圆角、无静态 shadow，完全服从 HWND 外框；
- browser clear root 单独保留 CSS 模拟圆角和描边；
- 实现文档明确区分 HWND 外框所有权与浏览器模拟外框。

### Iteration 2 — passed

Windows 原生透明竖胶囊和浏览器 clear compact 均确认外框所有权已经分离，没有发现
新的 P0/P1/P2。

### Iteration 3 — rejected after handoff

- [P2] Windows 默认圆角虽然消除了上一轮双框，但竖胶囊顶部仍有系统边界痕迹，
  底部半径偏小且无法随胶囊缩放变化。

Fixes:

- 悬浮窗口关闭 DWM 固定圆角和系统边线；
- 新增按实际客户区短边比例计算的 HWND rounded region；
- 卡片、横胶囊、竖胶囊分别使用 `18/320`、`1/2`、`16/48`；
- 所有 resize、DPI 重断言和形态切换路径都会重新应用 region；
- expanded 清除自定义 region 并恢复默认窗口外观。

用户后续截图证明该判断错误：`16/48` 在 145% 缩放下形成约 `22px` 的大圆头，且
高亮静态描边让窗口背景边更加明显。

### Iteration 4 — blocked pending native capture

- [P1] 胶囊轮廓被放大成大圆头，与选定参考图的轻微圆角不符。
- [P2] 常驻白色外沿过亮，看起来像不透明背景边。

Fixes:

- 横竖胶囊设计半径统一收回 `6px`；
- HWND 比例改为 horizontal `6/40`、vertical `6/48`；
- 参考机器 `66px` 竖条短边的测试半径由 `22px` 降为 `8px`；
- 静态外沿 alpha 从 `0.72/0.86/0.30` 降为 `0.30/0.42/0.16`；
- shell 继续保持零 border/radius/shadow，避免双框。

Post-fix evidence: 浏览器计算样式、局部截图与 region 单元测试通过；Windows 原生
工具窗口当前未被捕获接口枚举，因此底部两角和桌面背景收口仍待一次原生截图确认。

### Iteration 5 — rollback completed; comparison artifact blocked

- [P1] 缩小半径没有解决错误 region 的生命周期问题，反而出现标题栏残片和白条。

Rollback:

- 删除 `SetWindowRgn` / `CreateRoundRectRgn` 实现和 GDI feature；
- 删除 `set_window_shape` 命令以及所有 resize / DPI / mode 切换调用；
- 恢复 DWM 默认小圆角；
- Windows 桌面 `#root` 恢复零圆角、零静态描边；
- CSS 模拟圆角重新限制到 browser runtime；
- 完整重启进程，确保旧 HWND region 不会残留。

Post-rollback evidence: 新 HWND 的原生捕获显示标题栏残片、左右白条和大圆头均已
消失。捕获没有文件路径，无法按流程制作同尺寸并排图，因此不将 QA 标成 passed。

## Follow-up polish

- [P3] 极亮、近纯白壁纸上，低不透明度辅助文字仍可能接近最低舒适对比度；当前
  双层文字阴影已明显改善。后续如需加强，应只调 clear 的阴影或轻微 scrim，不要
  恢复深色前景或第二层面板。
- Windows forced-colors 和 reduced-transparency 已通过 CSS 规则检查，本轮没有在
  系统设置中再次切换验证。
- macOS 原生视觉未重跑；本次修改只作用于非 macOS 的 `--glass-clear`。

## Verification

- `npm test` — 18/18 passed.
- `npm run build` — passed.
- `git diff --check` — passed.
- Windows native clear vertical strip — visually captured after full process restart.
- Browser clear vertical strip — computed styles and console checked.
- Custom window-region implementation — removed; no references remain.

final result: blocked
