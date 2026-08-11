import assert from "node:assert/strict";
import test from "node:test";

import { QUOTA_LOW_REMAINING, bindingWindow } from "./quotaWindows.js";

function windowOf(key, remainingPercent, { resetExpired = false, available = true } = {}) {
  return { key, view: { available, remainingPercent, resetExpired } };
}

test("5h 还满着但每周快见底时显示每周", () => {
  const picked = bindingWindow([
    windowOf("five_hour", 95),
    windowOf("seven_day", 13),
  ]);
  assert.equal(picked.key, "seven_day");
});

test("两个窗口都充足时稳定显示短窗", () => {
  const picked = bindingWindow([
    windowOf("five_hour", 99),
    windowOf("seven_day", 97),
  ]);
  assert.equal(picked.key, "five_hour");
});

test("较长窗口只是略低时不接管", () => {
  const picked = bindingWindow([
    windowOf("five_hour", 80),
    windowOf("seven_day", 40),
  ]);
  assert.equal(picked.key, "five_hour");
});

test("越过告急线时长窗接管", () => {
  const picked = bindingWindow([
    windowOf("five_hour", 80),
    windowOf("seven_day", QUOTA_LOW_REMAINING),
  ]);
  assert.equal(picked.key, "seven_day");
});

test("两个窗口都告急时取剩余更少的窗口", () => {
  const picked = bindingWindow([
    windowOf("five_hour", 12),
    windowOf("seven_day", 4),
  ]);
  assert.equal(picked.key, "seven_day");
});

test("Codex 没有 5h 窗口时落到每周", () => {
  const picked = bindingWindow([windowOf("secondary", 53)]);
  assert.equal(picked.key, "secondary");
});

test("任一窗口归零就显示该窗口", () => {
  const picked = bindingWindow([
    windowOf("five_hour", 100),
    windowOf("seven_day", 0),
  ]);
  assert.equal(picked.key, "seven_day");
  assert.equal(picked.view.remainingPercent, 0);
});

test("告急窗口并列时保持短周期优先", () => {
  const picked = bindingWindow([
    windowOf("five_hour", 5),
    windowOf("seven_day", 5),
  ]);
  assert.equal(picked.key, "five_hour");
});

test("已过重置点的旧读数不参与比较", () => {
  const picked = bindingWindow([
    windowOf("five_hour", 100, { resetExpired: true }),
    windowOf("seven_day", 0, { resetExpired: true }),
    windowOf("monthly_cycle", 57.24),
  ]);
  assert.equal(picked.key, "monthly_cycle");
});

test("没有来源的窗口不参与比较", () => {
  const picked = bindingWindow([
    windowOf("five_hour", 0, { available: false }),
    windowOf("seven_day", 80),
  ]);
  assert.equal(picked.key, "seven_day");
});

test("全部失效时返回 null", () => {
  assert.equal(bindingWindow([windowOf("five_hour", 0, { resetExpired: true })]), null);
  assert.equal(bindingWindow([]), null);
  assert.equal(bindingWindow(undefined), null);
});
