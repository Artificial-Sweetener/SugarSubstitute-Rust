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
/** Prove a dropped image's run is found by its content and shown under the outputs that made it. */
import { webcrypto } from 'node:crypto';
import { beforeAll, expect, jest, test } from '@jest/globals';
import {
  findRunImages,
  runImagesBySource,
  type RunImage,
} from '../../../frontend/comfyui/ui/execution/infrastructure/RunImages.js';
import { decodeComfyWorkflowSources } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowSourceDecoder.js';

test('each run image goes under the Cube output that made it, in the order the run made them', () => {
  const sources = decodeComfyWorkflowSources({
    nodes: [
      {
        id: 'cube-a',
        type: 'def-a',
        properties: { sugarcubes_kind: 'cube', sugarcubes_cube: { instance_id: 'cube-a' } },
        outputs: [{ name: 'image', type: 'IMAGE' }],
      },
      {
        id: 'cube-b',
        type: 'def-b',
        properties: { sugarcubes_kind: 'cube', sugarcubes_cube: { instance_id: 'cube-b' } },
        outputs: [{ name: 'image', type: 'IMAGE' }],
      },
    ],
    definitions: { subgraphs: [] },
  });
  const paths = sources.map((source) => source.executionPath);
  expect(paths.length).toBeGreaterThan(0);
  const image = (path: string, index: number, name: string): RunImage => ({
    image: { filename: name, subfolder: '', type: 'output' },
    sourceNodeId: path,
    imageIndex: index,
  });
  const first = paths[0] ?? '';
  const groups = runImagesBySource(sources, [
    image(first, 2, 'second.png'),
    image('elsewhere:0', 1, 'stranger.png'),
    image(first, 1, 'first.png'),
  ]);
  expect(groups.map((group) => group.source.executionPath)).toEqual([first]);
  expect(groups[0]?.images.map((entry) => (entry as { filename: string }).filename)).toEqual([
    'first.png',
    'second.png',
  ]);
});

beforeAll(() => {
  // The browser hashes with Web Crypto, which the test document does not provide.
  if (!globalThis.crypto?.subtle) Object.defineProperty(globalThis, 'crypto', { value: webcrypto });
});

test('the dropped file is described by its name, size and content hash', async () => {
  const body = {
    images: [
      { image: { filename: 'a.png' }, source_node_id: 'cube:0', image_index: 1 },
      { image: 'broken', source_node_id: 'cube:0', image_index: 2 },
    ],
  };
  const fetchApi = jest.fn(
    async (_path: string, _init?: RequestInit) =>
      ({ ok: true, json: async () => body }) as unknown as Response,
  );
  // The test document's File cannot read its bytes back; this one reads them as a browser's does.
  const bytes = new Uint8Array([1, 2, 3]);
  const file = { name: 'a.png', size: 3, arrayBuffer: async () => bytes.buffer } as unknown as File;
  const images = await findRunImages({ fetchApi }, file, 'run-7');
  expect(images).toEqual([{ image: { filename: 'a.png' }, sourceNodeId: 'cube:0', imageIndex: 1 }]);
  const sent = JSON.parse(String(fetchApi.mock.calls[0]?.[1]?.body)) as Record<string, unknown>;
  expect(sent).toEqual({
    filename: 'a.png',
    size: 3,
    // SHA-256 of the bytes 01 02 03.
    sha256: '039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81',
    run_id: 'run-7',
  });
});
