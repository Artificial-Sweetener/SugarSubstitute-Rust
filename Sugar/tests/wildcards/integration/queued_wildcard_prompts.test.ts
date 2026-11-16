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
/** Prove a queued run's prompt texts are resolved for both ways a run is sent. */
import { jest } from '@jest/globals';
import { WildcardLibrary } from '../../../frontend/comfyui/ui/wildcards/application/WildcardLibrary.js';
import {
  ComfyWildcardFiles,
  type WildcardApiResponse,
} from '../../../frontend/comfyui/ui/wildcards/infrastructure/ComfyWildcardFiles.js';
import {
  ComfyWildcardPrompts,
  queuedPromptSeed,
} from '../../../frontend/comfyui/ui/wildcards/infrastructure/ComfyWildcardPrompts.js';
import type { ComfyGraph } from '../../../frontend/comfyui/ui/types/graph.js';

/** Answer Comfy's per-user file routes from an in-memory wildcard folder. */
function userdata(files: Record<string, string>) {
  const requests: string[] = [];
  const response = (status: number, body: unknown): WildcardApiResponse => ({
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
    text: async () => String(body),
  });
  const fetchApi = jest.fn(async (path: string, options?: RequestInit) => {
    requests.push(`${options?.method ?? 'GET'} ${path}`);
    if (path.startsWith('/userdata?')) {
      const query = new URLSearchParams(path.slice('/userdata?'.length));
      expect(Object.fromEntries(query)).toEqual({
        dir: 'sugar/wildcards',
        recurse: 'true',
        full_info: 'true',
      });
      return Object.keys(files).length
        ? response(
            200,
            Object.entries(files).map(([name, text]) => ({
              path: name,
              size: text.length,
              modified: 1,
            })),
          )
        : response(404, 'Directory not found');
    }
    const [location, action, destination] = path
      .split('?')[0]!
      .slice('/userdata/'.length)
      .split('/');
    const name = decodeURIComponent(location ?? '').replace(/^sugar\/wildcards\//u, '');
    if (action === 'move') {
      const target = decodeURIComponent(destination ?? '').replace(/^sugar\/wildcards\//u, '');
      if (!(name in files)) return response(404, '');
      if (target in files) return response(409, '');
      files[target] = files[name] ?? '';
      Reflect.deleteProperty(files, name);
      return response(200, target);
    }
    if (options?.method === 'POST') {
      if (name in files) return response(409, '');
      files[name] = String(options.body);
      return response(200, name);
    }
    return name in files ? response(200, files[name]) : response(404, '');
  });
  return { fetchApi, requests, files };
}

/** A live graph with one top-level prompt node and one Cube holding a sampler and a prompt. */
function graph(): ComfyGraph {
  const text = (name: string) => ({ name, type: 'customtext', value: '' });
  return {
    nodes: [
      { id: 9, widgets: [text('value')] },
      {
        id: 4,
        subgraph: {
          nodes: [
            { id: 2, widgets: [] },
            { id: 3, widgets: [text('text')] },
          ],
        },
      },
    ],
  } as unknown as ComfyGraph;
}

/** The prompt Comfy built from that graph. */
function output() {
  return {
    '9': { class_type: 'PrimitiveStringMultiline', inputs: { value: 'a {colors} sky' } },
    '4:2': { class_type: 'KSampler', inputs: { noise_seed: 7, seed: 42, positive: ['4:3', 0] } },
    '4:3': { class_type: 'CLIPTextEncode', inputs: { text: '{colors} and {missing}' } },
  };
}

test('the wildcard folder is listed, read, created in and renamed through Comfy user files', async () => {
  const api = userdata({ 'colors.txt': 'red\n' });
  const files = new ComfyWildcardFiles(api.fetchApi);
  expect((await files.list()).map((file) => [file.path, file.identifier, file.kind])).toEqual([
    ['colors.txt', 'colors', 'text'],
  ]);
  expect(await files.read('colors.txt')).toBe('red\n');
  expect(await files.read('absent.txt')).toBeNull();
  expect(await files.create('Sets/moods.txt', 'calm\n')).toBe('created');
  expect(await files.create('Sets/moods.txt', 'other')).toBe('exists');
  expect(await files.rename('Sets/moods.txt', 'Sets/feelings.txt')).toBe('renamed');
  expect(await files.rename('Sets/moods.txt', 'x.txt')).toBe('missing');
  expect(await files.rename('colors.txt', 'Sets/feelings.txt')).toBe('exists');
  expect(api.files).toEqual({ 'colors.txt': 'red\n', 'Sets/feelings.txt': 'calm\n' });
  expect(api.requests).toContain(
    'POST /userdata/sugar%2Fwildcards%2FSets%2Fmoods.txt?overwrite=false',
  );
  expect(api.requests).toContain(
    'POST /userdata/sugar%2Fwildcards%2FSets%2Fmoods.txt/move/sugar%2Fwildcards%2FSets%2Ffeelings.txt?overwrite=false',
  );
  // A folder nobody created yet is simply empty.
  expect(await new ComfyWildcardFiles(userdata({}).fetchApi).list()).toEqual([]);
});

test('the run seed is the first seed input in workflow stack order', () => {
  expect(queuedPromptSeed(graph(), output())).toBe('42');
  expect(queuedPromptSeed(graph(), { '9': { inputs: { value: 'text' } } })).toBeNull();
  expect(queuedPromptSeed(graph(), { '4:2': { inputs: { seed: [1, 0], noise_seed: 5 } } })).toBe(
    '5',
  );
});

test('a queued prompt and a directly run workflow both carry the resolved texts', async () => {
  const api = userdata({ 'colors.txt': 'red\ngreen\nblue\n' });
  const library = new WildcardLibrary(new ComfyWildcardFiles(api.fetchApi), jest.fn());
  const live = graph();
  const wildcards = new ComfyWildcardPrompts(
    () => ({ graph: live }),
    library,
    () => '1',
  );
  const payload = { output: output(), workflow: { nodes: [] }, client_id: 'c' };
  const queued = (await wildcards.prompt(payload)) as typeof payload;
  // Seed 42 picks the third of three candidates, as Python's random.Random(42).randrange(3) does.
  expect(queued.output['9'].inputs.value).toBe('a blue sky');
  expect(queued.output['4:3'].inputs.text).toBe('blue and {missing}');
  expect(queued.output['4:2']).toBe(payload.output['4:2']);
  expect(queued.client_id).toBe('c');
  // The prompt Comfy built, and so the workflow the person authored, keep their wildcards.
  expect(payload.output['9'].inputs.value).toBe('a {colors} sky');

  const serialized = {
    nodes: [{ id: 9, widgets_values: ['a {colors} sky'] }],
    definitions: {
      subgraphs: [
        {
          nodes: [
            {
              id: 3,
              widgets_values: ['{colors} and {missing}'],
              sugarcubes_widget_values: { text: '{colors} and {missing}' },
            },
          ],
        },
      ],
    },
  };
  expect(await wildcards.workflow(payload, serialized)).toEqual({
    nodes: [{ id: 9, widgets_values: ['a blue sky'] }],
    definitions: {
      subgraphs: [
        {
          nodes: [
            {
              id: 3,
              widgets_values: ['blue and {missing}'],
              sugarcubes_widget_values: { text: 'blue and {missing}' },
            },
          ],
        },
      ],
    },
  });
  expect(serialized.nodes[0]?.widgets_values).toEqual(['a {colors} sky']);

  // A run without wildcards is returned as it came and reads nothing.
  const before = api.fetchApi.mock.calls.length;
  const plain = { output: { '9': { inputs: { value: 'plain' } } } };
  expect(await wildcards.prompt(plain)).toBe(plain);
  expect(await wildcards.workflow(plain, serialized)).toBe(serialized);
  expect(api.fetchApi).toHaveBeenCalledTimes(before);
});
