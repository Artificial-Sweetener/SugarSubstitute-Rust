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
/** Prove detector art selection uses a separate modal and preserves the native model value. */
import { nextTick } from 'vue';
import { jest } from '@jest/globals';
import { ModelFixtureResizeObserver } from '../support/resize_observer.js';
import { ModelDetectorDialog } from '../../../frontend/comfyui/ui/model_library/ModelDetectorDialog.js';
import type { ModelDetectorPort } from '../../../frontend/comfyui/ui/model_library/ModelDetectors.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import { modelPickerCss } from '../../../frontend/comfyui/ui/model_library/ModelPickerStyles.js';

beforeEach(() => {
  Object.defineProperty(window, 'ResizeObserver', {
    value: ModelFixtureResizeObserver,
    configurable: true,
  });
  const style = document.createElement('style');
  style.textContent = modelPickerCss;
  document.head.append(style);
});

/** Settle deterministic Vue rendering without relying on timers or remote providers. */
async function settle(): Promise<void> {
  await Promise.resolve();
  await nextTick();
}

test('renders the reference library, immediately commits only art, and renders labels literally', async () => {
  const item = modelPickerChoices([], 'ultralytics', ['bbox/face.pt'])[0];
  if (!item) throw new Error('Missing fixture.');
  const choices = modelPickerChoices([], 'ultralytics', [
    'face-detection',
    'face-segmentation',
  ]).map((value) => ({
    ...value,
    title: '<b>Face</b>',
    version: value.value.endsWith('segmentation') ? 'Segmentation' : 'Bounding Box',
  }));
  const assign = jest.fn(async (_value: string, _asset: string, _signal: AbortSignal) => undefined);
  const port: ModelDetectorPort = {
    resolve: jest.fn(async () => [item]),
    library: jest.fn(async () => choices),
    assign,
  };
  const dialog = new ModelDetectorDialog(document, item, port);
  const completion = dialog.open();
  await settle();
  const overlay = document.querySelector('.sc-model-detector-overlay');
  const modal = overlay?.querySelector<HTMLElement>('.sc-model-detector-modal');
  if (!modal) throw new Error('Missing detector modal.');
  expect(getComputedStyle(modal).display).toBe('flex');
  const cancel = modal.querySelector<HTMLButtonElement>('.p-button-secondary');
  if (!cancel) throw new Error('Missing detector cancel action.');
  expect(getComputedStyle(cancel).fontSize).toBe('14px');
  const footer = modal.querySelector<HTMLElement>('.p-dialog-footer');
  const content = modal.querySelector<HTMLElement>('.p-dialog-content');
  if (!footer || !content) throw new Error('Missing detector layout regions.');
  expect(getComputedStyle(footer).height).toBe('81px');
  expect(getComputedStyle(content).paddingBottom).toBe('24px');
  expect(overlay?.textContent).toContain('Choose detector thumbnail for face');
  expect(overlay?.querySelector('b')).toBeNull();
  expect(assign).not.toHaveBeenCalled();
  const second = overlay?.querySelectorAll<HTMLElement>('[role=option]')[1];
  second?.click();
  await settle();
  await completion;
  expect(assign.mock.calls[0]?.slice(0, 2)).toEqual(['bbox/face.pt', 'face-segmentation']);
  expect(document.querySelector('.sc-model-detector-overlay')).toBeNull();
  expect(item.value).toBe('bbox/face.pt');
});

test('keeps failed preference selection visible and aborts it on close', async () => {
  const item = modelPickerChoices([], 'ultralytics', ['unknown.pt'])[0];
  if (!item) throw new Error('Missing fixture.');
  const port: ModelDetectorPort = {
    resolve: async () => [item],
    library: async () => [item],
    assign: async () => {
      throw new Error('<img> storage unavailable');
    },
  };
  const dialog = new ModelDetectorDialog(document, item, port);
  const completion = dialog.open();
  await settle();
  document.querySelector<HTMLElement>('.sc-model-detector-overlay [role=option]')?.click();
  await settle();
  expect(document.querySelector('.sc-model-detector-overlay')?.textContent).toContain(
    '<img> storage unavailable',
  );
  expect(document.querySelector('.sc-model-detector-overlay img')).toBeNull();
  dialog.close();
  await completion;
});
