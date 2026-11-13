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
/** Prove native seed popovers retain Comfy mode ownership and gain one correctly scoped reuse action. */
import { jest } from '@jest/globals';
import { NativeSeedMenuHost } from '../../../frontend/comfyui/ui/surface/appearance/NativeSeedMenuHost.js';
import {
  registerNativeWidgetMount,
  releaseNativeWidgetMount,
} from '../../../frontend/comfyui/ui/surface/NativeWidgetMountBoundary.js';
import type { NativeSeedControl } from '../../../frontend/comfyui/ui/surface/appearance/ComfyNativeSeedControl.js';

/** Flush the native lazy popup mount and its one scoped augmentation. */
async function settle(): Promise<void> {
  for (let n = 0; n < 4; n++) await Promise.resolve();
}

afterEach(() => document.body.replaceChildren());

/** Model Comfy's local mode ref and original popup radios independently of the authoritative seed value. */
function fixture() {
  const root = document.createElement('div');
  root.className = 'sugarcubes-native-widget';
  const button = document.createElement('button');
  button.dataset.testid = 'value-control';
  button.setAttribute('aria-controls', 'seed-popup');
  root.append(button);
  document.body.append(root);
  const popup = document.createElement('div');
  popup.id = 'seed-popup';
  popup.setAttribute('role', 'dialog');
  let mode: 'fixed' | 'randomize' = 'randomize';
  let nativeMode = 'randomize';
  let value = 999;
  let last: number | null = null;
  const original = jest.fn(() => {
    if (popup.isConnected) {
      popup.remove();
      button.setAttribute('aria-expanded', 'false');
      return;
    }
    popup.replaceChildren();
    for (const item of ['fixed', 'randomize'] as const) {
      const label = document.createElement('label');
      label.htmlFor = item;
      const radio = document.createElement('input');
      radio.type = 'radio';
      radio.value = item;
      radio.addEventListener('click', () => {
        nativeMode = item;
        mode = item;
      });
      label.append(radio);
      popup.append(label);
    }
    button.setAttribute('aria-expanded', 'true');
    document.body.append(popup);
  });
  button.addEventListener('click', original);
  const reuse = jest.fn(() => {
    if (last !== null) {
      value = last;
      mode = 'fixed';
    }
  });
  const seed: NativeSeedControl = {
    modes: ['fixed', 'randomize'],
    readMode: () => mode,
    writeMode: (next) => {
      if (next === 'fixed' || next === 'randomize') mode = next;
    },
    reuse: { readLast: () => last, reuse },
  };
  registerNativeWidgetMount(root, { vnode: {}, seed });
  const error = jest.fn();
  const host = new NativeSeedMenuHost(null, document, () => null, error);
  host.setup();
  return {
    root,
    button,
    popup,
    original,
    reuse,
    host,
    error,
    setLast: (next: number) => {
      last = next;
    },
    read: () => ({ value, mode, nativeMode }),
    dispose: () => {
      host.dispose();
      releaseNativeWidgetMount(root);
    },
  };
}

test('keeps the original popup and mode radios while adding one disabled action before any accepted seed', async () => {
  const f = fixture();
  f.button.click();
  await settle();
  expect(f.original).toHaveBeenCalledTimes(1);
  expect(f.popup.querySelectorAll('input[type="radio"]')).toHaveLength(2);
  expect(
    f.popup.querySelector<HTMLButtonElement>('[data-sc-seed-option="reuse-last"]')?.disabled,
  ).toBe(true);
  const unrelated = document.createElement('div');
  unrelated.setAttribute('role', 'dialog');
  unrelated.id = 'unrelated';
  document.body.append(unrelated);
  await settle();
  expect(unrelated.querySelector('[data-sc-seed-option]')).toBeNull();
  expect(f.popup.querySelectorAll('[data-sc-seed-option="reuse-last"]')).toHaveLength(1);
  f.dispose();
});

test('restores through the shared owner and synchronizes Comfy local mode using its original fixed radio', async () => {
  const f = fixture();
  f.setLast(42);
  f.button.click();
  await settle();
  const row = f.popup.querySelector<HTMLButtonElement>('[data-sc-seed-option="reuse-last"]');
  expect(row?.querySelector('.icon-\\[lucide--rotate-ccw\\]')).not.toBeNull();
  row?.click();
  await settle();
  expect(f.reuse).toHaveBeenCalledTimes(1);
  expect(f.read()).toEqual({ value: 42, mode: 'fixed', nativeMode: 'fixed' });
  expect(f.popup.isConnected).toBe(false);
  expect(f.error).not.toHaveBeenCalled();
  f.dispose();
});

test('closes captured popovers before graph replacement and never augments Fluent split boxes', async () => {
  const f = fixture();
  f.setLast(42);
  f.button.click();
  await settle();
  f.host.close();
  await settle();
  expect(f.popup.isConnected).toBe(false);
  f.root.dataset.scSeedBox = '';
  f.button.click();
  await settle();
  expect(f.popup.querySelector('[data-sc-seed-option="reuse-last"]')).toBeNull();
  expect(f.original).toHaveBeenCalled();
  f.dispose();
});

test('does not alter unregistered native controls or popup inventory', async () => {
  const f = fixture();
  releaseNativeWidgetMount(f.root);
  f.button.click();
  await settle();
  expect(f.original).toHaveBeenCalledTimes(1);
  expect(f.popup.querySelector('[data-sc-seed-option]')).toBeNull();
  f.dispose();
});

test('dismisses the added action with Escape and restores native trigger focus', async () => {
  const f = fixture();
  f.setLast(42);
  f.button.click();
  await settle();
  const row = f.popup.querySelector<HTMLButtonElement>('[data-sc-seed-option="reuse-last"]');
  row?.focus();
  row?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
  await settle();
  expect(f.popup.isConnected).toBe(false);
  expect(document.activeElement).toBe(f.button);
  f.dispose();
});

test.each([true, false])(
  'resolves Nodes 2.0 seed popovers with development ancestry=%s',
  async (ancestry) => {
    const f = fixture();
    f.host.dispose();
    f.root.className = 'lg-node';
    const mode = {
      name: 'control_after_generate',
      type: 'combo',
      value: 'randomize',
      options: { values: ['fixed', 'randomize'] },
    };
    const node = { id: 42, widgets: [{ name: 'noise_seed', value: 999, linkedWidgets: [mode] }] };
    const nativeWidget = {
      name: 'noise_seed',
      nodeLocatorId: 'root:42',
      options: { precision: 0 },
    };
    const vnode = {
      type: { __name: 'LGraphNode' },
      component: { subTree: { el: f.root } },
      children: [
        {
          type: { __name: 'WidgetInputNumber' },
          component: { props: { widget: nativeWidget }, subTree: { el: f.root } },
        },
      ],
    };
    if (ancestry) Object.assign(f.root, { __vueParentComponent: { vnode } });
    else {
      const mount = document.createElement('div');
      mount.className = 'sugarcubes-native-node-card';
      document.body.append(mount);
      mount.append(f.root);
      Object.assign(mount, { _vnode: vnode });
    }
    const resolve = jest.fn((owner: object, name: string) => {
      expect(owner).toBe(node);
      expect(name).toBe('noise_seed');
      return { readLast: () => 42, reuse: f.reuse };
    });
    const host = new NativeSeedMenuHost(
      { graph: { id: 'root', _nodes: [node] } },
      document,
      resolve,
      f.error,
    );
    host.setup();
    f.button.click();
    await settle();
    expect(
      f.popup.querySelector<HTMLButtonElement>('[data-sc-seed-option="reuse-last"]')?.disabled,
    ).toBe(false);
    expect(resolve).toHaveBeenCalledTimes(1);
    expect(f.error).not.toHaveBeenCalled();
    host.dispose();
    f.dispose();
  },
);
