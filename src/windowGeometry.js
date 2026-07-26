function normalizedScale(value) {
  return Number.isFinite(value) && value > 0 ? value : 1;
}

function monitorArea(monitor) {
  if (!monitor?.position || !monitor?.size) return null;
  return {
    x: monitor.workArea?.position?.x ?? monitor.position.x,
    y: monitor.workArea?.position?.y ?? monitor.position.y,
    width: monitor.workArea?.size?.width ?? monitor.size.width,
    height: monitor.workArea?.size?.height ?? monitor.size.height,
  };
}

function overlapArea(rect, area) {
  const overlapX =
    Math.min(rect.x + rect.width, area.x + area.width) - Math.max(rect.x, area.x);
  const overlapY =
    Math.min(rect.y + rect.height, area.y + area.height) - Math.max(rect.y, area.y);
  return Math.max(0, overlapX) * Math.max(0, overlapY);
}

/// 把 CSS 设计尺寸按应用缩放与目标显示器 DPI 换成整数物理像素。
function physicalWindowSize(width, height, contentScale = 1, monitorScale = 1) {
  const scale = normalizedScale(contentScale) * normalizedScale(monitorScale);
  return {
    width: Math.round(width * scale),
    height: Math.round(height * scale),
  };
}

/// 记忆坐标是物理像素；用每台显示器自己的 DPI 推导候选窗口大小，再选与工作区
/// 重叠最多的显示器。不能先读当前窗口 DPI——窗口随后可能恢复到另一台屏幕。
function monitorForWindowPosition(
  monitors,
  position,
  logicalSize,
  contentScale = 1,
) {
  if (
    !position ||
    !Number.isFinite(position.x) ||
    !Number.isFinite(position.y) ||
    !logicalSize ||
    !Number.isFinite(logicalSize.width) ||
    !Number.isFinite(logicalSize.height)
  ) {
    return null;
  }

  let best = null;
  let bestOverlap = 0;
  (monitors || []).forEach((monitor) => {
    const area = monitorArea(monitor);
    if (!area) return;
    const physical = physicalWindowSize(
      logicalSize.width,
      logicalSize.height,
      contentScale,
      monitor.scaleFactor,
    );
    const overlap = overlapArea(
      { x: position.x, y: position.y, ...physical },
      area,
    );
    if (overlap > bestOverlap) {
      best = monitor;
      bestOverlap = overlap;
    }
  });
  return best;
}

export { monitorForWindowPosition, physicalWindowSize };
