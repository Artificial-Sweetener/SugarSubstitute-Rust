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
/** Prove reference drag thresholds use committed centers and reversible transient permutations. */
import { expect, test } from '@jest/globals';
import { workflowTabDragPlacement } from '../../../frontend/comfyui/ui/surface/frame/WorkflowTabDragGeometry.js';

test('held tab follows the grab offset while neighbors reserve the held-tab-selected slot', () => {
  expect(workflowTabDragPlacement(3, 0, 180)).toEqual({
    target: 1,
    draggedLeft: 180,
    offsets: [180, -180, 0],
  });
  expect(workflowTabDragPlacement(3, 2, -320)).toEqual({
    target: 0,
    draggedLeft: 40,
    offsets: [180, 180, -320],
  });
});

test.each([
  [-200, 0],
  [95, 0],
  [96, 1],
  [275, 1],
  [276, 2],
  [999, 2],
])('pointer %s resolves stable slot %s', (pointer, target) => {
  expect(workflowTabDragPlacement(3, 0, pointer).target).toBe(target);
});

test('clamping never lets a tab escape occupied slots and returning the pointer restores order', () => {
  expect(workflowTabDragPlacement(3, 0, -400).draggedLeft).toBe(0);
  expect(workflowTabDragPlacement(3, 0, 999).draggedLeft).toBe(360);
  expect(workflowTabDragPlacement(3, 1, 0).offsets).toEqual([0, 0, 0]);
  expect(workflowTabDragPlacement(1, 0, 999)).toEqual({
    target: 0,
    draggedLeft: 0,
    offsets: [0],
  });
});

test('boundary jitter retains the preview until the held tab deliberately crosses the dead band', () => {
  expect(workflowTabDragPlacement(3, 0, 97, 0).target).toBe(1);
  expect(workflowTabDragPlacement(3, 0, 89, 1).target).toBe(1);
  expect(workflowTabDragPlacement(3, 0, 85, 1).target).toBe(1);
  expect(workflowTabDragPlacement(3, 0, 84, 1).target).toBe(0);
});

test('condensed tabs displace neighbors and resolve thresholds at their actual width', () => {
  expect(workflowTabDragPlacement(3, 0, 120, 0, 120)).toEqual({
    target: 1,
    draggedLeft: 120,
    offsets: [120, -120, 0],
  });
  expect(workflowTabDragPlacement(3, 0, 65, 0, 120).target).toBe(0);
  expect(workflowTabDragPlacement(3, 0, 66, 0, 120).target).toBe(1);
});
