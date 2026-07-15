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
import { describe, expect, test } from '@jest/globals';
import {
  snapshotSurfaceInstance,
  snapshotSurfaceValues,
} from '../../web/comfyui/ui/graph/SurfaceSnapshotter.js';

function makeSurface() {
  return {
    controls: [
      { control_id: 'node.brightness', symbol: 'node', input_name: 'brightness' },
      { control_id: 'node.seed', symbol: 'node', input_name: 'seed' },
      { control_id: 'node.r', symbol: 'node', input_name: 'r' },
      { control_id: 'node.g', symbol: 'node', input_name: 'g' },
      { control_id: 'node.b', symbol: 'node', input_name: 'b' },
    ],
  };
}

describe('surface snapshotter', () => {
  test('snapshotSurfaceValues preserves surface control order', () => {
    const snapshot = snapshotSurfaceValues(makeSurface(), {
      'node.b': 4,
      'node.brightness': 1,
      'node.g': 3,
      'node.r': 2,
      'node.seed': 999,
    });

    expect(snapshot.controls.map((control) => control.control_id)).toEqual([
      'node.brightness',
      'node.r',
      'node.g',
      'node.b',
    ]);
    expect(snapshot.controls.map((control) => control.value)).toEqual([1, 2, 3, 4]);
  });

  test('snapshotSurfaceInstance preserves tracked surface control order', () => {
    const graph = {
      _nodes: [
        {
          id: 17,
          properties: { sugarcubes_symbol: 'node' },
          widgets: [
            { name: 'b', value: 4 },
            { name: 'brightness', value: 1 },
            { name: 'g', value: 3 },
            { name: 'r', value: 2 },
            { name: 'seed', value: 999 },
          ],
        },
      ],
    };

    const snapshot = snapshotSurfaceInstance(graph, ['17'], makeSurface());

    expect(snapshot.controls.map((control) => control.control_id)).toEqual([
      'node.brightness',
      'node.r',
      'node.g',
      'node.b',
    ]);
    expect(snapshot.controls.map((control) => control.value)).toEqual([1, 2, 3, 4]);
  });

  test('snapshotSurfaceInstance resolves missing symbols from managed node ids', () => {
    const graph = {
      _nodes: [
        {
          id: 2147,
          type: 'SimpleSyrup.DetailSEGSByScaleFactorTiledDiffusion',
          properties: { 'Node name for S&R': 'SimpleSyrup.DetailSEGSByScaleFactorTiledDiffusion' },
          inputs: [
            { name: 'scale_factor', widget: { name: 'scale_factor' } },
            { name: 'cfg', widget: { name: 'cfg' } },
          ],
          widgets: [
            { name: 'scale_factor', value: 1.5 },
            { name: 'cfg', value: 6 },
          ],
        },
      ],
    };
    const surface = {
      controls: [
        {
          control_id: 'detailer.scale_factor',
          symbol: 'detailer',
          input_name: 'scale_factor',
          class_type: 'SimpleSyrup.DetailSEGSByScaleFactorTiledDiffusion',
        },
        {
          control_id: 'detailer.cfg',
          symbol: 'detailer',
          input_name: 'cfg',
          class_type: 'SimpleSyrup.DetailSEGSByScaleFactorTiledDiffusion',
        },
      ],
    };

    const snapshot = snapshotSurfaceInstance(graph, ['2147'], surface);

    expect(snapshot.controls).toEqual([
      { control_id: 'detailer.scale_factor', value: 1.5 },
      { control_id: 'detailer.cfg', value: 6 },
    ]);
  });
});
