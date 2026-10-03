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
/** Prove native advanced disclosure uses shared clipping motion and retains callback ownership. */
import { afterEach, beforeEach, expect, jest, test } from '@jest/globals';
import { NativeAdvancedDisclosure } from '../../../frontend/comfyui/ui/surface/appearance/NativeAdvancedDisclosure.js';
import { NATIVE_NODE_CARD_DISPOSE_EVENT } from '../../../frontend/comfyui/ui/surface/NativeNodeCardRenderer.js';
import type { FluentCardBoundary } from '../../../frontend/comfyui/ui/surface/appearance/NativeFluentCardAdapter.js';

beforeEach(() => {
  jest.useFakeTimers({ doNotFake: ['queueMicrotask'] });
});
afterEach(() => {
  document.body.replaceChildren();
  jest.useRealTimers();
});

/** Model native field visibility and animation completion without substituting widget state. */
function fixture() {
  const root = document.createElement('div');
  root.className = 'lg-node';
  root.dataset.scFluentNode = '';
  const fields = document.createElement('div');
  const advanced = document.createElement('div');
  advanced.dataset.scFluentAdvancedRow = '';
  const button = document.createElement('button');
  button.dataset.testid = 'advanced-inputs-button';
  root.append(fields, button);
  document.body.append(root);
  let expanded = false;
  const callback = jest.fn(() => {
    expanded = !expanded;
    if (expanded) fields.append(advanced);
    else advanced.remove();
  });
  button.addEventListener('click', callback);
  Object.defineProperty(fields, 'offsetHeight', {
    get: () =>
      80 +
      (advanced.parentElement && advanced.style.display !== 'none'
        ? advanced.dataset.styled === 'true'
          ? 120
          : 60
        : 0),
  });
  const effects: {
    frames: Keyframe[];
    options: KeyframeAnimationOptions;
    finish(): void;
    cancel: ReturnType<typeof jest.fn>;
  }[] = [];
  Object.defineProperty(fields, 'animate', {
    value: (frames: Keyframe[], options: KeyframeAnimationOptions) => {
      let finish = (): void => undefined;
      const finished = new Promise<void>((resolve) => {
        finish = resolve;
      });
      const animation = { frames, options, finished, finish: () => finish(), cancel: jest.fn() };
      effects.push(animation);
      return animation;
    },
  });
  const card: FluentCardBoundary = {
    root,
    body: null,
    header: null,
    collapsed: false,
    canCollapse: true,
    fields: [],
    numericFields: [],
    labelElements: [],
    advanced: { fields, button, expanded },
  };
  const adapter = new NativeAdvancedDisclosure();
  adapter.reconcile(card);
  return { root, fields, advanced, button, callback, effects, adapter };
}

test('opens after native refresh and retains advanced controls until the accordion finishes closing', async () => {
  const { fields, advanced, button, callback, effects, adapter } = fixture();
  button.click();
  expect(fields.style.height).toBe('80px');
  await jest.runAllTimersAsync();
  expect(callback).toHaveBeenCalledTimes(1);
  expect(effects[0]?.frames).toEqual([{ height: '80px' }, { height: '140px' }]);
  expect(effects[0]?.options).toMatchObject({ duration: 200, easing: 'cubic-bezier(0,0,0,1)' });
  effects[0]?.finish();
  await Promise.resolve();
  button.click();
  expect(callback).toHaveBeenCalledTimes(1);
  expect(advanced.parentElement).toBe(fields);
  expect(advanced.style.display).toBe('');
  expect(effects[1]?.frames).toEqual([{ height: '140px' }, { height: '80px' }]);
  effects[1]?.finish();
  await Promise.resolve();
  expect(fields.hasAttribute('data-sc-disclosing')).toBe(true);
  await jest.runAllTimersAsync();
  expect(callback).toHaveBeenCalledTimes(2);
  expect(advanced.parentElement).toBeNull();
  expect(fields.hasAttribute('data-sc-disclosing')).toBe(false);
  expect(fields.style.height).toBe('');
  adapter.clear();
});

test('reverses without committing a stale close and retains the latest choice on theme removal', async () => {
  const { fields, button, callback, effects, adapter } = fixture();
  button.click();
  await jest.runAllTimersAsync();
  effects[0]?.finish();
  await Promise.resolve();
  button.click();
  button.click();
  await jest.runAllTimersAsync();
  effects[1]?.finish();
  await Promise.resolve();
  expect(callback).toHaveBeenCalledTimes(1);
  adapter.clear();
  expect(fields.hasAttribute('data-sc-disclosing')).toBe(false);
});

test('theme-free native disclosure skips motion and teardown releases the listener', async () => {
  const { root, button, callback, effects, adapter } = fixture();
  root.removeAttribute('data-sc-fluent-node');
  button.click();
  await Promise.resolve();
  expect(callback).toHaveBeenCalledTimes(1);
  expect(effects).toHaveLength(0);
  adapter.clear();
  button.click();
  expect(callback).toHaveBeenCalledTimes(2);
});

test('measures expansion after deferred native mounting and appearance annotation', async () => {
  const { fields, advanced, button, effects, adapter } = fixture();
  let styled = false;
  Object.defineProperty(advanced, 'hidden', { configurable: true, get: () => !styled });
  button.addEventListener('click', () => {
    advanced.remove();
    window.setTimeout(() => {
      fields.append(advanced);
      window.requestAnimationFrame(() => {
        styled = true;
        advanced.dataset.styled = 'true';
      });
    }, 0);
  });
  button.click();
  await jest.runAllTimersAsync();
  expect(styled).toBe(true);
  expect(effects[0]?.frames[0]).toEqual({ height: '80px' });
  expect(effects[0]?.frames[1]).toEqual({ height: '200px' });
  adapter.clear();
});

test('commits a pending close before the native renderer removes its callbacks', async () => {
  const { root, fields, button, callback, effects, adapter } = fixture();
  button.click();
  await jest.runAllTimersAsync();
  effects[0]?.finish();
  await Promise.resolve();
  button.click();
  expect(callback).toHaveBeenCalledTimes(1);
  root.dispatchEvent(new Event(NATIVE_NODE_CARD_DISPOSE_EVENT));
  expect(callback).toHaveBeenCalledTimes(2);
  expect(fields.style.height).toBe('');
  expect(fields.hasAttribute('data-sc-disclosing')).toBe(false);
  adapter.clear();
});
