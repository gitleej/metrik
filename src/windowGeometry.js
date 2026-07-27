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

/// WebView2 偶尔在混合 DPI 环境保留一层额外 zoom，导致原生窗口物理尺寸正确，
/// 但 CSS 视口仍偏小。以当前“物理像素 / CSS 视口”的实测比例反算目标尺寸，
/// 不再假设系统 DPI 与 setZoom 就是完整缩放链。
function viewportCorrectedPhysicalSize({
  currentPhysicalWidth,
  currentPhysicalHeight,
  viewportWidth,
  viewportHeight,
  expectedWidth,
  expectedHeight,
}) {
  const values = [
    currentPhysicalWidth,
    currentPhysicalHeight,
    viewportWidth,
    viewportHeight,
    expectedWidth,
    expectedHeight,
  ];
  if (values.some((value) => !Number.isFinite(value) || value <= 0)) return null;

  const widthRatio = expectedWidth / viewportWidth;
  const heightRatio = expectedHeight / viewportHeight;
  // 防止隐藏/切形态的瞬时 0 尺寸把窗口放大到不可恢复；正常 DPI/zoom
  // 残差都落在 0.5–2 之间（应用自身允许的缩放范围也是 0.75–2）。
  if (
    widthRatio < 0.5 ||
    widthRatio > 2 ||
    heightRatio < 0.5 ||
    heightRatio > 2
  ) {
    return null;
  }
  return {
    width: Math.max(1, Math.round(currentPhysicalWidth * widthRatio)),
    height: Math.max(1, Math.round(currentPhysicalHeight * heightRatio)),
  };
}

function viewportCorrectedZoom({
  contentScale,
  viewportWidth,
  viewportHeight,
  expectedWidth,
  expectedHeight,
}) {
  const values = [
    contentScale,
    viewportWidth,
    viewportHeight,
    expectedWidth,
    expectedHeight,
  ];
  if (values.some((value) => !Number.isFinite(value) || value <= 0)) return null;

  const widthRatio = viewportWidth / expectedWidth;
  const heightRatio = viewportHeight / expectedHeight;
  // 真实 zoom 应在两条轴上给出近似相同的比例；差异过大说明布局/尺寸仍在变化，
  // 此时交给物理窗口兜底，不贸然改变内容缩放。
  if (
    widthRatio < 0.5 ||
    widthRatio > 2 ||
    heightRatio < 0.5 ||
    heightRatio > 2 ||
    Math.abs(widthRatio - heightRatio) > 0.08
  ) {
    return null;
  }
  return contentScale * ((widthRatio + heightRatio) / 2);
}

function horizontalStripTargetWidth({
  cellCount,
  cellWidth,
  controlsWidth,
  paddingLeft,
  paddingRight,
  gap,
  roundingAllowance = 1,
}) {
  const cells = Math.max(1, Math.round(cellCount || 0));
  return (
    cells * cellWidth +
    controlsWidth +
    paddingLeft +
    paddingRight +
    cells * gap +
    roundingAllowance
  );
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

export {
  horizontalStripTargetWidth,
  monitorForWindowPosition,
  physicalWindowSize,
  viewportCorrectedPhysicalSize,
  viewportCorrectedZoom,
};
