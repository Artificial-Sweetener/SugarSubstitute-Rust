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
/** Keep the canvas navigation solid: no bar or picker over the canvas is translucent or blurred. */
import { jest } from '@jest/globals';
import { createApp, nextTick } from 'vue';
import { mountCanvasOverlay } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayRuntime.js';
import { CanvasComparisonNavigation } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasComparisonNavigation.js';
import { CanvasPortFixture, canvasSnapshot } from '../support/CanvasPortFixture.js';

afterEach(() => {
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

test('the navigation bar asks the native canvas to blur nothing, whatever its material state', async () => {
  const root = document.createElement('section');
  document.body.append(root);
  const port = new CanvasPortFixture({ ...canvasSnapshot('native'), nativeBackdrop: 'pending' });
  const mounted = mountCanvasOverlay(root, port, jest.fn());
  try {
    const bar = root.querySelector<HTMLElement>('.sc-output-tabstrip');
    expect(bar).not.toBeNull();
    port.publish({ ...port.read(), revision: port.read().revision + 1, nativeBackdrop: 'ready' });
    await nextTick();
    expect(root.querySelector('.sc-output-tabstrip')).toBe(bar);
    expect(root.querySelector('[data-sc-canvas-backdrop]')).toBeNull();
    expect(root.querySelector('[data-sc-native-backdrop]')).toBeNull();
  } finally {
    mounted.dispose();
  }
});

test('neither comparison bar is offered as an acrylic surface', async () => {
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
    expect(root.querySelectorAll('.sc-output-tabstrip')).toHaveLength(2);
    expect(root.querySelector('[data-sc-canvas-backdrop]')).toBeNull();
    expect(root.querySelector('[data-sc-native-backdrop]')).toBeNull();
  } finally {
    app.unmount();
  }
});

test('bars and pickers are one solid surface in the browser and over the native canvas', () => {
  const root = document.createElement('section');
  document.body.append(root);
  const mounted = mountCanvasOverlay(
    root,
    new CanvasPortFixture(canvasSnapshot('browser')),
    jest.fn(),
  );
  try {
    const rules = [...document.styleSheets]
      .flatMap((sheet) => [...sheet.cssRules])
      .filter((entry): entry is CSSStyleRule => entry instanceof CSSStyleRule);
    const surfaces = rules.filter((entry) =>
      [
        '.sc-output-tabstrip,.sc-output-status',
        '.sc-output-tabstrip',
        '.sc-output-picker-popup',
      ].includes(entry.selectorText.replace(/,\s+/g, ',')),
    );
    expect(surfaces).toHaveLength(3);
    for (const entry of surfaces) {
      expect(entry.style.getPropertyValue('backdrop-filter')).toBe('');
      expect(entry.style.getPropertyValue('background')).not.toContain('transparent');
    }
    expect(rules.some((entry) => entry.selectorText.includes('data-sc-native-backdrop'))).toBe(
      false,
    );
  } finally {
    mounted.dispose();
  }
});
