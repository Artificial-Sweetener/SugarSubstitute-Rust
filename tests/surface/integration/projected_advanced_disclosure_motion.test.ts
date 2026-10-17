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
/** Prove Fluent advanced motion survives projected native remounts and retains exact-node state. */
import { afterEach, beforeEach, expect, jest, test } from '@jest/globals';
import { NativeProjectedAdvancedInputs } from '../../../frontend/comfyui/ui/surface/NativeProjectedAdvancedInputs.js';
import { NativeAdvancedDisclosure } from '../../../frontend/comfyui/ui/surface/appearance/NativeAdvancedDisclosure.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import { CardDisclosureMotion } from '../../../frontend/comfyui/ui/surface/motion/CardDisclosureMotion.js';

beforeEach(() => {
  jest.useFakeTimers({ doNotFake: ['queueMicrotask'] });
});
afterEach(() => {
  document.body.replaceChildren();
  jest.useRealTimers();
  jest.restoreAllMocks();
});

/** Replace the native root exactly as Comfy does when the footer's snapshotted state changes. */
function fixture(fluent = true, initiallyShown = false) {
  let disposed = false;
  let appearance = fluent;
  const target = document.createElement('div');
  target.className = 'sugarcubes-native-node-card';
  document.body.append(target);
  const beforeChange = jest.fn(),
    afterChange = jest.fn(),
    nativeToggle = jest.fn();
  const node: ComfyNode = {
    id: 'advanced',
    type: 'VAE Decode',
    showAdvanced: initiallyShown,
    graph: { beforeChange, afterChange },
    widgets: [{ name: 'tile_size', type: 'number', value: 512, options: { advanced: true } }],
  };
  const effects: {
    frames: Keyframe[];
    options: KeyframeAnimationOptions;
    finish(): void;
    cancel: ReturnType<typeof jest.fn>;
  }[] = [];
  const animate = (frames: Keyframe[], options: KeyframeAnimationOptions) => {
    let finish = (): void => undefined;
    const finished = new Promise<void>((resolve) => {
      finish = resolve;
    });
    const effect = { frames, options, finished, finish, cancel: jest.fn() };
    effects.push(effect);
    return effect;
  };
  Object.defineProperty(target, 'animate', { value: animate });
  Object.defineProperty(target, 'offsetHeight', {
    get: () => target.querySelector<HTMLElement>('.lg-node')?.offsetHeight ?? 0,
  });
  const presentation = new NativeAdvancedDisclosure();
  const render = (): void => {
    if (disposed) return;
    const root = document.createElement('div');
    root.className = 'lg-node';
    if (appearance) root.dataset.scStyleNode = '';
    const fields = document.createElement('div');
    fields.dataset.scStyleFields = '';
    const basic = document.createElement('div');
    basic.textContent = 'Use Tiling';
    const advanced = document.createElement('div');
    advanced.dataset.scStyleAdvancedRow = '';
    advanced.textContent = 'Tile Size';
    fields.append(basic);
    if (node.showAdvanced) fields.append(advanced);
    Object.defineProperty(fields, 'offsetHeight', {
      get: () => 40 + (advanced.parentElement && advanced.style.display !== 'none' ? 80 : 0),
    });
    Object.defineProperty(fields, 'animate', { value: animate });
    Object.defineProperty(root, 'offsetHeight', { get: () => fields.offsetHeight + 60 });
    const button = document.createElement('button');
    button.dataset.testid = 'advanced-inputs-button';
    button.textContent = node.showAdvanced ? 'Hide advanced inputs' : 'Show advanced inputs';
    button.addEventListener('click', nativeToggle);
    const footer = document.createElement('div');
    footer.append(button);
    root.append(fields, footer);
    target.replaceChildren(root);
    input.reconcile(footer);
    presentation.reconcile({
      root,
      body: null,
      header: null,
      fields: [],
      numericFields: [],
      seedFields: [],
      labelElements: [],
      collapsed: false,
      canCollapse: true,
      advanced: { fields, button, expanded: node.showAdvanced === true },
    });
  };
  const motion = new CardDisclosureMotion(target);
  const input = new NativeProjectedAdvancedInputs(target, node, render, motion);
  render();
  const click = (): void => target.querySelector<HTMLButtonElement>('button')?.click();
  const dispose = (): void => {
    disposed = true;
    input.dispose();
    presentation.clear();
    motion.dispose();
  };
  const disableFluent = (): void => {
    appearance = false;
    presentation.clear();
  };
  return {
    target,
    node,
    effects,
    beforeChange,
    afterChange,
    nativeToggle,
    presentation,
    click,
    dispose,
    disableFluent,
  };
}

test('animates on the stable mount across opening remounts and removes advanced rows after closing', async () => {
  const setup = fixture();
  const initialRoot = setup.target.firstElementChild;
  setup.click();
  expect(setup.node.showAdvanced).toBe(true);
  expect(setup.target.firstElementChild).not.toBe(initialRoot);
  expect(setup.target.style.height).toBe('100px');
  expect(setup.target.dataset.scDisclosureLayout).toBe('fill');
  await jest.runAllTimersAsync();
  expect(setup.effects[0]?.frames).toEqual([{ height: '100px' }, { height: '180px' }]);
  expect(setup.effects[0]?.options).toMatchObject({
    duration: 200,
    easing: 'cubic-bezier(0,0,0,1)',
  });
  setup.effects[0]?.finish();
  await Promise.resolve();
  expect(setup.target.hasAttribute('data-sc-disclosure-layout')).toBe(false);
  setup.click();
  expect(setup.node.showAdvanced).toBe(true);
  expect(setup.target.querySelector('[data-sc-style-advanced-row]')).not.toBeNull();
  expect(setup.effects[1]?.frames).toEqual([{ height: '180px' }, { height: '100px' }]);
  setup.effects[1]?.finish();
  await jest.runAllTimersAsync();
  expect(setup.node.showAdvanced).toBe(false);
  expect(setup.target.querySelector('[data-sc-style-advanced-row]')).toBeNull();
  expect(setup.target.style.height).toBe('');
  expect(setup.target.hasAttribute('data-sc-disclosing')).toBe(false);
  expect(setup.target.hasAttribute('data-sc-disclosure-layout')).toBe(false);
  expect(setup.beforeChange).toHaveBeenCalledTimes(2);
  expect(setup.afterChange).toHaveBeenCalledTimes(2);
  expect(setup.nativeToggle).not.toHaveBeenCalled();
  setup.dispose();
});

test('measures a card loaded with advanced inputs already shown and reverses a pending close', async () => {
  const setup = fixture(true, true);
  setup.click();
  expect(setup.effects[0]?.frames).toEqual([{ height: '180px' }, { height: '100px' }]);
  setup.click();
  await jest.runAllTimersAsync();
  setup.effects[0]?.finish();
  await Promise.resolve();
  expect(setup.node.showAdvanced).toBe(true);
  expect(setup.beforeChange).not.toHaveBeenCalled();
  expect(setup.effects[0]?.cancel).toHaveBeenCalled();
  setup.dispose();
});

test('commits the latest request and releases motion when Fluent presentation ends', async () => {
  const setup = fixture(true, true);
  setup.click();
  setup.disableFluent();
  expect(setup.node.showAdvanced).toBe(false);
  expect(setup.target.style.height).toBe('');
  expect(setup.target.hasAttribute('data-sc-disclosing')).toBe(false);
  setup.click();
  expect(setup.node.showAdvanced).toBe(true);
  expect(setup.effects).toHaveLength(1);
  setup.dispose();
});

test('releases presentation at projected mount disposal while native state remains available', () => {
  const setup = fixture(true, true);
  setup.click();
  setup.dispose();
  expect(setup.node.showAdvanced).toBe(false);
  expect(setup.target.hasAttribute('data-sc-disclosing')).toBe(false);
  expect(setup.target.style.height).toBe('');
});

test('retains immediate state transitions in the normal theme', async () => {
  const setup = fixture(false);
  setup.click();
  expect(setup.node.showAdvanced).toBe(true);
  setup.click();
  expect(setup.node.showAdvanced).toBe(false);
  await jest.runAllTimersAsync();
  expect(setup.effects).toHaveLength(0);
  setup.dispose();
});
