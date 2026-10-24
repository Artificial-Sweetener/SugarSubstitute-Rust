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
/** Prove native presentation fields retain host identity without entering graph data or duplicating commits. */
import { jest } from '@jest/globals';
import type { ComfyNode, ComfyWidget } from '../../../frontend/comfyui/ui/types/graph.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';
import { NativeNodeChoiceFields } from '../../../frontend/comfyui/ui/surface/NativeNodeChoiceFields.js';
import {
  nativePresentationWidgets,
  withNativePresentationWidgets,
} from '../../../frontend/comfyui/ui/surface/NativePresentationWidgets.js';
import { cubeFacePresentedWidgets } from '../../../frontend/comfyui/ui/surface/CubeFaceWidgetPolicy.js';
import { captureNodeWidgetValues } from '../../../frontend/comfyui/ui/graph/WorkflowWidgetSnapshotCapture.js';

/** Keep only native widget construction/removal outside the real surface ownership under test. */
function fixture() {
  const original: ComfyWidget = { name: 'image', value: 'input.png', type: 'combo' };
  const removed = jest.fn();
  const registered: unknown[] = [];
  const node: ComfyNode = { widgets: [original] };
  node.addCustomWidget = (value: unknown): ComfyWidget => {
    if (!isRecord(value) || typeof value.name !== 'string')
      throw new Error('Invalid native field.');
    registered.push(value.options);
    const widget: ComfyWidget = { ...value, name: value.name, onRemove: removed };
    node.widgets?.push(widget);
    return widget;
  };
  return {
    node,
    original,
    removed,
    registered,
    fields: new NativeNodeChoiceFields(node, 'feature', document),
  };
}

test('renders native fields through the card widget scope while live graph values and identity stay unchanged', () => {
  const f = fixture(),
    graphWidgets = f.node.widgets;
  const mount = f.fields.mountChoice(document.createElement('article'), 'Source', jest.fn());
  mount.update('composition', [
    { value: 'composition', label: 'Current Composition', blockers: [] },
  ]);
  expect(f.node.widgets).toBe(graphWidgets);
  expect(f.node.widgets).toEqual([f.original]);
  expect(captureNodeWidgetValues(f.node)).toEqual({ image: 'input.png' });
  expect(cubeFacePresentedWidgets(f.node)).toHaveLength(2);
  const projected = withNativePresentationWidgets(f.node, () => [...(f.node.widgets ?? [])]);
  expect(projected[1]?.options).toBe(f.registered[0]);
  expect(projected[1]?.options?.values).toEqual(['composition']);
  expect(projected[1]).toMatchObject({
    type: 'combo',
    label: 'Source',
    value: 'composition',
    serialize: false,
  });
  expect(f.node.widgets).toBe(graphWidgets);
  expect(f.node.widgets).toEqual([f.original]);
  expect(() =>
    withNativePresentationWidgets(f.node, () => {
      throw new Error('Native extraction failed.');
    }),
  ).toThrow('Native extraction failed.');
  expect(f.node.widgets).toEqual([f.original]);
  mount.dispose();
  expect(nativePresentationWidgets(f.node)).toEqual([f.original]);
  expect(f.removed).not.toHaveBeenCalled();
});

test('shares native field identity across simultaneous views and commits through one active source owner', () => {
  const f = fixture(),
    first = jest.fn(),
    second = jest.fn();
  const one = f.fields.mountChoice(document.createElement('article'), 'Source', first);
  const two = f.fields.mountChoice(document.createElement('article'), 'Source', second);
  one.update('file', [{ value: 'file', label: 'Load From File', blockers: [] }]);
  const widget = nativePresentationWidgets(f.node)[1];
  expect(nativePresentationWidgets(f.node)).toHaveLength(2);
  widget?.callback?.('file');
  expect(first).toHaveBeenCalledTimes(1);
  expect(second).not.toHaveBeenCalled();
  one.dispose();
  widget?.callback?.('file');
  expect(second).toHaveBeenCalledTimes(1);
  expect(f.removed).not.toHaveBeenCalled();
  two.dispose();
  widget?.callback?.('file');
  expect(second).toHaveBeenCalledTimes(1);
  const again = f.fields.mountChoice(document.createElement('article'), 'Source', second);
  again.update('file', [{ value: 'file', label: 'Load From File', blockers: [] }]);
  expect(nativePresentationWidgets(f.node)[1]).toBe(widget);
  again.dispose();
  expect(f.removed).not.toHaveBeenCalled();
});
