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
/** Prove installed model identities, literal labels and banner artwork cross the editor boundary unchanged. */
import {
  preparePromptLoraCatalog,
  promptLoraItem,
  promptLoraMediaRevision,
  promptLoraModel,
  promptLoraName,
} from '../../../frontend/comfyui/ui/prompt_editor/PromptLoraCatalog.js';
import { promptCatalogFixture } from '../support/PromptCatalogFixture.js';

test('prioritizes canonical installed identities only after native resolution and respects cancellation', async () => {
  const calls: string[][] = [];
  const adapter = preparePromptLoraCatalog(promptCatalogFixture(), 'http://localhost/', (items) =>
    calls.push(items.map((item) => item.id)),
  );
  await adapter.resolve(['example', 'folder/example', 'missing'], new AbortController().signal);
  expect(calls).toEqual([['example-lora']]);
  const cancellation = new AbortController();
  cancellation.abort();
  await adapter.resolve(['example'], cancellation.signal);
  expect(calls).toHaveLength(1);
});

test('resolves authored LoRA aliases to installed models and prefers the banner thumbnail', async () => {
  const adapter = preparePromptLoraCatalog(promptCatalogFixture(), 'http://127.0.0.1:8222/');
  const resources = await adapter.resolve(
    ['example', 'folder/example', 'missing'],
    new AbortController().signal,
  );
  expect(resources.map((resource) => resource.resolution)).toEqual(['found', 'found', 'missing']);
  expect(resources[0]).toMatchObject({
    model: { backendValue: 'folder/example.safetensors', promptName: 'folder/example' },
    title: 'Example <b>model</b>',
    thumbnail: {
      status: 'ready',
      url: 'http://127.0.0.1:8222/proof/banner.svg',
      width: 512,
      height: 128,
    },
  });
});

test('retains ambiguous aliases and unavailable media without inventing successful resolution', async () => {
  const snapshot = promptCatalogFixture();
  const first = snapshot.items[0];
  if (!first) throw new Error('Missing catalog fixture');
  const adapter = preparePromptLoraCatalog(
    {
      ...snapshot,
      items: [
        first,
        {
          ...first,
          id: 'second',
          value: 'other/example.safetensors',
          thumbnails: [
            {
              assetId: 'invalid',
              role: 'banner',
              width: 100,
              height: 100,
              url: 'javascript:alert(1)',
            },
          ],
        },
      ],
    },
    'http://localhost/',
  );
  const resources = await adapter.resolve(
    ['example', 'other/example'],
    new AbortController().signal,
  );
  expect(resources[0]).toMatchObject({ resolution: 'ambiguous', candidateCount: 2 });
  expect(resources[1]).toMatchObject({ resolution: 'found', thumbnail: { status: 'unavailable' } });
});

test('a library update that changes nothing about LoRAs keeps the catalog an editor already has', () => {
  const snapshot = promptCatalogFixture();
  const first = preparePromptLoraCatalog(snapshot, 'http://localhost/');
  const later = preparePromptLoraCatalog(
    { ...snapshot, revision: snapshot.revision + 1 },
    'http://localhost/',
    undefined,
    first,
  );
  expect(later).toBe(first);
});

test('an unchanged banner keeps its descriptor across a catalog update and a changed one gets a new one', async () => {
  const snapshot = promptCatalogFixture();
  const lora = snapshot.items[0];
  if (!lora) throw new Error('Missing catalog fixture');
  const banner = async (catalog: ReturnType<typeof preparePromptLoraCatalog>): Promise<unknown> => {
    const [resource] = await catalog.resolve(['folder/example'], new AbortController().signal);
    if (resource?.resolution !== 'found') throw new Error('Fixture LoRA did not resolve');
    return resource.thumbnail;
  };
  const first = preparePromptLoraCatalog(snapshot, 'http://localhost/');
  const grown = preparePromptLoraCatalog(
    {
      ...snapshot,
      revision: snapshot.revision + 1,
      items: [...snapshot.items, { ...lora, id: 'another', value: 'another/model.safetensors' }],
    },
    'http://localhost/',
    undefined,
    first,
  );
  expect(grown).not.toBe(first);
  expect(await banner(grown)).toBe(await banner(first));
  const repainted = preparePromptLoraCatalog(
    {
      ...snapshot,
      revision: snapshot.revision + 2,
      items: [
        {
          ...lora,
          thumbnails: lora.thumbnails.map((image) => ({ ...image, url: `${image.url}?changed` })),
        },
      ],
    },
    'http://localhost/',
    undefined,
    grown,
  );
  expect(await banner(repainted)).not.toBe(await banner(first));
  expect(await banner(repainted)).toMatchObject({ status: 'ready' });
});

test('installed LoRAs are completed by anything a person knows them by and map back to their record', () => {
  const snapshot = promptCatalogFixture();
  const adapter = preparePromptLoraCatalog(snapshot, 'http://localhost/');
  const catalog = adapter.completionCatalog;
  if (!catalog) throw new Error('The prompt catalog offers no LoRA completion');
  // Name, folder, base model and trigger words all find the model; nothing else does.
  for (const query of ['', 'exam', 'folder/ex', 'sdxl', 'example trigger'])
    expect(catalog.search(query).map((row) => row.item.model.backendValue)).toEqual([
      'folder/example.safetensors',
    ]);
  expect(catalog.search('no such model')).toEqual([]);
  expect(catalog.search('exam')[0]?.scheduleName).toBe('folder/example');
  expect(promptLoraModel(adapter, 'folder/example.safetensors')).toBe(snapshot.items[0]);
  expect(promptLoraModel(adapter, 'missing.safetensors')).toBeNull();
});

test('an installed LoRA is named as a prompt writes it and found as the editor describes it', () => {
  expect(promptLoraName('folder\\example.safetensors')).toBe('folder\\example');
  expect(promptLoraName('plain.ckpt')).toBe('plain');
  expect(promptLoraName('no-extension')).toBe('no-extension');
  const adapter = preparePromptLoraCatalog(promptCatalogFixture(), 'http://localhost/');
  expect(promptLoraItem(adapter, 'folder/example.safetensors')).toMatchObject({
    model: { promptName: 'folder/example', backendValue: 'folder/example.safetensors' },
    triggerWords: ['example trigger'],
  });
  expect(promptLoraItem(adapter, 'missing.safetensors')).toBeNull();
});

test('a LoRA whose image alone changed keeps the catalog and everything prepared from it', async () => {
  const snapshot = promptCatalogFixture();
  const lora = snapshot.items[0];
  if (!lora) throw new Error('Missing catalog fixture');
  const first = preparePromptLoraCatalog(snapshot, 'http://localhost/');
  const completion = first.completionCatalog;
  const repainted = {
    ...lora,
    thumbnails: lora.thumbnails.map((image) => ({ ...image, url: `${image.url}?changed` })),
  };
  const later = preparePromptLoraCatalog(
    { ...snapshot, revision: snapshot.revision + 1, items: [repainted] },
    'http://localhost/',
    undefined,
    first,
  );
  // The same catalog: no editor retires a menu, a completion popup or a gesture for an image.
  expect(later).toBe(first);
  expect(later.completionCatalog).toBe(completion);
  expect(later.revision).toBe(first.revision);
  expect(promptLoraMediaRevision(later)).toBe(1);
  // Chips, the model lookup and the editor's record all describe the new image.
  const [resource] = await later.resolve(['folder/example'], new AbortController().signal);
  expect(resource).toMatchObject({
    resolution: 'found',
    thumbnail: { status: 'ready', url: 'http://localhost/proof/banner.svg?changed' },
  });
  expect(promptLoraModel(later, lora.value)).toBe(repainted);
  expect(promptLoraItem(later, lora.value)?.thumbnail).toMatchObject({
    url: 'http://localhost/proof/banner.svg?changed',
  });
});

test('a LoRA whose name or words changed replaces the catalog', () => {
  const snapshot = promptCatalogFixture();
  const lora = snapshot.items[0];
  if (!lora) throw new Error('Missing catalog fixture');
  const first = preparePromptLoraCatalog(snapshot, 'http://localhost/');
  for (const change of [{ title: 'Renamed' }, { trainedWords: ['another trigger'] }]) {
    const later = preparePromptLoraCatalog(
      { ...snapshot, revision: snapshot.revision + 1, items: [{ ...lora, ...change }] },
      'http://localhost/',
      undefined,
      first,
    );
    expect(later).not.toBe(first);
    expect(promptLoraMediaRevision(first)).toBe(0);
  }
});
