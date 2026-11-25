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
/** A catalog that replaces another keeps every unchanged model's entries and answers as one built from nothing. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { createLoraCatalogAdapter } from '../../../../../../frontend/sugarprompt/services/src/lora/catalog-adapter.ts';
import type {
  LoraCatalogItem,
  LoraResource,
} from '../../../../../../frontend/sugarprompt/services/src/lora/contracts.ts';

/** One installed model as a host describes it. */
function model(folder: string, name: string, title = name): LoraCatalogItem {
  return {
    model: {
      promptName: `${folder}/${name}`,
      backendValue: `${folder}/${name}.safetensors`,
      basename: name,
    },
    title,
    subtitle: null,
    triggerWords: [],
    thumbnail: { status: 'unavailable' },
    modelPageUrl: null,
  };
}

const NAMES = ['style/ink', 'ink', 'Ink Wash', 'pose/ink', 'style/oil', 'oil', 'Oil Paint', 'gone'];

/** What a catalog answers for every name, without the item identities. */
async function answers(items: readonly LoraCatalogItem[], previous?: ReturnType<typeof adapt>) {
  const adapter = adapt(items, previous);
  const found = await adapter.resolve(NAMES, new AbortController().signal);
  return {
    adapter,
    found: found.map((resource: LoraResource) =>
      resource.resolution === 'found'
        ? `${resource.name} -> ${resource.model.backendValue} "${resource.title}"`
        : `${resource.name} -> ${resource.resolution}`,
    ),
  };
}

function adapt(
  items: readonly LoraCatalogItem[],
  previous?: ReturnType<typeof createLoraCatalogAdapter>,
) {
  return createLoraCatalogAdapter(
    { revision: String(items.length), authoritative: true, items },
    { searchMetadata: [] },
    previous,
  );
}

test('a replacing catalog answers exactly as one built from nothing, through renames, arrivals and departures', async () => {
  const ink = model('style', 'ink'),
    oil = model('style', 'oil'),
    gone = model('style', 'gone');
  const first = await answers([ink, oil, gone]);
  // A lookup named one model, a second model with the same file name arrived, a third left.
  const named = model('style', 'ink', 'Ink Wash');
  const next = [named, oil, model('pose', 'ink')];
  const continued = await answers(next, first.adapter);
  const fresh = await answers(next.map((item) => ({ ...item })));
  assert.deepEqual(continued.found, fresh.found);
  assert.equal(continued.found.includes('Ink Wash -> style/ink.safetensors "Ink Wash"'), true);
  assert.equal(continued.found.includes('ink -> ambiguous'), true);
  assert.equal(continued.found.includes('gone -> missing'), true);
  // And back again: what left is found once more, what arrived is gone.
  const restored = await answers([ink, oil, gone], continued.adapter);
  assert.deepEqual(restored.found, first.found);
});

test('an unchanged model keeps the same item in the catalog that replaces it', async () => {
  const ink = model('style', 'ink'),
    oil = model('style', 'oil');
  const first = adapt([ink, oil]);
  const second = adapt([ink, model('style', 'oil', 'Oil Paint')], first);
  const signal = new AbortController().signal;
  const [before] = first.completionCatalog!.search('ink');
  const [after] = second.completionCatalog!.search('ink');
  assert.equal(before?.item, after?.item);
  const [renamed] = second.completionCatalog!.search('oil paint');
  assert.equal(renamed?.item.title, 'Oil Paint');
  assert.equal((await second.resolve(['Oil Paint'], signal))[0]?.resolution, 'found');
});
