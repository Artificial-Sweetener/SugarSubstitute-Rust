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
/** Prove the rail keeps the reference order and resolves each group's presented member. */
import { describe, expect, test } from '@jest/globals';
import {
  canvasEditSlotOf,
  canvasEditSlotViews,
  type CanvasEditSlotId,
} from '../../../frontend/comfyui/ui/canvas_editing/domain/CanvasEditTools.js';
import type { CanvasToolId } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { editState } from '../support/CanvasEditHostFixture.js';

const none = new Map<CanvasEditSlotId, CanvasToolId>();

describe('canvas tool rail slots', () => {
  test('orders navigation, move, brush, eraser, selection, mask shapes and transform', () => {
    const views = canvasEditSlotViews(editState(), none);
    expect(views.map((view) => [view.id, view.members.map((member) => member.id)])).toEqual([
      ['navigation', ['pan-zoom']],
      ['move', ['move']],
      ['brush', ['brush']],
      ['eraser', ['eraser']],
      ['selection', ['select-rectangle', 'select-ellipse', 'select-lasso']],
      ['mask-shapes', ['mask-rectangle', 'mask-ellipse', 'mask-lasso']],
      ['transform', ['transform', 'shared-edges']],
    ]);
    expect(views.filter((view) => view.grouped).map((view) => view.id)).toEqual([
      'selection',
      'mask-shapes',
      'transform',
    ]);
  });

  test('presents the active member, else the remembered one, else the first', () => {
    const remembered = new Map<CanvasEditSlotId, CanvasToolId>([['selection', 'select-lasso']]);
    const first = canvasEditSlotViews(editState(), none);
    expect(first.find((view) => view.id === 'selection')?.current.id).toBe('select-rectangle');
    const recalled = canvasEditSlotViews(editState(), remembered);
    expect(recalled.find((view) => view.id === 'selection')?.current).toEqual({
      id: 'select-lasso',
      available: true,
      active: false,
    });
    const active = canvasEditSlotViews(editState({ tool: 'select-ellipse' }), remembered);
    expect(active.find((view) => view.id === 'selection')?.current).toEqual({
      id: 'select-ellipse',
      available: true,
      active: true,
    });
  });

  test('disables unavailable tools and omits tools the canvas does not offer', () => {
    const state = editState({
      tools: [
        { id: 'pan-zoom', available: true },
        { id: 'brush', available: false },
        { id: 'mask-rectangle', available: false },
        { id: 'mask-lasso', available: false },
      ],
    });
    const views = canvasEditSlotViews(state, none);
    expect(views.map((view) => view.id)).toEqual(['navigation', 'brush', 'mask-shapes']);
    expect(views.find((view) => view.id === 'brush')?.current.available).toBe(false);
    expect(views.find((view) => view.id === 'mask-shapes')?.members.map(({ id }) => id)).toEqual([
      'mask-rectangle',
      'mask-lasso',
    ]);
  });

  test('maps each tool to its owning slot', () => {
    expect(canvasEditSlotOf('select-lasso')).toBe('selection');
    expect(canvasEditSlotOf('shared-edges')).toBe('transform');
    expect(canvasEditSlotOf('pan-zoom')).toBe('navigation');
  });
});
