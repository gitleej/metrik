/// 一个 Agent 可以同时受几个额度窗口约束（5 小时 / 每周 / 月度）。卡片行和
/// 胶囊格只有一个数字的位置，这个文件决定该显示哪一个。
///
/// 平时显示周期最短的窗口；有窗口告急时，改显示告急窗口里剩余最少的。
/// 这个数字必须回答“现在还能不能用”，而不是只显示一个看起来最宽裕的窗口。

/// “告急”的界线，与 App.jsx 里 quotaSeverity 的 warn 档共用。
export const QUOTA_LOW_REMAINING = 15;

/// 从后端已按周期排序的窗口列表里挑出当前真正约束用量的那个。
/// 已过重置点的旧读数和没有来源的窗口不参与选择。
export function bindingWindow(windows) {
  const live = (windows || []).filter(
    (window) => window.view.available && !window.view.resetExpired,
  );
  if (!live.length) return null;
  const low = live.filter(
    (window) => window.view.remainingPercent <= QUOTA_LOW_REMAINING,
  );
  if (!low.length) return live[0];
  return low.reduce((tightest, window) =>
    window.view.remainingPercent < tightest.view.remainingPercent ? window : tightest,
  );
}
