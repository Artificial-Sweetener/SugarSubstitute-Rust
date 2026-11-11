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
/** Prove numeric constraints and field attribution retain exact model ownership through projected names. */
import {
  nativeDimensionLimits,
  nativeDimensionTargets,
} from '../../../frontend/comfyui/ui/editor_actions/infrastructure/NativeDimensionFieldMetadata.js';
import { projectNativeWidgetEdits } from '../../../frontend/comfyui/ui/surface/NativeProjectedWidgetEdits.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';

test('uses native step2 rather than the LiteGraph tenfold drag step', () => {
  expect(nativeDimensionLimits({ options: { min: 16, max: 16384, step: 80, step2: 8 } })).toEqual({
    minimum: 16,
    maximum: 16384,
    multiple: 8,
  });
  expect(nativeDimensionLimits({ spec: ['INT', { min: 8, max: 2048, step: 8 }] })).toEqual({
    minimum: 8,
    maximum: 2048,
    multiple: 8,
  });
});

test('maps forwarded aliases to their actual nested owner without guessing from reused node IDs', () => {
  const source: ComfyNode = { id: 1, widgets: [{ name: 'width', value: 512 }] };
  const other: ComfyNode = { id: 1, widgets: [{ name: 'width', value: 1024 }] };
  const graph = {
    _nodes: [
      { id: 10, subgraph: { _nodes: [other] } },
      { id: 20, subgraph: { _nodes: [source] } },
    ],
  };
  const widgets = projectNativeWidgetEdits(source, [{ name: 'width' }]);
  if (!Array.isArray(widgets) || !isRecord(widgets[0])) throw new Error('Missing projection');
  const renamed = { ...widgets[0], name: 'output_width' };
  expect(nativeDimensionTargets(renamed, graph)).toEqual([{ nodeId: '20:1', inputName: 'width' }]);
  expect(nativeDimensionTargets(renamed, { _nodes: [other] })).toEqual([]);
  expect(nativeDimensionTargets({ name: 'width' }, graph)).toEqual([]);
});
