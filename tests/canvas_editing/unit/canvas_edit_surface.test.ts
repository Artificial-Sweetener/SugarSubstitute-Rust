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
/** Prove which contextual surface owns the canvas, in the reference precedence. */
import { describe, expect, test } from '@jest/globals';
import {
  canvasEditChromeSuppressed,
  canvasEditSurface,
} from '../../../frontend/comfyui/ui/canvas_editing/domain/CanvasEditSurface.js';
import { editState } from '../support/CanvasEditHostFixture.js';

const selection = { bounds: { left: 10, top: 10, width: 50, height: 40 } };

describe('canvas editing surface', () => {
  test('shows selection actions only with a selection', () => {
    expect(canvasEditSurface(editState(), false)).toEqual({ kind: 'none' });
    expect(canvasEditSurface(editState({ selection }), false)).toEqual({ kind: 'selection' });
  });

  test('ranks coverage, modification, sessions and transform above selection actions', () => {
    const transform = { kind: 'transform' as const, undo: false, redo: false };
    expect(canvasEditSurface(editState({ selection, mode: 'coverage' }), true)).toEqual({
      kind: 'coverage',
    });
    expect(canvasEditSurface(editState({ selection }), true)).toEqual({ kind: 'modify' });
    expect(
      canvasEditSurface(
        editState({ session: { kind: 'modify', undo: false, redo: false } }),
        false,
      ),
    ).toEqual({ kind: 'modify' });
    expect(
      canvasEditSurface(
        editState({ selection, session: { kind: 'move', undo: true, redo: false } }),
        false,
      ),
    ).toEqual({ kind: 'session', tool: 'move', armed: false });
    expect(canvasEditSurface(editState({ tool: 'shared-edges' }), false)).toEqual({
      kind: 'session',
      tool: 'shared-edges',
      armed: true,
    });
    expect(canvasEditSurface(editState({ selection, session: transform }), false)).toEqual({
      kind: 'transform',
      target: 'selection',
    });
    expect(canvasEditSurface(editState({ session: transform }), false)).toEqual({
      kind: 'transform',
      target: 'layer',
    });
  });

  test('hides the rail and options only while an exclusive editor owns input', () => {
    expect(canvasEditChromeSuppressed({ kind: 'coverage' })).toBe(true);
    expect(canvasEditChromeSuppressed({ kind: 'modify' })).toBe(true);
    expect(canvasEditChromeSuppressed({ kind: 'transform', target: 'layer' })).toBe(true);
    expect(canvasEditChromeSuppressed({ kind: 'selection' })).toBe(false);
    expect(
      canvasEditChromeSuppressed({ kind: 'session', tool: 'shared-edges', armed: false }),
    ).toBe(false);
    expect(canvasEditChromeSuppressed({ kind: 'none' })).toBe(false);
  });
});
