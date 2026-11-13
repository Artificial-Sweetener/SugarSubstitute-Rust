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
/** Preserve seed modes in saved real Cube definitions during native preconfiguration. */
import { expect, test } from '@jest/globals';
import { CubeSerializedWidgetRehydrator } from '../../../frontend/comfyui/ui/cube/CubeSerializedWidgetRehydrator.js';

test.each(['fixed', 'increment', 'decrement', 'randomize'])(
  'restores %s alongside the saved seed and other sampler settings',
  (mode) => {
    const sampler = {
      id: 1,
      type: 'KSampler',
      inputs: ['seed', 'steps'].map((name) => ({ name, widget: { name } })),
      widgets_values: [709312264123456, mode, 28],
    };
    const workflow = {
      definitions: {
        subgraphs: [{ id: 'cube-graph', nodes: [sampler], extra: { sugarcubes_kind: 'cube' } }],
      },
    };
    const rehydrator = new CubeSerializedWidgetRehydrator(() => ({
      widgets: [
        { name: 'seed', value: 0 },
        {
          name: 'control_after_generate',
          type: 'combo',
          value: 'randomize',
          options: { serialize: false, values: ['fixed', 'increment', 'decrement', 'randomize'] },
        },
        { name: 'steps', value: 20 },
      ],
    }));
    expect(rehydrator.prepare(workflow)).toBe(1);
    expect(sampler.widgets_values).toEqual([709312264123456, mode, 28]);
    expect(rehydrator.prepare(workflow)).toBe(1);
    expect(sampler.widgets_values).toEqual([709312264123456, mode, 28]);
  },
);

test('discards unsupported saved seed modes while retaining adjacent numeric values', () => {
  const sampler = {
    id: 1,
    type: 'KSampler',
    inputs: ['seed', 'steps'].map((name) => ({ name, widget: { name } })),
    widgets_values: [123, 'unknown mode', 28],
  };
  const rehydrator = new CubeSerializedWidgetRehydrator(() => ({
    widgets: [
      { name: 'seed', value: 0 },
      {
        name: 'control_after_generate',
        type: 'combo',
        value: 'randomize',
        options: { serialize: false, values: ['fixed', 'randomize'] },
      },
      { name: 'steps', value: 20 },
    ],
  }));
  rehydrator.prepare({
    definitions: {
      subgraphs: [{ id: 'cube-graph', nodes: [sampler], extra: { sugarcubes_kind: 'cube' } }],
    },
  });
  expect(sampler.widgets_values).toEqual([123, 'randomize', 28]);
});
