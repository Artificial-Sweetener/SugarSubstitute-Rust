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
/** Prove a new catalog generation keeps what it left unchanged, for every consumer at once. */
import { expect, test } from '@jest/globals';
import { continueModelCatalog } from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';
import type {
  ModelCatalogItem,
  ModelCatalogSnapshot,
} from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';

/** One installed model as the decoder publishes it: a new object on every call. */
function model(id: string, banner: string): ModelCatalogItem {
  return {
    id,
    kind: 'loras',
    value: `${id}.safetensors`,
    rootId: 'models',
    relativePath: `${id}.safetensors`,
    folder: '',
    title: id,
    version: 'v1',
    baseModel: 'SDXL',
    provider: '',
    providerUrl: '',
    providerModelId: '',
    providerVersionId: '',
    providerLinks: [],
    description: '',
    trainedWords: [],
    thumbnails: [{ assetId: banner, role: 'banner', width: 512, height: 128, url: `/a/${banner}` }],
  };
}

/** One decoded generation of the catalog. */
function generation(revision: number, items: ModelCatalogItem[]): ModelCatalogSnapshot {
  return { revision, scanning: false, fieldContracts: [], items };
}

test('a generation that changes no model keeps every entry and the list itself', () => {
  const first = generation(1, [model('a', 'one'), model('b', 'two')]);
  const second = continueModelCatalog(first, generation(2, [model('a', 'one'), model('b', 'two')]));
  expect(second.revision).toBe(2);
  expect(second.items).toBe(first.items);
  expect(second.fieldContracts).toBe(first.fieldContracts);
});

test('a generation that changes one model replaces only that entry', () => {
  const first = generation(1, [model('a', 'one'), model('b', 'two')]);
  const second = continueModelCatalog(
    first,
    generation(2, [model('a', 'one'), model('b', 'new'), model('c', 'three')]),
  );
  expect(second.items).not.toBe(first.items);
  expect(second.items[0]).toBe(first.items[0]);
  expect(second.items[1]).not.toBe(first.items[1]);
  expect(second.items[1]?.thumbnails[0]?.url).toBe('/a/new');
  expect(second.items).toHaveLength(3);
});

test('the first generation is published as decoded', () => {
  const first = generation(1, [model('a', 'one')]);
  expect(continueModelCatalog(null, first)).toBe(first);
});
