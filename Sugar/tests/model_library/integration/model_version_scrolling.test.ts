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
/** Prove wheel gestures move the version strip without selecting cards or reaching the graph. */
import { createApp, h, nextTick } from 'vue';
import { jest } from '@jest/globals';
import { ModelVersionFamily } from '../../../frontend/comfyui/ui/model_library/vue/ModelVersionFamily.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import type { ModelRelease } from '../../../frontend/comfyui/ui/model_library/ModelReleases.js';

/** Render the actual version strip with measured overflow supplied at the browser boundary. */
async function fixture() {
  const item = modelPickerChoices([], 'checkpoints', ['current.safetensors'])[0];
  if (!item) throw new Error('Missing model fixture.');
  const version: ModelRelease = {
    versionId: '1',
    name: 'v1',
    baseModel: 'SDXL',
    publishedAt: '',
    pageUrl: '',
    previewUrl: null,
    onDisk: false,
    inUse: false,
    files: [],
  };
  const root = document.createElement('div');
  document.body.append(root);
  const select = jest.fn();
  const outerWheel = jest.fn();
  root.addEventListener('wheel', outerWheel);
  const app = createApp({
    render: () =>
      h(ModelVersionFamily, {
        item,
        versions: [version],
        selected: '',
        busy: false,
        status: '',
        onSelect: select,
      }),
  });
  app.mount(root);
  await nextTick();
  const strip = root.querySelector<HTMLElement>('.sc-model-timeline');
  const portrait = root.querySelector<HTMLElement>('.sc-model-version-portrait');
  if (!strip || !portrait) throw new Error('Missing version strip.');
  Object.defineProperties(strip, {
    clientWidth: { value: 300, configurable: true },
    scrollWidth: { value: 900, configurable: true },
  });
  return {
    strip,
    portrait,
    select,
    outerWheel,
    dispose: () => {
      app.unmount();
      root.remove();
    },
  };
}

test.each([
  [{ deltaY: 80 }, 80],
  [{ deltaX: 65 }, 65],
  [{ deltaX: 65, deltaY: 120 }, 65],
  [{ deltaY: 2, deltaMode: 1 }, 32],
  [{ deltaX: 1, deltaMode: 2 }, 300],
] as const)('scrolls version cards horizontally for wheel input %j', async (input, distance) => {
  const current = await fixture();
  try {
    current.strip.scrollLeft = 50;
    const wheel = new WheelEvent('wheel', { ...input, bubbles: true, cancelable: true });
    current.portrait.dispatchEvent(wheel);
    expect(current.strip.scrollLeft).toBe(50 + distance);
    expect(wheel.defaultPrevented).toBe(true);
    expect(current.outerWheel).not.toHaveBeenCalled();
    expect(current.select).not.toHaveBeenCalled();
  } finally {
    current.dispose();
  }
});

test('contains wheel input at both ends and ignores empty strips and browser zoom', async () => {
  const current = await fixture();
  try {
    current.strip.scrollLeft = 550;
    const end = new WheelEvent('wheel', { deltaY: 200, bubbles: true, cancelable: true });
    current.strip.dispatchEvent(end);
    expect(current.strip.scrollLeft).toBe(600);
    expect(end.defaultPrevented).toBe(true);
    current.strip.dispatchEvent(
      new WheelEvent('wheel', { deltaX: -1000, bubbles: true, cancelable: true }),
    );
    expect(current.strip.scrollLeft).toBe(0);
    const zoom = new WheelEvent('wheel', {
      deltaY: 120,
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });
    current.strip.dispatchEvent(zoom);
    expect(zoom.defaultPrevented).toBe(false);
    expect(current.strip.scrollLeft).toBe(0);
    Object.defineProperty(current.strip, 'scrollWidth', { value: 300 });
    const empty = new WheelEvent('wheel', { deltaY: 120, bubbles: true, cancelable: true });
    current.strip.dispatchEvent(empty);
    expect(current.strip.scrollLeft).toBe(0);
    expect(empty.defaultPrevented).toBe(false);
  } finally {
    current.dispose();
  }
});

test('owns horizontal wheel input before nested card handlers can consume it', async () => {
  const current = await fixture();
  try {
    current.portrait.addEventListener('wheel', (event) => {
      event.preventDefault();
      event.stopPropagation();
    });
    current.strip.scrollLeft = 50;
    const wheel = new WheelEvent('wheel', { deltaX: 65, bubbles: true, cancelable: true });
    current.portrait.dispatchEvent(wheel);
    expect(current.strip.scrollLeft).toBe(115);
    expect(wheel.defaultPrevented).toBe(true);
    expect(current.select).not.toHaveBeenCalled();
  } finally {
    current.dispose();
  }
});

test('scrolls horizontal input already claimed by an enclosing host boundary', async () => {
  const current = await fixture();
  try {
    current.strip.scrollLeft = 50;
    const wheel = new WheelEvent('wheel', { deltaX: 65, bubbles: true, cancelable: true });
    wheel.preventDefault();
    current.portrait.dispatchEvent(wheel);
    expect(current.strip.scrollLeft).toBe(115);
    expect(current.outerWheel).not.toHaveBeenCalled();
  } finally {
    current.dispose();
  }
});
