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

/** Prove Save Defaults captures a nested wrapper through its surface identity. */

import { describe, expect, jest, test } from '@jest/globals';
import { FlavorService } from '../../../frontend/comfyui/ui/flavors/FlavorService.js';
import type { FlavorMetadata } from '../../../frontend/comfyui/ui/flavors/FlavorService.js';

const WRAPPER_ID = 'ff5e3223-5b47-497a-944e-4c26c8c55545';

describe('nested wrapper default save', () => {
  test('submits every portable wrapper value and omits volatile seed state', async () => {
    const metadata = {
      managed: true,
      instance_id: 'instance-1',
      cube_id: 'local/personal/Krea2/Text to Image.cube',
      nodes: ['1'],
      surface: {
        default_flavor_id: 'default',
        controls: [
          control('width'),
          control('height'),
          control('seed'),
          control('steps'),
          control('cfg'),
          control('sampler_name'),
          control('scheduler'),
          control('batch_size'),
        ],
      },
      authored_flavors: [{ id: 'default', name: 'Default', values: {} }],
    } as FlavorMetadata;
    const graph = {
      _nodes: [
        {
          id: 1,
          type: WRAPPER_ID,
          properties: { sugarcubes_symbol: 'ksampler' },
          widgets: [
            widget('width', 1216),
            widget('height', 1536),
            widget('seed', 123456),
            widget('steps', 8),
            widget('cfg', 1),
            widget('sampler_name', 'euler'),
            widget('scheduler', 'normal'),
            widget('batch_size', 1),
          ],
        },
      ],
      _groups: [{ properties: { sugarcubes: metadata } }],
      setDirtyCanvas: jest.fn(),
    };
    const api = {
      saveAuthoredFlavor: jest.fn(async (_payload: string) => ({
        response: { ok: true },
        data: { saved: { flavor_id: 'default' } },
      })),
    };
    const service = new FlavorService({
      adapter: { getApp: () => ({ graph, canvas: { setDirty: jest.fn() } }) },
      api,
    });

    expect(await service.saveAuthoredFlavor(metadata, { flavorId: 'default' })).toBe(true);

    const request = JSON.parse(api.saveAuthoredFlavor.mock.calls[0]![0]) as {
      values: Record<string, unknown>;
    };
    expect(request.values).toEqual({
      'ksampler.width': 1216,
      'ksampler.height': 1536,
      'ksampler.steps': 8,
      'ksampler.cfg': 1,
      'ksampler.sampler_name': 'euler',
      'ksampler.scheduler': 'normal',
      'ksampler.batch_size': 1,
    });
  });
});

function control(inputName: string) {
  return {
    control_id: `ksampler.${inputName}`,
    symbol: 'ksampler',
    input_name: inputName,
    class_type: WRAPPER_ID,
  };
}

function widget(name: string, value: unknown) {
  return { name, value };
}
