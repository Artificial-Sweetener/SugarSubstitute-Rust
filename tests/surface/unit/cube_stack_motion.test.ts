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
/** Prove visible-cube selection and interruptible rail inset motion against scoped geometry. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { CubeStackScrollSelection } from '../../../frontend/comfyui/ui/surface/stack/CubeStackScrollSelection.js';
import { MasonryStackInsetMotion } from '../../../frontend/comfyui/ui/surface/masonry/MasonryStackInsetMotion.js';

afterEach(() => {
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

test('selects the first whole visible cube and coalesces only identified surface events', () => {
  const element = document.createElement('section');
  document.body.append(element);
  for (const [id, y] of [
    ['first', 0],
    ['second', 400],
    ['third', 800],
  ] as const) {
    const header = document.createElement('header');
    header.dataset.masonrySection = `cube:${id}`;
    header.dataset.masonryY = String(y);
    header.dataset.masonryHeight = '376';
    element.append(header);
  }
  Object.defineProperties(element, { scrollHeight: { value: 1400 }, clientHeight: { value: 500 } });
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.push(callback);
    return frames.length;
  });
  const update = jest.fn<(id: string | null) => void>();
  const tracker = new CubeStackScrollSelection(element, update);
  tracker.refresh(['first', 'second', 'third']);
  frames.shift()!(0);
  expect(update).toHaveBeenLastCalledWith('first');
  element.scrollTop = 200;
  element.dispatchEvent(new Event('scroll'));
  element.dispatchEvent(new Event('scroll'));
  expect(frames).toHaveLength(1);
  frames.shift()!(0);
  expect(update).toHaveBeenLastCalledWith('first');
  element.scrollTop = 376;
  element.dispatchEvent(new Event('scroll'));
  frames.shift()!(0);
  expect(update).toHaveBeenLastCalledWith('first');
  element.scrollTop = 377;
  element.dispatchEvent(new Event('scroll'));
  frames.shift()!(0);
  expect(update).toHaveBeenLastCalledWith('second');
  element.scrollTop = 600;
  element.dispatchEvent(new Event('scroll'));
  frames.shift()!(0);
  expect(update).toHaveBeenLastCalledWith('second');
  element.scrollTop = 900;
  element.dispatchEvent(new Event('sc-masonry-layout'));
  frames.shift()!(0);
  expect(update).toHaveBeenLastCalledWith('third');
  element.scrollTop = 1200;
  tracker.refresh(['first', 'second', 'third']);
  frames.shift()!(0);
  expect(update).toHaveBeenLastCalledWith('third');
  document.body.append(document.createElement('dialog'));
  expect(frames).toHaveLength(0);
  tracker.dispose();
  element.dispatchEvent(new Event('scroll'));
  expect(frames).toHaveLength(0);
});

test('includes the content inset, ignores filtered or invalid bounds and avoids redundant updates', () => {
  const element = document.createElement('section');
  const content = document.createElement('div');
  element.append(content);
  Object.defineProperty(element, 'clientHeight', { value: 200 });
  Object.defineProperty(content, 'offsetTop', { value: 52 });
  for (const [id, y, height] of [
    ['first', '0', '300'],
    ['second', '324', '300'],
    ['invalid', 'broken', '300'],
  ] as const) {
    const header = document.createElement('header');
    header.dataset.masonrySection = `cube:${id}`;
    header.dataset.masonryY = y;
    header.dataset.masonryHeight = height;
    content.append(header);
  }
  const update = jest.fn<(id: string | null) => void>();
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.push(callback);
    return frames.length;
  });
  const tracker = new CubeStackScrollSelection(element, update);
  const ids = ['first', 'second', 'invalid'];
  tracker.refresh(ids);
  frames.shift()!(0);
  element.scrollTop = 351;
  tracker.refresh(ids);
  frames.shift()!(0);
  expect(update).toHaveBeenCalledTimes(1);
  expect(update).toHaveBeenLastCalledWith('first');
  element.scrollTop = 353;
  tracker.refresh(ids);
  frames.shift()!(0);
  expect(update).toHaveBeenLastCalledWith('second');
  content.children[1]?.setAttribute('data-sc-search-card-hidden', '');
  element.scrollTop = 0;
  tracker.refresh(ids);
  frames.shift()!(0);
  expect(update).toHaveBeenLastCalledWith('first');
  tracker.dispose();
});

test('coalesces refresh geometry with scoped layout events after presentation writes settle', () => {
  const element = document.createElement('section');
  const header = document.createElement('header');
  header.dataset.masonrySection = 'cube:visible';
  header.dataset.masonryY = '0';
  header.dataset.masonryHeight = '300';
  element.append(header);
  const height = jest.fn(() => 200);
  Object.defineProperty(element, 'clientHeight', { get: height });
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.push(callback);
    return frames.length;
  });
  const update = jest.fn<(id: string | null) => void>();
  const tracker = new CubeStackScrollSelection(element, update);
  try {
    for (let index = 0; index < 10; index++) tracker.refresh(['visible']);
    element.dispatchEvent(new Event('sc-masonry-layout'));
    expect(height).not.toHaveBeenCalled();
    expect(frames).toHaveLength(1);
    frames.shift()!(0);
    expect(height).toHaveBeenCalledTimes(1);
    expect(update).toHaveBeenLastCalledWith('visible');
  } finally {
    tracker.dispose();
  }
});

test('commits inset layout once and animates from the current painted position across reversals', () => {
  const element = document.createElement('section');
  const content = document.createElement('div');
  element.append(content);
  const motion = new MasonryStackInsetMotion();
  let painted: number | null = null;
  jest.spyOn(content, 'getBoundingClientRect').mockImplementation(() => ({
    left:
      painted ??
      Number.parseFloat(element.style.getPropertyValue('--sc-masonry-stack-width') || '0'),
    top: 0,
    right: 0,
    bottom: 0,
    x: 0,
    y: 0,
    width: 100,
    height: 100,
    toJSON: () => ({}),
  }));
  const effects: { frames: Keyframe[]; options: KeyframeAnimationOptions; cancel: () => void }[] =
    [];
  Object.defineProperty(content, 'animate', {
    value: (frames: Keyframe[], options: KeyframeAnimationOptions) => {
      const effect = {
        frames,
        options,
        cancel: jest.fn(),
        finished: new Promise<Animation>(() => undefined),
      };
      effects.push(effect);
      return effect;
    },
  });
  const layout = jest.fn(() => {
    painted = null;
  });
  motion.set(element, content, 82, layout);
  expect(effects).toHaveLength(0);
  expect(content.getBoundingClientRect).not.toHaveBeenCalled();
  motion.set(element, content, 248, layout);
  expect(effects[0]?.frames).toEqual([
    { transform: 'translateX(-166px)' },
    { transform: 'translateX(0px)' },
  ]);
  expect(effects[0]?.options.duration).toBeGreaterThan(0);
  motion.set(element, content, 248, layout);
  expect(layout).toHaveBeenCalledTimes(2);
  painted = 160;
  motion.set(element, content, 82, layout);
  expect(effects[0]?.cancel).toHaveBeenCalledTimes(1);
  expect(effects[1]?.frames[0]).toEqual({ transform: 'translateX(78px)' });
  motion.dispose();
  expect(effects[1]?.cancel).toHaveBeenCalledTimes(1);
});

test('honors reduced motion while preserving the final reserved width', () => {
  const original = Object.getOwnPropertyDescriptor(window, 'matchMedia');
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    value: () => ({ matches: true }),
  });
  try {
    const element = document.createElement('section');
    const content = document.createElement('div');
    const animate = jest.fn();
    Object.defineProperty(content, 'animate', { value: animate });
    jest.spyOn(content, 'getBoundingClientRect').mockImplementation(() => ({
      left: Number.parseFloat(element.style.getPropertyValue('--sc-masonry-stack-width') || '0'),
      top: 0,
      right: 0,
      bottom: 0,
      x: 0,
      y: 0,
      width: 1,
      height: 1,
      toJSON: () => ({}),
    }));
    const motion = new MasonryStackInsetMotion();
    motion.set(element, content, 82, () => undefined);
    motion.set(element, content, 248, () => undefined);
    expect(animate).not.toHaveBeenCalled();
    expect(element.style.getPropertyValue('--sc-masonry-stack-width')).toBe('248px');
    motion.dispose();
  } finally {
    if (original) Object.defineProperty(window, 'matchMedia', original);
    else Reflect.deleteProperty(window, 'matchMedia');
  }
});

test('waits for the latest inset destination across interrupted motion before graph assembly', async () => {
  const element = document.createElement('section');
  const content = document.createElement('div');
  element.append(content);
  jest.spyOn(content, 'getBoundingClientRect').mockImplementation(() => ({
    left: Number.parseFloat(element.style.getPropertyValue('--sc-masonry-stack-width') || '0'),
    top: 0,
    right: 0,
    bottom: 0,
    x: 0,
    y: 0,
    width: 100,
    height: 100,
    toJSON: () => ({}),
  }));
  const finishes: Array<() => void> = [];
  Object.defineProperty(content, 'animate', {
    value: () => {
      let finish = (): void => undefined;
      const finished = new Promise<void>((resolve) => {
        finish = resolve;
      });
      finishes.push(finish);
      return { finished, cancel: finish };
    },
  });
  const motion = new MasonryStackInsetMotion();
  motion.set(element, content, 82, () => undefined);
  motion.set(element, content, 248, () => undefined);
  let settled = false;
  const pending = motion.settled().then(() => {
    settled = true;
  });
  motion.set(element, content, 82, () => undefined);
  for (let turn = 0; turn < 4; turn += 1) await Promise.resolve();
  expect(settled).toBe(false);
  finishes[1]?.();
  await pending;
  expect(settled).toBe(true);
  await motion.settled();
  motion.dispose();
});
