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
/** Prove clipped disclosure, interruption, native-state timing, and reduced-motion cleanup. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { CardDisclosureMotion } from '../../../frontend/comfyui/ui/surface/motion/CardDisclosureMotion.js';
import { DISCLOSURE_LAYOUT_EVENT } from '../../../frontend/comfyui/ui/surface/motion/DisclosureLayout.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** Model intrinsic card sizing and the browser animation completion boundary. */
function fixture() {
  const target = document.createElement('div');
  target.style.height = '200px';
  target.style.overflow = 'visible';
  const root = document.createElement('div');
  root.className = 'lg-node';
  root.dataset.scFluentNode = '';
  root.style.height = '200px';
  const wrapper = document.createElement('div');
  wrapper.dataset.testid = 'node-inner-wrapper';
  wrapper.style.border = '1px solid';
  const header = document.createElement('div');
  header.dataset.testid = 'node-header-1';
  header.style.height = '49px';
  wrapper.append(header);
  root.append(wrapper);
  target.append(root);
  document.body.append(target);
  const effects: { frames: Keyframe[]; cancel: ReturnType<typeof jest.fn>; finish(): void }[] = [];
  Object.defineProperty(target, 'animate', {
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
  return { target, root, effects, motion: new CardDisclosureMotion(target) };
}

test('keeps native content mounted until closing completes and then releases owned clipping', async () => {
  const { target, effects, motion } = fixture();
  const apply = jest.fn();
  const layout = jest.fn();
  target.addEventListener(DISCLOSURE_LAYOUT_EVENT, layout);
  motion.toggle(false, apply);
  expect(apply).not.toHaveBeenCalled();
  expect(effects[0]?.frames).toEqual([{ height: '200px' }, { height: '51px' }]);
  expect(target.style.overflow).toBe('clip');
  expect(target.hasAttribute('data-sc-disclosing')).toBe(true);
  expect(target.dataset.scDisclosureHeight).toBe('51');
  expect(layout).toHaveBeenCalledTimes(1);
  effects[0]?.finish();
  await Promise.resolve();
  expect(apply).toHaveBeenCalledWith(true);
  expect(target.style.overflow).toBe('visible');
  expect(target.hasAttribute('data-sc-disclosing')).toBe(false);
  expect(target.hasAttribute('data-sc-disclosure-height')).toBe(false);
  expect(layout).toHaveBeenCalledTimes(2);
  motion.dispose();
});

test('reverses mid-close and ignores stale completions and disposal', async () => {
  const { effects, motion } = fixture();
  const apply = jest.fn();
  motion.toggle(false, apply);
  motion.toggle(false, apply);
  expect(effects[0]?.cancel).toHaveBeenCalledTimes(1);
  expect(apply).toHaveBeenCalledWith(false);
  effects[0]?.finish();
  await Promise.resolve();
  expect(apply).not.toHaveBeenCalledWith(true);
  motion.dispose();
  effects[1]?.finish();
  await Promise.resolve();
  expect(apply).not.toHaveBeenCalledWith(true);
});

test('skips animation under reduced motion or when the appearance is disabled', () => {
  const { target, root, effects, motion } = fixture();
  const apply = jest.fn();
  root.removeAttribute('data-sc-fluent-node');
  motion.toggle(false, apply);
  expect(apply).toHaveBeenCalledWith(true);
  expect(effects).toHaveLength(0);
  root.dataset.scFluentNode = '';
  const original = Object.getOwnPropertyDescriptor(window, 'matchMedia');
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    value: () => ({ matches: true }),
  });
  try {
    motion.toggle(true, apply);
    expect(apply).toHaveBeenCalledWith(false);
    expect(effects).toHaveLength(0);
    expect(target.hasAttribute('data-sc-disclosing')).toBe(false);
  } finally {
    if (original) Object.defineProperty(window, 'matchMedia', original);
    else Reflect.deleteProperty(window, 'matchMedia');
  }
  motion.dispose();
});

test('retains the latest user choice when a view unmount interrupts closing', () => {
  const { effects, motion } = fixture();
  const apply = jest.fn();
  motion.toggle(false, apply);
  motion.dispose();
  expect(apply).toHaveBeenCalledWith(true);
  expect(effects[0]?.cancel).toHaveBeenCalledTimes(1);
});
