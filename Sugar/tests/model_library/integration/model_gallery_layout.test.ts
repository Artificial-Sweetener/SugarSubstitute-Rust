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
/** Prove gallery rendering uses the reference's stable largest-thumbnail aspect ratio. */
import { createApp, nextTick } from 'vue';
import { jest, test, expect } from '@jest/globals';
import { ModelPickerWall } from '../../../frontend/comfyui/ui/model_library/vue/ModelPickerWall.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import type { ModelThumbnail } from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';
import { ModelFixtureResizeObserver } from '../support/resize_observer.js';

test('uses the largest standard preview for layout even when smaller rounded variants differ', async () => {
  Object.defineProperty(window, 'ResizeObserver', {
    configurable: true,
    value: ModelFixtureResizeObserver,
  });
  const base = modelPickerChoices([], 'checkpoints', ['model.safetensors'])[0];
  if (!base) throw new Error('Missing gallery model.');
  const thumbnails = [
    { assetId: 'a'.repeat(64), role: 'standard', width: 512, height: 512, url: '/small' },
    { assetId: 'b'.repeat(64), role: 'standard', width: 1280, height: 640, url: '/large' },
    { assetId: 'c'.repeat(64), role: 'banner', width: 1800, height: 100, url: '/banner' },
  ] satisfies readonly ModelThumbnail[];
  const host = document.createElement('div');
  document.body.append(host);
  const app = createApp(ModelPickerWall, {
    items: [{ ...base, thumbnails }],
    selected: base.value,
    active: 0,
    optionPrefix: 'layout',
  });
  app.mount(host);
  try {
    await nextTick();
    expect(host.querySelector<HTMLElement>('.sc-model-tile')?.style.width).toBe('396px');
  } finally {
    app.unmount();
    host.remove();
    jest.restoreAllMocks();
  }
});
