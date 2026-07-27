import assert from "node:assert/strict";
import test from "node:test";

import {
  horizontalStripTargetWidth,
  monitorForWindowPosition,
  physicalWindowSize,
  viewportCorrectedPhysicalSize,
  viewportCorrectedZoom,
} from "./windowGeometry.js";

function monitor(x, width, scaleFactor, workHeight = 1080) {
  return {
    position: { x, y: 0 },
    size: { width, height: workHeight },
    workArea: {
      position: { x, y: 0 },
      size: { width, height: workHeight - 40 },
    },
    scaleFactor,
  };
}

test("physical size combines app scale with destination monitor DPI", () => {
  assert.deepEqual(physicalWindowSize(320, 440, 1.25, 1.5), {
    width: 600,
    height: 825,
  });
  assert.deepEqual(physicalWindowSize(52, 272, 1, 1.75), {
    width: 91,
    height: 476,
  });
});

test("remembered position selects the destination monitor instead of the current one", () => {
  const primary = monitor(0, 1920, 1.25);
  const secondary = monitor(1920, 3840, 2);
  const selected = monitorForWindowPosition(
    [primary, secondary],
    { x: 2400, y: 120 },
    { width: 320, height: 440 },
    1,
  );
  assert.equal(selected, secondary);
  assert.deepEqual(physicalWindowSize(52, 272, 1, selected.scaleFactor), {
    width: 104,
    height: 544,
  });
});

test("overlap chooses the monitor that will contain most of the restored window", () => {
  const left = monitor(-1920, 1920, 1.5);
  const right = monitor(0, 1920, 2);
  const selected = monitorForWindowPosition(
    [left, right],
    { x: -120, y: 80 },
    { width: 320, height: 320 },
    1,
  );
  assert.equal(selected, right);
});

test("fully off-screen remembered positions do not select a monitor", () => {
  const selected = monitorForWindowPosition(
    [monitor(0, 1920, 1.5)],
    { x: 9000, y: 9000 },
    { width: 320, height: 440 },
    1,
  );
  assert.equal(selected, null);
});

test("horizontal strip width includes every outer flex gap", () => {
  assert.equal(
    horizontalStripTargetWidth({
      cellCount: 2,
      cellWidth: 68,
      controlsWidth: 146,
      paddingLeft: 6,
      paddingRight: 5,
      gap: 4,
    }),
    302,
  );
  assert.equal(
    horizontalStripTargetWidth({
      cellCount: 0,
      cellWidth: 68,
      controlsWidth: 146,
      paddingLeft: 6,
      paddingRight: 5,
      gap: 4,
    }),
    230,
  );
});

test("runtime viewport corrects a hidden WebView zoom layer", () => {
  assert.deepEqual(
    viewportCorrectedPhysicalSize({
      currentPhysicalWidth: 560,
      currentPhysicalHeight: 560,
      viewportWidth: 256,
      viewportHeight: 256,
      expectedWidth: 320,
      expectedHeight: 320,
    }),
    { width: 700, height: 700 },
  );
});

test("runtime viewport correction rejects transient invalid measurements", () => {
  assert.equal(
    viewportCorrectedPhysicalSize({
      currentPhysicalWidth: 560,
      currentPhysicalHeight: 560,
      viewportWidth: 0,
      viewportHeight: 0,
      expectedWidth: 320,
      expectedHeight: 320,
    }),
    null,
  );
});

test("runtime viewport can cancel a hidden WebView zoom layer", () => {
  assert.equal(
    viewportCorrectedZoom({
      contentScale: 1,
      viewportWidth: 256,
      viewportHeight: 256,
      expectedWidth: 320,
      expectedHeight: 320,
    }),
    0.8,
  );
});
