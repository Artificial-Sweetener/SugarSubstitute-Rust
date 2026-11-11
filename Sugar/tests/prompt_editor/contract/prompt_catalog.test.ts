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
import { preparePromptLoraCatalog } from '../../../frontend/comfyui/ui/prompt_editor/PromptLoraCatalog.js';
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
