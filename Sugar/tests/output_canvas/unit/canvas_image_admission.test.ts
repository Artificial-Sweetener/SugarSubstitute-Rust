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
/** Prove exact byte accounting and selection-scoped large-image consent. */

import {
  CanvasImageBudget,
  CANVAS_IMAGE_BYTE_LIMIT,
} from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageBudget.js';
import { CanvasImageAllowances } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageAllowances.js';
import { canvasViewId } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import { canvasImageBytes } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasImageResource.js';

const fourK = { width: 3840, height: 2160 };
const large = { width: 8192, height: 8192 };

describe('canvas image admission', () => {
  test('accounts duplicate selected identities as two decoded images rather than one', () => {
    const allowances = new CanvasImageAllowances();
    const view = canvasViewId('duplicate');
    allowances.grant(view, 1, [
      { key: 'a', dimensions: large },
      { key: 'a', dimensions: large },
    ]);
    expect(allowances.read(view, 1, ['a', 'a'])?.bytes).toBe(canvasImageBytes(large) * 2);
    expect(allowances.read(view, 1, ['a'])).toBeNull();
    allowances.dispose();
  });
  test('accounts outgoing and incoming 4K images against the same byte authority', () => {
    const budget = new CanvasImageBudget();
    const tickets = Array.from({ length: 4 }, () => budget.reserve(fourK, false));
    expect(tickets.every(Boolean)).toBe(true);
    expect(budget.estimatedBytes).toBe(canvasImageBytes(fourK) * 4);
    expect(budget.reserve(fourK, false)).toBeNull();
    for (const ticket of tickets)
      if (ticket) {
        budget.release(ticket);
        budget.release(ticket);
      }
    expect(budget.estimatedBytes).toBe(0);
  });

  test('allows one provisional unknown and rejects unexpectedly oversized pixels', () => {
    const budget = new CanvasImageBudget();
    const ticket = budget.reserve(null, false)!;
    expect(budget.reserve(null, false)).toBeNull();
    expect(budget.measure(ticket, large)).toBe(false);
    budget.release(ticket);
    expect(budget.estimatedBytes).toBe(0);
    expect(budget.unknownLoads).toBe(0);
  });

  test('ignores foreign reservations even when their diagnostic numbers collide', () => {
    const budget = new CanvasImageBudget();
    const other = new CanvasImageBudget();
    const ticket = budget.reserve(fourK, false)!;
    const foreign = other.reserve(fourK, false)!;
    expect(ticket.id).toBe(foreign.id);
    budget.release(foreign);
    expect(budget.measure(foreign, large)).toBe(false);
    expect(budget.estimatedBytes).toBe(canvasImageBytes(fourK));
    budget.dispose();
  });

  test('allows an explicitly approved unknown pair and disables thumbnails during large-image staging', () => {
    const budget = new CanvasImageBudget();
    budget.allowance(CANVAS_IMAGE_BYTE_LIMIT, 2);
    const a = budget.reserve(null, false)!;
    expect(budget.measure(a, large)).toBe(true);
    const b = budget.reserve(null, false)!;
    expect(budget.measure(b, large)).toBe(true);
    expect(budget.estimatedBytes).toBe(canvasImageBytes(large) * 2);
    expect(budget.reserve({ width: 1, height: 1 }, true)).toBeNull();
    budget.release(a);
    budget.release(b);
    expect(budget.estimatedBytes).toBe(0);
  });

  test('returns canceled unknown consent only to the same allowance epoch', () => {
    const budget = new CanvasImageBudget();
    budget.allowance(CANVAS_IMAGE_BYTE_LIMIT, 1);
    const canceled = budget.reserve(null, false)!;
    budget.release(canceled);
    const current = budget.reserve(null, false)!;
    expect(budget.measure(current, large)).toBe(true);
    budget.allowance(CANVAS_IMAGE_BYTE_LIMIT, 1);
    const obsolete = budget.reserve(null, false)!;
    budget.allowance(CANVAS_IMAGE_BYTE_LIMIT);
    budget.release(obsolete);
    expect(budget.reserve(null, false)).toBeNull();
    budget.dispose();
  });

  test('scopes consent to exact view, generation and selected identities through a pair swap', () => {
    const allowances = new CanvasImageAllowances();
    const view = canvasViewId('a');
    const size = { ...large };
    allowances.grant(view, 1, [
      { key: 'a', dimensions: size },
      { key: 'b', dimensions: null },
    ]);
    size.width = 1;
    const before = allowances.read(view, 1, ['b', 'a'])!;
    expect(before.bytes).toBe(canvasImageBytes(large));
    expect(before.unknownLoads).toBe(1);
    expect(allowances.read(view, 2, ['a', 'b'])).toBeNull();
    expect(allowances.read(canvasViewId('b'), 1, ['a', 'b'])).toBeNull();
    expect(allowances.read(view, 1, ['a', 'c'])).toBeNull();
    const measured = { ...fourK };
    allowances.measure(view, 1, 'b', measured);
    measured.width = 1;
    const after = allowances.read(view, 1, ['a', 'b'])!;
    expect(after.approvalId).toBe(before.approvalId);
    expect(after.bytes).toBe(canvasImageBytes(large) + canvasImageBytes(fourK));
    expect(allowances.dimensions(view, 1, 'b')).toEqual(fourK);
    allowances.measure(view, 1, 'b', large);
    expect(allowances.read(view, 1, ['a', 'b'])).toEqual(after);
    allowances.remove(view);
    expect(allowances.read(view, 1, ['a', 'b'])).toBeNull();
    allowances.dispose();
  });

  test.each([
    { width: 0, height: 1 },
    { width: 1.5, height: 1 },
    { width: Infinity, height: 1 },
  ])('rejects invalid decoded geometry %o', (dimensions) => {
    expect(() => canvasImageBytes(dimensions)).toThrow(RangeError);
  });
});
