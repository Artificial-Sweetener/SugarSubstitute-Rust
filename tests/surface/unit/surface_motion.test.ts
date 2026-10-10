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
/** Prove interruption, stable placement, retirement, and reduced-motion behavior. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { SurfaceMotion } from '../../../frontend/comfyui/ui/surface/motion/SurfaceMotion.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** Bind deterministic completion to the browser animation boundary. */
function fixture() {
  const element = document.createElement('article');
  document.body.append(element);
  const effects: { frames: Keyframe[]; cancel: ReturnType<typeof jest.fn>; finish(): void }[] = [];
  Object.defineProperty(element, 'animate', {
    value: (frames: Keyframe[]) => {
      let finish = (): void => undefined;
      const finished = new Promise<void>((resolve) => {
        finish = resolve;
      });
      const effect = { frames, finished, cancel: jest.fn(), finish: () => finish() };
      effects.push(effect);
      return effect;
    },
  });
  return { element, effects, motion: new SurfaceMotion() };
}

test('preserves committed geometry and coalesces unchanged placement while interrupting changes', () => {
  const { element, effects, motion } = fixture();
  motion.place([{ element, rectangle: { x: 0, y: 0, width: 400, height: 100 } }]);
  expect(effects).toHaveLength(0);
  motion.settle();
  motion.place([{ element, rectangle: { x: 400, y: 120, width: 400, height: 200 } }]);
  expect(element.style.transform).toBe('translate(400px, 120px)');
  expect(effects[0]?.frames[0]?.transform).toBe('translate(0px, 0px)');
  expect(element.hasAttribute('data-sc-surface-moving')).toBe(true);
  motion.place([{ element, rectangle: { x: 400, y: 120, width: 400, height: 200 } }]);
  expect(effects).toHaveLength(1);
  motion.place([{ element, rectangle: { x: 0, y: 250, width: 400, height: 200 } }]);
  expect(effects[0]?.cancel).toHaveBeenCalledTimes(1);
  motion.dispose();
  expect(effects[1]?.cancel).toHaveBeenCalledTimes(1);
  expect(element.hasAttribute('data-sc-surface-moving')).toBe(false);
});

test('reads all interrupted positions before committing any neighbor destination', () => {
  const first = fixture();
  const second = fixture();
  const motion = first.motion;
  const rectangles = (offset: number) => [
    { element: first.element, rectangle: { x: 0, y: offset, width: 400, height: 100 } },
    { element: second.element, rectangle: { x: 0, y: offset + 120, width: 400, height: 100 } },
  ];
  motion.place(rectangles(0));
  motion.settle();
  motion.place(rectangles(100));
  const readStyle = window.getComputedStyle.bind(window);
  const reads: string[][] = [];
  jest.spyOn(window, 'getComputedStyle').mockImplementation((element) => {
    reads.push([first.element.style.transform, second.element.style.transform]);
    return readStyle(element);
  });
  motion.place(rectangles(200));
  expect(reads).toEqual([
    ['translate(0px, 100px)', 'translate(0px, 220px)'],
    ['translate(0px, 100px)', 'translate(0px, 220px)'],
  ]);
  expect(first.effects[0]?.cancel).toHaveBeenCalledTimes(1);
  expect(second.effects[0]?.cancel).toHaveBeenCalledTimes(1);
  expect(first.element.style.transform).toBe('translate(0px, 200px)');
  expect(second.element.style.transform).toBe('translate(0px, 320px)');
  motion.dispose();
  second.motion.dispose();
});

test('restores existing pointer interaction after moving neighbors can no longer cover the edited card', async () => {
  const { element, effects, motion } = fixture();
  element.style.setProperty('pointer-events', 'auto', 'important');
  motion.place([{ element, rectangle: { x: 0, y: 0, width: 400, height: 100 } }]);
  motion.settle();
  motion.place([{ element, rectangle: { x: 400, y: 120, width: 400, height: 100 } }]);
  expect(element.hasAttribute('data-sc-surface-moving')).toBe(true);
  effects[0]?.finish();
  await Promise.resolve();
  await Promise.resolve();
  expect(element.style.getPropertyValue('pointer-events')).toBe('auto');
  expect(element.style.getPropertyPriority('pointer-events')).toBe('important');
  expect(element.hasAttribute('data-sc-surface-moving')).toBe(false);
  motion.dispose();
});

test('lets disclosure own continuous height and neighboring movement without scaling content', () => {
  const { element, effects, motion } = fixture();
  motion.place([{ element, rectangle: { x: 0, y: 0, width: 400, height: 200 } }]);
  motion.settle();
  motion.place([{ element, rectangle: { x: 0, y: 0, width: 400, height: 100 } }]);
  expect(effects).toHaveLength(0);
  motion.place([{ element, rectangle: { x: 0, y: 20, width: 400, height: 100 } }], false);
  expect(effects).toHaveLength(0);
  expect(element.style.transform).toBe('translate(0px, 20px)');
  motion.dispose();
});

test('finishes placement effects without losing destinations needed by the next layout', () => {
  const { element, effects, motion } = fixture();
  motion.place([{ element, rectangle: { x: 0, y: 0, width: 400, height: 100 } }]);
  motion.settle();
  motion.place([{ element, rectangle: { x: 400, y: 120, width: 400, height: 100 } }]);
  motion.finishAnimations();
  expect(effects[0]?.cancel).toHaveBeenCalledTimes(1);
  expect(element.style.transform).toBe('translate(400px, 120px)');
  motion.place([{ element, rectangle: { x: 400, y: 120, width: 400, height: 100 } }]);
  expect(effects).toHaveLength(1);
  motion.place([{ element, rectangle: { x: 0, y: 0, width: 400, height: 100 } }]);
  expect(effects[1]?.frames[0]?.transform).toBe('translate(400px, 120px)');
  motion.dispose();
});

test('animates scroll extent once and reverses from its current displayed height', () => {
  const { element, effects, motion } = fixture();
  motion.resizeHeight(element, 800);
  expect(effects).toHaveLength(0);
  motion.settle();
  motion.resizeHeight(element, 600);
  expect(effects[0]?.frames).toEqual([{ height: '800px' }, { height: '600px' }]);
  motion.resizeHeight(element, 600.25);
  expect(effects).toHaveLength(1);
  element.style.height = '720px';
  motion.resizeHeight(element, 900);
  expect(effects[0]?.cancel).toHaveBeenCalledTimes(1);
  expect(effects[1]?.frames).toEqual([{ height: '720px' }, { height: '900px' }]);
  motion.dispose();
});

test('commits initial scroll extent without forcing a computed-layout read', () => {
  const { element, motion } = fixture();
  const computedStyle = jest.spyOn(window, 'getComputedStyle');
  motion.resizeHeight(element, 800);
  expect(computedStyle).not.toHaveBeenCalled();
  expect(element.style.height).toBe('800px');
  motion.dispose();
});

test('cancels stale retirement without disposing a revived native card', async () => {
  const { element, effects, motion } = fixture();
  const release = jest.fn();
  motion.retire(element, release);
  motion.revive(element);
  effects[0]?.finish();
  await Promise.resolve();
  await Promise.resolve();
  expect(release).not.toHaveBeenCalled();
  effects[1]?.finish();
  await Promise.resolve();
  await Promise.resolve();
  motion.retire(element, release);
  effects[2]?.finish();
  await Promise.resolve();
  await Promise.resolve();
  expect(release).toHaveBeenCalledTimes(1);
  motion.dispose();
});

test('uses immediate destination and cleanup when the browser requests reduced motion', () => {
  const { element, effects, motion } = fixture();
  const original = Object.getOwnPropertyDescriptor(window, 'matchMedia');
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    value: () => ({ matches: true }),
  });
  try {
    motion.settle();
    motion.place([{ element, rectangle: { x: 10, y: 20, width: 400, height: 100 } }]);
    const release = jest.fn();
    motion.retire(element, release);
    expect(element.style.transform).toBe('translate(10px, 20px)');
    expect(effects).toHaveLength(0);
    expect(release).toHaveBeenCalledTimes(1);
    motion.dispose();
  } finally {
    if (original) Object.defineProperty(window, 'matchMedia', original);
    else Reflect.deleteProperty(window, 'matchMedia');
  }
});
