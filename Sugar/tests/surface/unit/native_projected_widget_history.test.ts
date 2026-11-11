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
/** Prove copied native controls commit to current objects through native undo boundaries. */
import { expect, jest, test } from '@jest/globals';
import { projectNativeWidgetEdits } from '../../../frontend/comfyui/ui/surface/NativeProjectedWidgetEdits.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';
import { isRecord } from '../../../frontend/comfyui/ui/types/common.js';

/** Narrow the projected host callback before invoking its public edit surface. */
function edit(node: ComfyNode, value: unknown, callback: (value: unknown) => void): void {
  const projected = projectNativeWidgetEdits(node, [{ name: 'steps', callback }]);
  if (
    !Array.isArray(projected) ||
    !isRecord(projected[0]) ||
    typeof projected[0].callback !== 'function'
  )
    throw new Error('Missing projected widget callback');
  projected[0].callback(value);
}

test('captures nested widget changes through the root history owner before callback and final serialization', () => {
  const widget = { name: 'steps', value: 20 };
  const snapshots: number[] = [];
  const beforeChange = jest.fn(() => {
    snapshots.push(widget.value);
  });
  const afterChange = jest.fn(() => {
    snapshots.push(widget.value);
  });
  const rootGraph = { beforeChange, afterChange };
  const node: ComfyNode = { widgets: [widget], rootGraph, graph: { beforeChange: jest.fn() } };
  const callback = jest.fn(() => {
    expect(widget.value).toBe(21);
  });
  edit(node, 21, callback);
  expect(snapshots).toEqual([20, 21]);
  expect(callback).toHaveBeenCalledWith(21);
  expect(beforeChange).toHaveBeenCalledTimes(1);
  expect(afterChange).toHaveBeenCalledTimes(1);
  const restored = { name: 'steps', value: 20 };
  node.widgets = [restored];
  edit(node, 22, jest.fn());
  expect(restored.value).toBe(22);
  expect(widget.value).toBe(21);
});

test('balances native transactions when the host widget callback fails', () => {
  const failure = new Error('host callback failed');
  const afterChange = jest.fn();
  const node: ComfyNode = {
    widgets: [{ name: 'steps', value: 20 }],
    graph: { beforeChange: jest.fn(), afterChange },
  };
  expect(() =>
    edit(node, 21, () => {
      throw failure;
    }),
  ).toThrow(failure);
  expect(afterChange).toHaveBeenCalledTimes(1);
});

test('leaves unknown host-only projections intact', () => {
  const widget = { name: 'external', callback: jest.fn() };
  expect(projectNativeWidgetEdits({}, [widget])).toEqual([widget]);
  expect(projectNativeWidgetEdits({}, null)).toBeNull();
});
