# Widget 数据共享困境 — 交接文档

> 日期:2026-08-13
> 分支:`codex/fix-macos-widget-bundle`(未提交,全在工作树)
> 作者:ZCode agent 会话(接手自 Codex session `019ff620`)

---

## 一句话现状

**菜单栏(menubar tray)已经完全修好**;**桌面 WidgetKit 小组件的数据刷新卡住**——host app 能把数据写到磁盘上的正确位置,但 Widget extension 进程不读它,根因尚未定位。用户已要求暂停。

---

## 用户最初的诉求

1. 菜单栏状态栏图标里缺 GLM(只显示 Kimi + ChatGPT)
2. macOS 版有个"莫名其妙的自定义 agent",要删掉,和 Win 统一
3. 软件里用户自己选哪些 agent、排什么顺序,菜单栏和小组件都要对应显示

---

## 已完成、确定有效的改动 ✅

### 1. 彻底删除「自定义 agent」(custom)功能

前后端 + 文档全删,**编译 + 测试全绿**(npm test 38 passed,cargo test 200 passed)。

| 文件 | 改动 |
|---|---|
| `src/App.jsx` | 删 `AGENT_META.custom`、`CustomSourcesCard` 组件、import |
| `src/usageClient.js` | 删 `getCustomSources`/`setCustomSources` |
| `src/styles.css` | 删 `.agent-icon--custom` |
| `src-tauri/src/domain.rs` | `AGENT_IDS` 从 9→8,删 `"custom"` |
| `src-tauri/src/engine.rs` | 删 6 处 custom 逻辑(declared_custom、source_views 参数、custom-local SourceView 等) |
| `src-tauri/src/custom_sources.rs` | **整个文件删除** |
| `src-tauri/src/custom_sources_e2e.rs` | **整个文件删除** |
| `src-tauri/src/adapters/claude.rs` | 删 `for_custom_sources` |
| `src-tauri/src/lib.rs` | 删 mod 声明、2 个 command、invoke_handler 注册 |
| `src-tauri/src/storage.rs` | `matches!("claude"|"custom")` → `== "claude"` |
| `src-tauri/src/widget_snapshot.rs` | 删 `agent_label` 的 custom 映射 |
| `src-tauri/src/detect.rs` | 删 custom 探针 |
| `docs/ARCHITECTURE.md` | 删 User-declared sources 段落 |

### 2. 修复菜单栏 tray 的 agent 显示

菜单栏数据链路完全打通,已验证:
- GLM 图标出现,显示真实 Session 配额百分比
- WorkBuddy/Qoder label 正确(不再退化成 "Agent")
- custom 不再出现
- agent 顺序跟随用户设置

### 3. 修复 `--publish-widget-snapshot` CLI 不传 agent 顺序

`src-tauri/src/lib.rs` 的 `publish_widget_snapshot_from_database`:从 `build_cached_snapshot`(只读)改成 `build_snapshot`(真扫描),并从数据库读出用户保存的 `macos_widget_agents`(含顺序)传给 persist。改用 `None` 会让 Widget 按固定 AGENT_IDS 顺序显示,与用户设置不一致。

### 4. 修复 Widget 后台不自动更新的设计缺陷(代码层面)

`src/App.jsx:4611` 的 `document.visibilityState === "visible"` 守卫导致合上面板后 setInterval 不跑 → 扫描停止。代码已确认,CLI 路径修复已合入(见上条),但**完整的前端独立后台调度未做**。

---

## 卡住的地方 ❌:桌面 WidgetKit 小组件不刷新

### 现象
- host app(通过 `metrik-widget-publish` helper)把 JSON 写到 `~/Library/Containers/app.metrik.desktop.widget/Data/Library/Preferences/app.metrik.desktop.widget.plist` —— **成功,内容正确**(GLM 排第一、数据齐全)
- 但桌面小组件 UI **始终显示旧数据**(ChatGPT 66%、GLM 94%),杀 widget 进程、重新注册 appex、reload、删 plist 都试过,**不刷新**
- 最终截图显示组件甚至变成空白

### 已排除的可能性
- ❌ 不是数据没写对(plist 文件存在、内容验证正确)
- ❌ 不是 entitlements 缺 App Group(加过、删过,都不行)
- ❌ 不是 publisher 路径错(CLI 输出确认写到 widget container)
- ❌ 不是 Swift bindingWindow 逻辑错(模拟运行结果正确)
- ❌ 不是 agent 顺序错(plist 里 GLM 确实第一)

### 当前方案(Codex 原方案,已回退到此)
publisher helper 嵌入 widget extension 的 bundle identity(`app.metrik.desktop.widget`),让 `UserDefaults.standard` 解析进 widget 的 sandbox container。两个 entitlements 只保留 `app-sandbox`,**不含 `application-groups`**。

```swift
// publisher: UserDefaults.standard.set(data, forKey: "widgetSnapshotJSON")
// widget:    UserDefaults.standard.data(forKey: "widgetSnapshotJSON")
```

### 怀疑的根因(未证实)
1. **WidgetKit timeline 缓存**:Widget 的 `getTimeline` 用 `policy: .after(now + 5min)`,系统在到期前不重调。但杀进程、reload、删组件重加都试过仍不刷新,说明可能不止是 timeline 问题。
2. **bundle-identity 共享在当前 macOS 版本下可能失效**:publisher 写到的 `app.metrik.desktop.widget.plist` 和 widget extension 进程的 `UserDefaults.standard` 读的,可能因为 macOS sandbox 改进而不再解析到同一处。需要用 `log show` 抓 widget 进程的 `MetrikWidgetStore.load()` 日志确认(但 release build 的 os_log 被 stripped,抓不到)。
3. **ad-hoc 签名下 entitlements 处理变化**:没有 Team ID,系统对 widget extension 的 sandbox container 归属可能有特殊处理。

### 已验证的关键事实(给排查用)
- Widget extension 进程路径:`/Applications/Metrik.app/Contents/PlugIns/MetrikWidget.appex/Contents/MacOS/MetrikWidget`
- Widget container:`~/Library/Containers/app.metrik.desktop.widget/Data`
- Widget 能读到的 plist(历史成功过):`~/Library/Containers/app.metrik.desktop.widget/Data/Library/Preferences/app.metrik.desktop.widget.plist`
- Group Container(试过,widget 读不到):`~/Library/Group Containers/group.app.metrik.desktop/`
- widget extension bundle id:`app.metrik.desktop.widget`
- publisher 嵌入的 bundle id:同上(通过 `-sectcreate __TEXT __info_plist`)
- 无 Apple Developer Team ID(ad-hoc 签名)

---

## 下一步建议(给接手的人)

### 优先尝试:确认 Widget 到底读到了什么
关键是用 **debug build** 的 widget extension 抓到 `MetrikWidgetStore.load()` 的 os_log 输出:
```bash
# debug build 带 os_log,能抓到 "Shared snapshot decoded/missing/decode failed"
log stream --predicate 'subsystem == "app.metrik.desktop.widget"' --style compact
```
如果看到 "missing" → 是路径/suite 问题;看到 "decode failed" → 是数据格式问题;看到 "decoded with N agents" 但 UI 没变 → 是 WidgetKit 渲染缓存。

### 备选方向
1. **看其它无 Team ID 的开源 macOS widget app 怎么共享数据**。GitHub 上 `metriks`/`raycast`/`itsycal` 类项目的 widget extension 数据通道值得参考。Codex 和本会话都钻进了 App Group,但别的方案可能根本不用 App Group(比如 widget extension 自己扫日志,或用 `NSFileCoordinator` + shared file extension)。
2. **widget extension 内嵌轻量扫描器**:不依赖 host 传数据,widget 自己在 `getTimeline` 里直接读 `~/.codex/sessions`、`~/.zcode/cli/db/db.sqlite` 等。代价是 widget 进程要自己做解析,但彻底绕开共享存储难题。注意 widget sandbox 对 `~` 的访问需要 entitlement。
3. **申请一个免费的 Apple Developer Team ID**。用户明确说没有账号,但免费 developer signing 也带 Team ID,能让 App Group 正式生效。这是最标准但需要用户行动的方案。

### 不要再重复的弯路
- ❌ 不要再反复改 entitlements 加/删 App Group —— 试过了,都不影响结果
- ❌ 不要再改 `UserDefaults(suiteName:)` vs `UserDefaults.standard` vs 文件 —— 三种都试过,数据都能写,widget 都不读
- ❌ 不要只验证"publisher 写成功"就以为通了 —— 必须验证 **widget 进程的 load() 读到了**

---

## 工作树状态(未提交)

所有改动在 `codex/fix-macos-widget-bundle` 分支的工作树里,**未 commit**。

- 已删除 custom 的改动是干净、完整的,可以单独提取成一个 commit
- Widget 数据共享的 Swift 改动目前回退到了 Codex 原方案(`UserDefaults.standard` + bundle identity)
- `/Applications/Metrik.app` 已安装最新构建(debug 用)

### 建议的 commit 拆分
1. `Remove the custom agent feature`(前端 + 后端 + 文档,全绿)
2. `Pass user's widget agent order through the CLI publisher`(lib.rs 一处)
3. Widget 数据共享问题:暂不 commit,等定位到根因

---

---

## 后续(2026-08-13 00:13 CST):已解决 ✅

**根因:`/Applications/Metrik.app` 里装的 widget extension 是旧的 app-group 构建,从来不是代码 bug。** 工作树的 `UserDefaults.standard` + bundle-identity 方案本身就是对的,但此前"已安装最新构建"的说法不成立——安装的 appex 二进制里还带着 `group.app.metrik.desktop`(`strings` 可验证),entitlements 也还带着 application-groups。

### 排查定论(按本文档"优先尝试"节的 log 方案抓到)

1. `log stream --predicate 'subsystem == "app.metrik.desktop.widget"'` 抓到 widget 进程反复报 "Shared snapshot preference missing"(error/notice 级日志 release build 也会输出,**不需要 debug build**)。
2. `log show` 抓 cfprefsd:widget 进程实际读的 domain 是 `group.app.metrik.desktop`,被 cfprefsd 拒绝(ad-hoc 签名无 Team ID,group container 访问被拒)→ 证明跑的是旧二进制。
3. 对照实验:同方式构建的 sandboxed 测试 reader(嵌同样 bundle id)用 `UserDefaults.standard` 能读到 1528 字节 → 写入链路和 bundle-identity 共享机制本身没问题。

### 修复动作

```bash
/bin/zsh scripts/build-macos-widget-extension.sh   # 重新构建 appex + helpers
# 替换 /Applications/Metrik.app/Contents/{PlugIns/MetrikWidget.appex, Helpers/*}
codesign --force --sign - /Applications/Metrik.app  # 重签外层
pkill -f MetrikWidget.appex; killall chronod        # 必须杀旧 extension 进程,kill chronod 不够
```

### 验证

CLI `--publish-widget-snapshot` 写入后 150ms 内,新 extension 进程日志报 "Shared snapshot decoded with 4 agents"(GLM 第一,顺序正确),链路 host → publisher → widget container plist → cfprefsd → widget load() 全通。

### 教训(增补"不要再重复的弯路")

- ❌ 不要只重装 app 就以为 appex 更新了——用 `strings …/MetrikWidget | grep group.app` 或 entitlements dump 验证装进 /Applications 的二进制确实是新构建。
- ❌ `killall chronod` 不会杀 extension 进程,旧 `MetrikWidget` 进程会继续用磁盘上已替换的旧镜像响应 timeline 请求,必须 `pkill -f MetrikWidget.appex`。
- ✅ 怀疑部署层面问题时,先对比"运行的二进制"和"源码",再改代码。

---

## 附:本会话里 Codex 的前科

这个会话接手自 Codex session `019ff620`,它在 Widget 上空转了 1 小时+、改了 entitlements 多次、compaction 了一次。它的最后状态:
- 改了源码但装进 /Applications 的二进制是半成品(agent_label 缺 workbuddy/qoder 映射)
- 在工作树里删了 App Group entitlements(commit b9b8836 加的),改成 bundle-identity 方案
- 从未验证 widget extension 进程真的能读到数据
- 把 widget container plist 弄出了多份不一致的缓存(Group Container 一份、widget container 一份、publisher container 一份)

接手时应先 `git stash` 或逐文件 review 工作树,不要直接在它的基础上继续叠改动。
