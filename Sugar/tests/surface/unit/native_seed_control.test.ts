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
/** Prove seed detection and mode edits retain canonical Comfy widget ownership. */
import { expect, jest, test } from '@jest/globals';
import { ComfyNativeSeedControlAdapter } from '../../../frontend/comfyui/ui/surface/appearance/ComfyNativeSeedControl.js';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';

/** Create a renamed integer editor linked to Comfy's native mode control. */
function fixture(name = 'generation_number') {
  const callback = jest.fn();
  const changed = jest.fn();
  const afterChange = jest.fn();
  const control = {
    name: 'control_after_generate',
    type: 'combo',
    value: 'randomize',
    callback,
    options: {
      canvasOnly: true,
      serialize: false,
      values: ['fixed', 'increment', 'decrement', 'randomize'],
    },
  };
  const widget: UnknownRecord = { name, type: 'number', value: 123, linkedWidgets: [control] };
  const node: UnknownRecord = {
    id: 1,
    widgets: [widget, control],
    onWidgetChanged: changed,
    graph: { afterChange },
  };
  const graph = { id: 'root', _nodes: [node] };
  const adapter = new ComfyNativeSeedControlAdapter({ graph });
  adapter.beginBatch();
  const safeWidget = { name, nodeLocatorId: 'root:1' };
  return { adapter, graph, node, control, widget, safeWidget, callback, changed, afterChange };
}

test('detects renamed integers by their host mode relationship and changes only that mode', () => {
  const f = fixture();
  const control = f.adapter.read(f.safeWidget, 0);
  if (!control) throw new Error('Seed control missing');
  expect(control.modes).toEqual(['fixed', 'increment', 'decrement', 'randomize']);
  for (const mode of control.modes) {
    control.writeMode(mode);
    expect(control.readMode()).toBe(mode);
    expect(f.control.value).toBe(mode);
    expect(f.widget.value).toBe(123);
  }
  expect(f.callback).toHaveBeenCalledTimes(4);
  expect(f.changed).toHaveBeenLastCalledWith(
    'control_after_generate',
    'randomize',
    'decrement',
    f.control,
  );
  expect(f.afterChange).toHaveBeenCalledTimes(4);
  control.writeMode('randomize');
  expect(f.callback).toHaveBeenCalledTimes(4);
  f.adapter.endBatch();
  expect(f.adapter.read(f.safeWidget, 0)).toBeNull();
});

test('rejects float/custom/unknown modes and explicit schema opt-outs', () => {
  const f = fixture('seed');
  expect(f.adapter.read(f.safeWidget, 2)).toBeNull();
  expect(
    f.adapter.read({ ...f.safeWidget, spec: { control_after_generate: false } }, 0),
  ).toBeNull();
  f.control.options.values.push('something else');
  expect(f.adapter.read(f.safeWidget, 0)).toBeNull();
  expect(f.adapter.read({ name: 'seed', nodeLocatorId: 'missing:1' }, 0)).toBeNull();
});

test('uses exact seed fallback only with a real adjacent native mode control', () => {
  const f = fixture('seed');
  delete f.widget.linkedWidgets;
  expect(f.adapter.read(f.safeWidget, 0)).not.toBeNull();
  f.widget.name = 'seedling';
  expect(f.adapter.read({ ...f.safeWidget, name: 'seedling' }, 0)).toBeNull();
  f.node.widgets = [f.widget];
  expect(
    f.adapter.read(
      { ...f.safeWidget, name: 'seedling', spec: { control_after_generate: true } },
      0,
    ),
  ).toBeNull();
});

test('resolves a promoted seed to its internal mode without changing the exposed value', () => {
  const f = fixture('seed');
  const promoted: UnknownRecord = { name: 'Seed Input', value: 123 };
  const subgraph = {
    id: 'inner',
    _nodes: [f.node],
    inputNode: { slots: [{ name: 'Seed Input', linkIds: [9] }] },
    getLink: () => ({ target_id: 1, target_slot: 0 }),
  };
  f.node.inputs = [{ widget: { name: 'seed' } }];
  const cube = { id: 'cube', widgets: [promoted], subgraph };
  const adapter = new ComfyNativeSeedControlAdapter({ graph: { id: 'root', _nodes: [cube] } });
  adapter.beginBatch();
  const control = adapter.read({ name: 'Seed Input', nodeLocatorId: 'root:cube' }, 0);
  expect(control).not.toBeNull();
  control?.writeMode('fixed');
  expect(f.control.value).toBe('fixed');
  expect(promoted.value).toBe(123);
  expect(f.widget.value).toBe(123);
  expect(adapter.read({ name: 'seed', nodeLocatorId: 'inner:1' }, 0)).not.toBeNull();
});

test('indexes root subgraphs and rejects cycles and missing promoted widget links', () => {
  const f = fixture('seed');
  const cube: UnknownRecord = { id: 'cube', widgets: [{ name: 'seed' }] };
  const subgraph: UnknownRecord = {
    id: 'inner',
    _nodes: [f.node, cube],
    inputNode: { slots: [{ name: 'seed', linkIds: [9] }] },
    getLink: () => ({ target_id: 'cube', target_slot: 0 }),
  };
  cube.subgraph = subgraph;
  cube.inputs = [{ widget: { name: 'seed' } }];
  const root = { id: 'root', _nodes: [], _subgraphs: new Map([['inner', subgraph]]) };
  subgraph.rootGraph = root;
  const adapter = new ComfyNativeSeedControlAdapter({ graph: root });
  adapter.beginBatch();
  expect(adapter.read({ name: 'seed', nodeLocatorId: 'inner:1' }, 0)).not.toBeNull();
  expect(adapter.read({ name: 'seed', nodeLocatorId: 'inner:cube' }, 0)).toBeNull();
});
