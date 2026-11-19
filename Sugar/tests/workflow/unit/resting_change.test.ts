/** A burst of changes is reported once, after it stops. */
import { expect, test } from '@jest/globals';
import { RestingChange } from '../../../frontend/comfyui/ui/workflow/infrastructure/RestingChange.js';

/** Timers that fire only when the test says time has passed. */
function timers() {
  let next = 0;
  const pending = new Map<number, { at: number; run: () => void }>();
  let now = 0;
  return {
    setTimeout: (run: () => void, milliseconds: number) => {
      next += 1;
      pending.set(next, { at: now + milliseconds, run });
      return next;
    },
    clearTimeout: (timer: number) => void pending.delete(timer),
    advance: (milliseconds: number) => {
      now += milliseconds;
      for (const [timer, entry] of [...pending])
        if (entry.at <= now) {
          pending.delete(timer);
          entry.run();
        }
    },
    waiting: () => pending.size,
  };
}

test('changes that keep coming are reported once, a full quiet period after the last', () => {
  const clock = timers();
  let reports = 0;
  const change = new RestingChange(clock, 1000, () => (reports += 1));
  for (let frame = 0; frame < 120; frame += 1) {
    change.changed();
    clock.advance(16);
  }
  expect(reports).toBe(0);
  expect(clock.waiting()).toBe(1);
  clock.advance(983);
  expect(reports).toBe(0);
  clock.advance(17);
  expect(reports).toBe(1);
  clock.advance(5000);
  expect(reports).toBe(1);
  change.changed();
  clock.advance(1000);
  expect(reports).toBe(2);
});

test('an owner that read the current state itself spends the waiting report', () => {
  const clock = timers();
  let reports = 0;
  const change = new RestingChange(clock, 1000, () => (reports += 1));
  change.changed();
  change.settle();
  change.settle();
  clock.advance(5000);
  expect(reports).toBe(0);
  expect(clock.waiting()).toBe(0);
});
