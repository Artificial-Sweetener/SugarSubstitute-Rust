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
/** Characterize Substitute's metadata action eligibility, order and navigation safety. */
import {
  modelContextEntries,
  modelProviderPage,
} from '../../../frontend/comfyui/ui/model_library/ModelContextMenu.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';

const choice = modelPickerChoices([], 'loras', ['test.safetensors'])[0];
if (!choice) throw new Error('Missing test model.');
const model = {
  ...choice,
  id: 'a'.repeat(64),
  provider: 'civitai',
  providerModelId: '123',
  providerVersionId: '456',
  baseModel: 'Illustrious',
  providerUrl: 'https://civitai.com/models/123',
};

test('places version browsing before canvas thumbnails and disables the canvas action without output', () => {
  expect(
    modelContextEntries(model, true, true, false).map((item) => [item.label, item.enabled]),
  ).toEqual([
    ['Go to CivitAI page', true],
    ['Refresh CivitAI metadata', true],
    ['See other versions', true],
    ['Set thumbnail from canvas', false],
  ]);
  expect(
    modelContextEntries(model, true, false, true).find(
      (entry) => entry.command === 'output-thumbnail',
    )?.enabled,
  ).toBe(true);
});
test('detectors use the illustration chooser rather than metadata and output actions', () => {
  expect(
    modelContextEntries({ ...model, kind: 'ultralytics', providerUrl: '' }, true, false, false).map(
      (item) => item.command,
    ),
  ).toEqual(['detector-thumbnail']);
  expect(modelContextEntries({ ...model, providerUrl: '' }, false, false, false)).toEqual([]);
});
test.each([
  'javascript:alert(1)',
  'https://civitai.com.evil.test/models/123',
  'http://civitai.com/models/123',
  'https://secret@civitai.com/models/123',
])('rejects unsafe provider navigation %s', (value) => {
  expect(modelProviderPage(value)).toBeNull();
});

test('retains ordered provider page actions without repeating the primary link', () => {
  const linked = {
    ...model,
    providerLinks: [
      {
        providerId: 'openmodeldb',
        providerName: 'OpenModelDB',
        modelId: '4x-Fixture',
        versionId: '4x-Fixture',
        modelPageUrl: 'https://openmodeldb.info/models/4x-Fixture',
      },
      {
        providerId: 'civitai',
        providerName: 'CivitAI',
        modelId: '123',
        versionId: '456',
        modelPageUrl: 'https://civitai.com/models/123',
      },
    ],
  };
  expect(modelContextEntries(linked, true, false, false).map((entry) => entry.label)).toEqual([
    'Go to OpenModelDB page',
    'Go to CivitAI page',
    'Refresh CivitAI metadata',
    'See other versions',
    'Set thumbnail from canvas',
  ]);
});

test('offers other versions without an update and omits classification editing', () => {
  const entries = modelContextEntries(model, true, false, false);
  expect(entries.at(-2)).toMatchObject({ label: 'See other versions', enabled: true });
  expect(entries.some((entry) => entry.label === 'Content classification')).toBe(false);
});

test.each([false, true])(
  'omits other versions for a single-version family with update availability %s',
  (hasUpdate) => {
    const entries = modelContextEntries({ ...model, versionCount: 1 }, true, hasUpdate, false);
    expect(entries.some((entry) => entry.command === 'versions')).toBe(false);
    expect(entries.some((entry) => entry.command === 'output-thumbnail')).toBe(true);
    expect(
      modelContextEntries({ ...model, versionCount: 2 }, true, hasUpdate, false).some(
        (entry) => entry.command === 'versions',
      ),
    ).toBe(true);
  },
);

test.each([
  { ...model, provider: 'openmodeldb' },
  { ...model, providerModelId: '' },
  { ...model, providerVersionId: '' },
  { ...model, baseModel: '' },
  { ...model, kind: 'upscale_models' },
])('omits version browsing when provider history cannot establish a compatible family', (item) => {
  expect(
    modelContextEntries(item, true, false, false).some((entry) => entry.command === 'versions'),
  ).toBe(false);
});
