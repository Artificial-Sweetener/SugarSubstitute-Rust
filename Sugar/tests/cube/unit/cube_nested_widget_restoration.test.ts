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
/** Prove nested instance values survive host migration without changing shared defaults. */
import { expect, test } from '@jest/globals';
import { CubeNestedWidgetRestoration } from '../../../frontend/comfyui/ui/cube/CubeNestedWidgetRestoration.js';
import { CubeSerializedWidgetRehydrator } from '../../../frontend/comfyui/ui/cube/CubeSerializedWidgetRehydrator.js';

test('preserves nested values while their host definitions are unavailable during graph teardown', () => {
  const node = {
    id: 'one',
    type: 'nested',
    inputs: [{ widget: { name: 'value' } }],
    widgets_values: [1.8],
  };
  const workflow = {
    definitions: {
      subgraphs: [
        { id: 'cube', extra: { sugarcubes_kind: 'cube' }, nodes: [node] },
        { id: 'nested', nodes: [] },
      ],
    },
  };
  const rehydrator = new CubeSerializedWidgetRehydrator(() => ({ widgets: [] }));
  expect(rehydrator.prepare(workflow)).toBe(2);
  expect(node.widgets_values).toEqual([1.8]);
  const live = { id: 'one', widgets: [{ name: 'value', value: 1.5 }] };
  rehydrator.restore({ subgraphs: new Map([['cube', { _nodes: [live] }]]) });
  expect(live.widgets[0]?.value).toBe(1.8);
});

test('restores each promoted instance by identity once and keeps other graphs unchanged', () => {
  const restoration = new CubeNestedWidgetRestoration();
  const first = { id: 'one', widgets: [{ name: 'value', value: 1.5 }] };
  const second = { id: 'two', widgets: [{ name: 'value', value: 1.5 }] };
  const inner = { id: 'default', widgets: [{ name: 'value', value: 1.5 }] };
  restoration.capture(
    {
      id: 'cube',
      nodes: [
        {
          id: 'one',
          type: 'nested',
          inputs: [{ widget: { name: 'value' } }],
          widgets_values: [1.8],
        },
        {
          id: 'two',
          type: 'nested',
          inputs: [{ widget: { name: 'value' } }],
          widgets_values: [2.1],
        },
        { id: 'ordinary', type: 'other', widgets_values: [99] },
      ],
    },
    new Set(['nested']),
    () => null,
  );
  const graph = {
    subgraphs: new Map([
      ['cube', { _nodes: [first, second] }],
      ['nested', { _nodes: [inner] }],
    ]),
  };
  restoration.restore(graph);
  expect(first.widgets[0]?.value).toBe(1.8);
  expect(second.widgets[0]?.value).toBe(2.1);
  expect(inner.widgets[0]?.value).toBe(1.5);
  first.widgets[0]!.value = 3;
  restoration.restore(graph);
  expect(first.widgets[0]?.value).toBe(3);
});

test('drops unfinished and unavailable hydration without applying stale workflow values', () => {
  const restoration = new CubeNestedWidgetRestoration();
  const definition = {
    id: 'cube',
    nodes: [{ id: 'one', type: 'nested', widgets_values: [1.8] }],
  };
  restoration.capture(definition, new Set(['nested']), () => null);
  restoration.capture(definition, new Set(['nested']), () => ({ widgets: [] }));
  restoration.restore(null);
  const node = { id: 'one', widgets: [{ name: 'value', value: 1.5 }] };
  restoration.capture(definition, new Set(['nested']), () => node);
  restoration.reset();
  restoration.restore({ _subgraphs: new Map([['cube', { _nodes: [node] }]]) });
  expect(node.widgets[0]?.value).toBe(1.5);
});
