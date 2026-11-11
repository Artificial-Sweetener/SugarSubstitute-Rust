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
/** Prove the canvas shortcut map: the reference keys only, gated by the owning surface. */
import { describe, expect, test } from '@jest/globals';
import {
  canvasEditKeyIntent,
  type CanvasEditKey,
} from '../../../frontend/comfyui/ui/canvas_editing/domain/CanvasEditKeys.js';
import type { CanvasEditSurface } from '../../../frontend/comfyui/ui/canvas_editing/domain/CanvasEditSurface.js';
import { editState } from '../support/CanvasEditHostFixture.js';

function key(name: string, options: Partial<CanvasEditKey> = {}): CanvasEditKey {
  return {
    phase: 'down',
    key: name,
    ctrl: false,
    meta: false,
    shift: false,
    alt: false,
    ...options,
  };
}

const none: CanvasEditSurface = { kind: 'none' };
const command = (name: string, options: Partial<CanvasEditKey> = {}, state = editState()) => {
  const intent = canvasEditKeyIntent(key(name, options), state, none);
  return intent?.kind === 'command' ? intent.command : intent;
};

describe('canvas editing shortcuts', () => {
  test('holds and releases temporary navigation with Space', () => {
    expect(command(' ')).toEqual({ type: 'navigation', held: true });
    expect(command(' ', { phase: 'up' })).toEqual({ type: 'navigation', held: false });
    expect(command(' ', { ctrl: true })).toBeNull();
  });

  test('walks history with Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y', () => {
    expect(command('z', { ctrl: true })).toEqual({ type: 'history', action: 'undo' });
    expect(command('Z', { ctrl: true, shift: true })).toEqual({ type: 'history', action: 'redo' });
    expect(command('y', { ctrl: true })).toEqual({ type: 'history', action: 'redo' });
    expect(command('z', { meta: true })).toEqual({ type: 'history', action: 'undo' });
    expect(command('z', { ctrl: true, alt: true })).toBeNull();
    expect(command('z', { ctrl: true, phase: 'up' })).toBeNull();
  });

  test('commits with Enter, cancels with Esc and clears a selection with Delete', () => {
    expect(command('Enter')).toEqual({ type: 'commit' });
    expect(command('Escape')).toEqual({ type: 'cancel' });
    expect(command('Delete')).toBeNull();
    const selected = editState({ selection: { bounds: { left: 0, top: 0, width: 4, height: 4 } } });
    expect(command('Delete', {}, selected)).toEqual({ type: 'selection', action: 'clear' });
  });

  test('nudges by one pixel, or ten with Shift, only while moving or transforming', () => {
    expect(command('ArrowLeft')).toBeNull();
    const moving = editState({ tool: 'move' });
    expect(command('ArrowLeft', {}, moving)).toEqual({ type: 'nudge', dx: -1, dy: 0 });
    expect(command('ArrowDown', { shift: true }, moving)).toEqual({ type: 'nudge', dx: 0, dy: 10 });
    const transforming = editState({ session: { kind: 'transform', undo: false, redo: false } });
    expect(command('ArrowUp', {}, transforming)).toEqual({ type: 'nudge', dx: 0, dy: -1 });
  });

  test('defines no tool hotkeys or bracket keys', () => {
    for (const name of ['b', 'e', 'm', 'v', '[', ']', 'l'])
      expect(canvasEditKeyIntent(key(name), editState(), none)).toBeNull();
  });

  test('admits only Esc in the coverage editor and Enter, Esc and Space while modifying', () => {
    const state = editState({ mode: 'coverage' });
    expect(canvasEditKeyIntent(key('Escape'), state, { kind: 'coverage' })).toEqual({
      kind: 'close-coverage',
    });
    for (const name of [' ', 'Enter', 'Delete', 'z'])
      expect(
        canvasEditKeyIntent(key(name, { ctrl: name === 'z' }), state, { kind: 'coverage' }),
      ).toBeNull();
    const modify: CanvasEditSurface = { kind: 'modify' };
    expect(canvasEditKeyIntent(key('Escape'), editState(), modify)).toEqual({
      kind: 'cancel-modify',
    });
    expect(canvasEditKeyIntent(key('Enter'), editState(), modify)).toEqual({
      kind: 'apply-modify',
    });
    expect(canvasEditKeyIntent(key('z', { ctrl: true }), editState(), modify)).toBeNull();
    expect(canvasEditKeyIntent(key(' '), editState(), modify)).toEqual({
      kind: 'command',
      command: { type: 'navigation', held: true },
    });
  });
});
