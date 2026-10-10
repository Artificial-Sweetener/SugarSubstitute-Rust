//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Prove bounded drawer travel, exact timeline reversal, and deterministic presentation teardown. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { CubeStackDrawerMotion } from '../../../frontend/comfyui/ui/surface/stack/CubeStackDrawerMotion.js';
import { fluentMotion } from '../../../frontend/comfyui/ui/surface/appearance/FluentThemeRuntime.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

test('cascades cubes upward without overshoot and finishes every effect with masonry', async () => {
  const f = fixture(12);
  const motion = new CubeStackDrawerMotion(f.element);
  const entering = motion.setVisible(true);
  expect(f.effects).toHaveLength(14);
  expect(f.element.inert).toBe(true);
  expect(f.element.dataset.stackMotion).toBe('enter');
  for (const effect of f.effects) expect(effect.options.duration).toBe(fluentMotion.duration);
  expect(f.effects[0]?.options.easing).toBe(fluentMotion.easing);
  for (const effect of f.effects.slice(1)) {
    expect(effect.frames[0]?.translate).toBe('0 32px');
    expect(effect.frames.at(-1)?.translate).toBe('0 0');
    expect(effect.frames.at(-1)?.offset).toBe(1);
    expect(effect.frames.some((frame) => String(frame.translate).includes('-'))).toBe(false);
    expect(effect.frames.at(-2)?.offset).toBeLessThanOrEqual(0.25);
    expect(effect.frames.at(-2)?.easing).toBe(fluentMotion.easing);
  }
  expect(f.effects[2]?.frames.at(-2)?.offset).toBeGreaterThan(0);
  f.finish();
  expect(await entering).toBe(true);
  expect(motion.settledVisible).toBe(true);
  expect(f.element.style.translate).toBe('none');
  expect(f.element.style.opacity).toBe('1');
  expect(f.element.inert).toBe(false);
  expect(f.rows[0]?.style.transform).toBe('translateX(12px)');
  expect(f.effects.every((effect) => effect.cancel.mock.calls.length === 1)).toBe(true);
  motion.dispose();
});

test('reverses in place without new effects, stale completions, or restarting a cascade', async () => {
  const f = fixture(3);
  const motion = new CubeStackDrawerMotion(f.element);
  const entering = motion.setVisible(true);
  for (const effect of f.effects) effect.currentTime = 73;
  const leaving = motion.setVisible(false);
  expect(f.effects).toHaveLength(5);
  expect(f.effects.every((effect) => effect.currentTime === 73)).toBe(true);
  for (const effect of f.effects) expect(effect.updatePlaybackRate).toHaveBeenLastCalledWith(-1);
  const reopening = motion.setVisible(true);
  expect(f.effects).toHaveLength(5);
  for (const effect of f.effects) expect(effect.updatePlaybackRate).toHaveBeenLastCalledWith(1);
  f.finish();
  expect(await entering).toBe(false);
  expect(await leaving).toBe(false);
  expect(await reopening).toBe(true);
  expect(f.element.dataset.stackMotion).toBeUndefined();
  expect(f.element.style.opacity).toBe('1');
  motion.dispose();
});

test('plays the same complete cascade backward when hiding a settled stack', async () => {
  const f = fixture(2);
  const motion = new CubeStackDrawerMotion(f.element);
  const entry = motion.setVisible(true);
  f.finish();
  await entry;
  const oldFrames = f.effects.map((effect) => effect.frames);
  const exit = motion.setVisible(false);
  const outgoing = f.effects.slice(4);
  expect(outgoing.map((effect) => effect.frames)).toEqual(oldFrames);
  for (const effect of outgoing) {
    expect(effect.currentTime).toBe(fluentMotion.duration);
    expect(effect.updatePlaybackRate).toHaveBeenLastCalledWith(-1);
  }
  f.finish();
  expect(await exit).toBe(true);
  expect(f.element.style.opacity).toBe('0');
  expect(f.element.inert).toBe(true);
  motion.dispose();
});

test('cancels a prepared or moving surface without leaving compositor effects or stale promises', async () => {
  const f = fixture(1);
  const motion = new CubeStackDrawerMotion(f.element);
  expect(f.element.style.opacity).toBe('0');
  const pending = motion.setVisible(true);
  motion.dispose();
  expect(await pending).toBe(false);
  expect(f.effects.every((effect) => effect.cancel.mock.calls.length === 1)).toBe(true);
  expect(f.element.dataset.stackMotion).toBeUndefined();
  expect(f.element.inert).toBe(false);
  expect(await motion.setVisible(true)).toBe(false);
});

test('shows and hides immediately for reduced motion or absent compositor support', async () => {
  const f = fixture(2);
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    value: () => ({ matches: true }),
  });
  try {
    const motion = new CubeStackDrawerMotion(f.element);
    expect(await motion.setVisible(true)).toBe(true);
    expect(f.effects).toHaveLength(0);
    expect(f.element.style.opacity).toBe('1');
    expect(f.element.style.translate).toBe('none');
    expect(await motion.setVisible(false)).toBe(true);
    expect(f.element.style.opacity).toBe('0');
    motion.dispose();
  } finally {
    Reflect.deleteProperty(window, 'matchMedia');
  }
  Reflect.deleteProperty(f.element, 'animate');
  const motion = new CubeStackDrawerMotion(f.element);
  expect(await motion.setVisible(true)).toBe(true);
  expect(f.element.style.opacity).toBe('1');
  motion.dispose();
});

test('reports unexpected compositor failure instead of treating it as a completed entrance', async () => {
  const f = fixture(1);
  const motion = new CubeStackDrawerMotion(f.element);
  const pending = motion.setVisible(true);
  for (const effect of f.effects) effect.fail(new Error('Compositor failed'));
  await expect(pending).rejects.toThrow('Cube stack drawer animation failed.');
  motion.dispose();
});

test.each(['button', 'fluent-button'] as const)(
  'cascades the %s Add cubes control last, including empty and long stacks',
  async (tag) => {
    for (const count of [0, 3, 12]) {
      const f = fixture(count, tag);
      const motion = new CubeStackDrawerMotion(f.element);
      const entering = motion.setVisible(true);
      expect(f.addButton.animate).toHaveBeenCalledTimes(1);
      const effect = f.effects.at(-1);
      expect(effect?.options.duration).toBe(fluentMotion.duration);
      expect(effect?.frames.at(-2)?.offset).toBe(Math.min(0.25, count * 0.055));
      expect(effect?.frames.at(-2)?.translate).toBe('0 32px');
      expect(effect?.frames.at(-2)?.easing).toBe(fluentMotion.easing);
      expect(effect?.frames.at(-1)?.translate).toBe('0 0');
      for (const animation of f.effects) animation.currentTime = 73;
      const leaving = motion.setVisible(false);
      expect(f.effects).toHaveLength(count + 2);
      expect(effect?.currentTime).toBe(73);
      expect(effect?.updatePlaybackRate).toHaveBeenLastCalledWith(-1);
      f.finish();
      expect(await entering).toBe(false);
      expect(await leaving).toBe(true);
      expect(effect?.cancel).toHaveBeenCalledTimes(1);
      motion.dispose();
    }
  },
);

/** Model only the external animation boundary; real DOM rows retain independent reorder geometry. */
function fixture(count: number, addTag: 'button' | 'fluent-button' = 'button') {
  const element = document.createElement('aside');
  element.inert = false;
  const list = document.createElement('ul');
  list.className = 'sc-cube-stack__rows';
  const rows = Array.from({ length: count }, () => {
    const row = document.createElement('li');
    row.className = 'sc-cube-stack-row';
    row.style.transform = 'translateX(12px)';
    list.append(row);
    return row;
  });
  const addButton = document.createElement(addTag);
  addButton.className = 'sc-cube-stack__add';
  element.append(list, addButton);
  document.body.append(element);
  const effects: Array<{
    frames: Keyframe[];
    options: KeyframeAnimationOptions;
    currentTime: number;
    cancel: ReturnType<typeof jest.fn>;
    updatePlaybackRate: ReturnType<typeof jest.fn>;
    finish(): void;
    fail(error: Error): void;
  }> = [];
  const animate = (frames: Keyframe[], options: KeyframeAnimationOptions): Animation => {
    const completion: { finish?: () => void; fail?: (error: Error) => void } = {};
    const finished = new Promise<void>((resolve, reject) => {
      completion.finish = resolve;
      completion.fail = reject;
    });
    const effect = {
      frames,
      options,
      currentTime: 0,
      finished,
      pause: jest.fn(),
      play: jest.fn(),
      updatePlaybackRate: jest.fn(),
      cancel: jest.fn(() => completion.fail?.(new Error('Canceled'))),
      finish: () => completion.finish?.(),
      fail: (error: Error) => completion.fail?.(error),
    };
    effects.push(effect);
    return effect as unknown as Animation;
  };
  for (const target of [element, ...rows, addButton])
    Object.defineProperty(target, 'animate', { configurable: true, value: jest.fn(animate) });
  return {
    element,
    rows,
    addButton,
    effects,
    finish: () => effects.forEach((effect) => effect.finish()),
  };
}
