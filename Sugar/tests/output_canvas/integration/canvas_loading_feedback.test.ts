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
/** Prove loading feedback remains delayed, accessible, and attachment-scoped. */
import { jest } from '@jest/globals';
import { nextTick } from 'vue';
import { mountCanvasOverlay } from '../../../frontend/comfyui/ui/output_canvas/presentation/vue/CanvasOverlayRuntime.js';
import { CanvasPortFixture, canvasSnapshot } from '../support/CanvasPortFixture.js';
import { browserCanvasSnapshot } from '../../../frontend/comfyui/ui/output_canvas/presentation/browser/BrowserCanvasSnapshot.js';
import { canvasDisplayPlan } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasDisplayPlan.js';
import { canvasCatalogFixture } from '../support/CanvasCatalogFixture.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.useRealTimers();
});

test('renders an unselected canvas without an empty-state status bar', () => {
  const initial = canvasSnapshot('empty-canvas');
  const catalog = canvasCatalogFixture();
  const view = { ...initial.view, sessionId: catalog.sessionId, selection: null };
  const snapshot = browserCanvasSnapshot(
    initial.address,
    canvasDisplayPlan(view, catalog, { primary: null, secondary: null }, null),
    catalog,
    [],
    null,
    true,
    false,
    [],
    false,
    null,
  );
  const root = document.createElement('section');
  document.body.append(root);
  const mounted = mountCanvasOverlay(root, new CanvasPortFixture(snapshot), jest.fn());
  expect(root.querySelector('.sc-output-status')).toBeNull();
  expect(root.textContent).not.toContain('Choose a workflow output');
  mounted.dispose();
});

test.each([true, false])(
  'reports actual displayed preview=%s after the desired plan changes',
  (preview) => {
    const initial = canvasSnapshot('displayed-preview');
    const catalog = canvasCatalogFixture();
    const view = { ...initial.view, sessionId: catalog.sessionId };
    const snapshot = browserCanvasSnapshot(
      initial.address,
      canvasDisplayPlan(view, catalog, { primary: null, secondary: null }, null),
      catalog,
      [
        {
          slot: 'primary',
          frame: {
            key: 'displayed',
            runOrder: 1,
            previewLane: preview ? 'preview:stream' : null,
            selection: view.selection!,
            preview,
            handle: document.createElement('img'),
            dimensions: { width: 512, height: 288 },
          },
          status: { state: 'loading', message: '', canRetry: false, canAdmitLargeImage: false },
        },
      ],
      null,
      true,
      true,
      [],
      false,
      null,
    );
    expect(snapshot.preview).toBe(preview);
  },
);

/** Mount a real reusable overlay with an image decode pending. */
function loadingOverlay() {
  jest.useFakeTimers();
  const initial = canvasSnapshot('loading-feedback');
  const port = new CanvasPortFixture({
    ...initial,
    resource: { ...initial.resource, state: 'loading', message: 'Now loading' },
  });
  const root = document.createElement('section');
  document.body.append(root);
  const mounted = mountCanvasOverlay(root, port, jest.fn());
  return { root, port, mounted };
}

test('shows only an accessible spinner after a continuous 250 ms wait', async () => {
  const { root, port, mounted } = loadingOverlay();
  expect(root.textContent).not.toContain('Now loading');
  expect(root.querySelector('.sc-output-status')).toBeNull();
  jest.advanceTimersByTime(249);
  await nextTick();
  expect(root.querySelector('.sc-output-loading')).toBeNull();
  for (let revision = 2; revision < 20; revision++) {
    port.publish({ ...port.read(), revision });
    await nextTick();
  }
  jest.advanceTimersByTime(1);
  await nextTick();
  expect(root.querySelector('[role="status"][aria-label="Loading workflow image"]')).not.toBeNull();
  expect(root.querySelector('.sc-output-spinner')?.getAttribute('aria-hidden')).toBe('true');
  port.publish({
    ...port.read(),
    revision: port.read().revision + 1,
    resource: { ...port.read().resource, state: 'ready' },
  });
  await nextTick();
  expect(root.querySelector('.sc-output-loading')).toBeNull();
  mounted.dispose();
});

test('short preview loads never flash and retirement cancels pending feedback', async () => {
  const { root, port, mounted } = loadingOverlay();
  for (let frame = 0; frame < 100; frame++) {
    jest.advanceTimersByTime(100);
    port.publish({
      ...port.read(),
      revision: port.read().revision + 1,
      resource: { ...port.read().resource, state: 'ready' },
    });
    await nextTick();
    expect(root.querySelector('.sc-output-loading')).toBeNull();
    port.publish({
      ...port.read(),
      revision: port.read().revision + 1,
      resource: { ...port.read().resource, state: 'loading' },
    });
    await nextTick();
  }
  mounted.dispose();
  expect(jest.getTimerCount()).toBe(0);
  jest.advanceTimersByTime(1000);
  await nextTick();
  expect(root.childElementCount).toBe(0);
});

test('covers comparison loads and removes the spinner on failure or disconnection', async () => {
  const { root, port, mounted } = loadingOverlay();
  port.publish({
    ...port.read(),
    revision: port.read().revision + 1,
    resource: { ...port.read().resource, state: 'ready' },
    secondaryResource: { ...port.read().resource, state: 'loading' },
  });
  await nextTick();
  jest.advanceTimersByTime(250);
  await nextTick();
  expect(root.querySelector('.sc-output-loading')).not.toBeNull();
  port.publish({
    ...port.read(),
    revision: port.read().revision + 1,
    secondaryResource: {
      ...port.read().resource,
      state: 'error',
      message: 'Image unavailable',
      canRetry: true,
    },
  });
  await nextTick();
  expect(root.querySelector('.sc-output-loading')).toBeNull();
  expect(root.textContent).toContain('Image unavailable');
  expect(root.textContent).toContain('Retry image');
  port.publish({
    ...port.read(),
    revision: port.read().revision + 1,
    connected: false,
    secondaryResource: { ...port.read().resource, state: 'loading' },
  });
  await nextTick();
  jest.advanceTimersByTime(250);
  await nextTick();
  expect(root.querySelector('.sc-output-loading')).toBeNull();
  expect(root.textContent).toContain('Disconnected');
  mounted.dispose();
});
