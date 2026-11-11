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
/** Keep native chrome opaque until frame acknowledgement, with browser material unchanged. */
import { jest } from '@jest/globals';
import { createApp, nextTick } from 'vue';
import { mountCanvasOverlay } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayRuntime.js';
import { CanvasComparisonNavigation } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasComparisonNavigation.js';
import { CanvasPortFixture, canvasSnapshot } from '../support/CanvasPortFixture.js';

afterEach(() => {
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

test('native readiness alone repaints navigation without a camera or source transition', async () => {
  const root = document.createElement('section');
  document.body.append(root);
  const port = new CanvasPortFixture({ ...canvasSnapshot('native'), nativeBackdrop: 'pending' });
  const mounted = mountCanvasOverlay(root, port, jest.fn());
  try {
    const bar = root.querySelector<HTMLElement>('[data-sc-canvas-backdrop]');
    expect(bar?.dataset.scNativeBackdrop).toBe('pending');
    expect(root.querySelector('.sc-output-backdrop')).toBeNull();
    expect(root.querySelector('.sc-output-backdrop-image')).toBeNull();
    port.publish({ ...port.read(), revision: port.read().revision + 1, nativeBackdrop: 'ready' });
    await nextTick();
    expect(root.querySelector('[data-sc-canvas-backdrop]')).toBe(bar);
    expect(bar?.dataset.scNativeBackdrop).toBe('ready');
    port.publish({ ...port.read(), revision: port.read().revision + 1, nativeBackdrop: 'pending' });
    await nextTick();
    expect(bar?.dataset.scNativeBackdrop).toBe('pending');
  } finally {
    mounted.dispose();
  }
});

test('both comparison bars use the same acknowledged material state', async () => {
  const root = document.createElement('section');
  document.body.append(root);
  const app = createApp(CanvasComparisonNavigation, {
    snapshot: { ...canvasSnapshot('native'), nativeBackdrop: 'ready' as const },
    send: () => {},
    chooseRoute: () => {},
  });
  try {
    app.mount(root);
    await nextTick();
    expect(
      [...root.querySelectorAll<HTMLElement>('[data-sc-canvas-backdrop]')].map(
        (bar) => bar.dataset.scNativeBackdrop,
      ),
    ).toEqual(['ready', 'ready']);
    expect(root.querySelector('[style*="background-image"]')).toBeNull();
  } finally {
    app.unmount();
  }
});

test('browser bars retain CSS blur while native readiness uses exactly one translucent tint', () => {
  const root = document.createElement('section');
  document.body.append(root);
  const mounted = mountCanvasOverlay(
    root,
    new CanvasPortFixture(canvasSnapshot('browser')),
    jest.fn(),
  );
  try {
    const rules = [...document.styleSheets].flatMap((sheet) => [...sheet.cssRules]);
    const rule = (selector: string) =>
      rules.find(
        (entry): entry is CSSStyleRule =>
          entry instanceof CSSStyleRule && entry.selectorText === selector,
      )?.style;
    expect(root.querySelector('[data-sc-native-backdrop]')).toBeNull();
    expect(rule('.sc-output-tabstrip')?.getPropertyValue('backdrop-filter')).toBe(
      'blur(var(--p-toast-blur,10px))',
    );
    expect(
      rule('.sc-output-tabstrip[data-sc-native-backdrop]')?.getPropertyValue('backdrop-filter'),
    ).toBe('none');
    expect(
      rule('.sc-output-tabstrip[data-sc-native-backdrop]')?.getPropertyValue('background'),
    ).toContain('Canvas');
    expect(
      rule('.sc-output-tabstrip[data-sc-native-backdrop="ready"]')?.getPropertyValue('background'),
    ).toBe('var(--sc-output-glass)');
    expect(rule('.sc-output-backdrop-image')).toBeUndefined();
  } finally {
    mounted.dispose();
  }
});
