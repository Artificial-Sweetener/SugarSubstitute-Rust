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
/** Prove Substitute's adaptive output hierarchy and Comfy's completed-image boundary. */
import { modelOutputThumbnailMenu } from '../../../frontend/comfyui/ui/model_library/ModelOutputThumbnailMenu.js';
import type { ModelOutputThumbnailChoice } from '../../../frontend/comfyui/ui/model_library/ModelOutputThumbnails.js';
import { ComfyOutputThumbnailSource } from '../../../frontend/comfyui/ui/model_library/ComfyOutputThumbnailSource.js';
import { buildCubeOutputExecutionId } from '../../../frontend/comfyui/ui/cube/execution/CubeOutputExecutionIdentity.js';

/** Construct one concrete reference choice without coupling grouping to image transport. */
function choice(
  id: string,
  overrides: Partial<ModelOutputThumbnailChoice> = {},
): ModelOutputThumbnailChoice {
  return {
    id,
    sceneKey: '',
    sceneTitle: '',
    sceneOrder: 0,
    sourceKey: 'cube',
    sourceLabel: 'Result',
    batch: 1,
    ...overrides,
  };
}

test('matches disabled, singleton and current-image submenu states', () => {
  expect(modelOutputThumbnailMenu([], null).enabled).toBe(false);
  const first = choice('first');
  expect(modelOutputThumbnailMenu([first], first)).toMatchObject({
    imageId: 'first',
    command: 'output-thumbnail',
  });
  const second = choice('second');
  expect(modelOutputThumbnailMenu([first, second], second).children).toMatchObject([
    { label: 'Current image', imageId: 'second' },
    { separator: true },
    {
      label: 'Result',
      children: [
        { label: 'Image 1', imageId: 'first' },
        { label: 'Image 2', imageId: 'second' },
      ],
    },
  ]);
  expect(modelOutputThumbnailMenu([first, second], choice('stale')).children?.[0]?.label).toBe(
    'Result',
  );
});

test('reuses captured cube boundary results and labels before deduplicating internal output copies', () => {
  const images = [{ filename: 'cube-result.png', type: 'output' }];
  const graph = {
    _nodes: [
      {
        id: 'cube',
        title: 'SDXL/Text to Image',
        subgraph: { outputs: [{ name: 'image' }], _nodes: [{ id: 2, title: 'VAE Decode' }] },
      },
    ],
  };
  const marker = buildCubeOutputExecutionId('cube', 0);
  const source = new ComfyOutputThumbnailSource(
    { graph, nodeOutputs: { 'cube:2': { images } } },
    document,
    (executionId) => (executionId === marker ? { images } : undefined),
  );
  expect(source.choices()).toHaveLength(1);
  expect(source.choices()[0]).toMatchObject({
    sourceKey: marker,
    sourceLabel: 'SDXL/Text to Image / image',
  });
});

test.each([0, 2])(
  'keeps Current image for a deduplicated native output at index %i',
  (imageIndex) => {
    const images = [
      { filename: 'shared.png', type: 'output' },
      { filename: 'other.png', type: 'output' },
      { filename: 'shared.png', type: 'output' },
    ];
    const native = { id: 2, title: 'Save Image', imageIndex };
    const cube = {
      id: 'cube',
      title: 'Result cube',
      subgraph: { outputs: [{ name: 'image' }], _nodes: [native] },
    };
    const graph = { _nodes: [cube] };
    const app = {
      graph,
      nodeOutputs: { 'cube:2': { images } },
      canvas: { selected_nodes: { '2': native } },
    };
    const marker = buildCubeOutputExecutionId('cube', 0);
    const source = new ComfyOutputThumbnailSource(app, document, (executionId) =>
      executionId === marker ? { images } : undefined,
    );
    const choices = source.choices();
    expect(choices).toHaveLength(2);
    expect(source.activeChoice()).toEqual(choices[0]);
    expect(choices[0]?.sourceKey).toBe(marker);
    expect(modelOutputThumbnailMenu(choices, source.activeChoice()).children?.[0]).toMatchObject({
      label: 'Current image',
      imageId: choices[0]?.id,
    });
    native.imageIndex = 1;
    expect(source.activeChoice()).toEqual(choices[1]);
    native.imageIndex = 3;
    expect(source.activeChoice()).toBeNull();
    graph._nodes = [];
    expect(source.activeChoice()).toBeNull();
  },
);

test('collapses singleton scenes and batches while preserving source and image ordering', () => {
  const images = [
    choice('later', { sceneKey: 'b', sceneTitle: 'Later scene', sceneOrder: 2 }),
    choice('one', { sceneKey: 'a', sceneTitle: 'First scene', sceneOrder: 1 }),
    choice('two', { sceneKey: 'a', sceneTitle: 'First scene', sceneOrder: 1, batch: 2 }),
    choice('three', {
      sceneKey: 'a',
      sceneTitle: 'First scene',
      sceneOrder: 1,
      batch: 2,
      sourceKey: 'other',
      sourceLabel: 'Other output',
    }),
  ];
  expect(modelOutputThumbnailMenu(images, null).children).toMatchObject([
    {
      label: 'First scene',
      children: [
        { label: 'Batch 1', imageId: 'one' },
        {
          label: 'Batch 2',
          children: [
            { label: 'Result', imageId: 'two' },
            { label: 'Other output', imageId: 'three' },
          ],
        },
      ],
    },
    { label: 'Later scene', imageId: 'later' },
  ]);
});

test('orders equal-priority scenes by Unicode scalar title and key like Substitute', () => {
  const choices = ['a', '\u{1f600}', '\ue000', 'Z'].map((title) =>
    choice(title, { sceneKey: title, sceneTitle: title }),
  );
  expect(modelOutputThumbnailMenu(choices, null).children?.map((entry) => entry.label)).toEqual([
    'Z',
    'a',
    '\ue000',
    '\u{1f600}',
  ]);
  expect(
    modelOutputThumbnailMenu(
      [
        choice('lower', { sceneKey: 'a', sceneTitle: 'Same' }),
        choice('upper', { sceneKey: 'Z', sceneTitle: 'Same' }),
      ],
      null,
    ).children?.map((entry) => entry.imageId),
  ).toEqual(['upper', 'lower']);
});

test('reads durable nested outputs, deduplicates concrete images and ignores previews or orphaned records', async () => {
  const node = { id: 7, title: '<img>Result', imageIndex: 1 };
  const graph = { _nodes: [{ id: 3, title: 'Cube', subgraph: { _nodes: [node] } }] };
  const images = [
    { filename: 'first.png', subfolder: 'test', type: 'output' },
    { filename: 'second.png', type: 'temp' },
    { filename: 'first.png', subfolder: 'test', type: 'output' },
    { filename: 'input.png', type: 'input' },
    { url: 'https://evil.test/image.png', type: 'output' },
  ];
  const app = {
    graph,
    nodeOutputs: { '3:7': { images }, orphan: { images } },
    nodePreviewImages: { '3:7': ['blob:live'] },
    canvas: { selected_nodes: { '7': node } },
  };
  const source = new ComfyOutputThumbnailSource(app, document, () => undefined);
  expect(source.choices()).toHaveLength(2);
  expect(source.choices()[0]?.sourceLabel).toBe('Cube / <img>Result');
  expect(source.activeChoice()?.id).toBe(source.choices()[1]?.id);
  node.imageIndex = -1;
  expect(source.activeChoice()).toBeNull();
  app.nodeOutputs['3:7'].images = [];
  await expect(source.image('stale', new AbortController().signal)).rejects.toThrow(
    'no longer available',
  );
  expect(source.choices()).toEqual([]);
});
