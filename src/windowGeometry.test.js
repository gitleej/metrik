import assert from "node:assert/strict";
import test from "node:test";

import {
  monitorForWindowPosition,
  physicalWindowSize,
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
