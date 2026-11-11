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
/** Translate canvas keyboard shortcuts into editing intents without touching browser events. */
import type {
  CanvasEditCommand,
  CanvasEditState,
} from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import type { CanvasEditSurface } from './CanvasEditSurface.js';

/** The parts of a keyboard event that select a shortcut. */
export interface CanvasEditKey {
  readonly phase: 'down' | 'up';
  readonly key: string;
  readonly ctrl: boolean;
  readonly meta: boolean;
  readonly shift: boolean;
  readonly alt: boolean;
}

export type CanvasEditKeyIntent =
  | { readonly kind: 'command'; readonly command: CanvasEditCommand }
  | { readonly kind: 'close-coverage' }
  | { readonly kind: 'apply-modify' }
  | { readonly kind: 'cancel-modify' };

const NUDGES: Readonly<Record<string, readonly [number, number]>> = {
  ArrowLeft: [-1, 0],
  ArrowRight: [1, 0],
  ArrowUp: [0, -1],
  ArrowDown: [0, 1],
};

/**
 * Resolve one key to its editing intent, or null when the canvas does not own it.
 *
 * Space holds temporary navigation; Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y walk document history;
 * Enter commits and Esc cancels; Delete clears the selected area; arrows nudge by one pixel, or
 * ten with Shift, while moving or transforming. The coverage panel blocks every key except Esc,
 * and a selection modification preview admits only navigation, Enter and Esc.
 */
export function canvasEditKeyIntent(
  key: CanvasEditKey,
  state: CanvasEditState,
  surface: CanvasEditSurface,
): CanvasEditKeyIntent | null {
  if (surface.kind === 'coverage')
    return key.phase === 'down' && key.key === 'Escape' ? { kind: 'close-coverage' } : null;
  // Auto-repeat still restates the hold, so the page never scrolls; the owner sends changes only.
  if (key.key === ' ' && !key.ctrl && !key.meta && !key.alt)
    return { kind: 'command', command: { type: 'navigation', held: key.phase === 'down' } };
  if (key.phase !== 'down') return null;
  if (surface.kind === 'modify') {
    if (key.key === 'Escape') return { kind: 'cancel-modify' };
    return key.key === 'Enter' ? { kind: 'apply-modify' } : null;
  }
  const command =
    (key.ctrl || key.meta) && !key.alt ? historyCommand(key) : plainCommand(key, state);
  return command ? { kind: 'command', command } : null;
}

function historyCommand(key: CanvasEditKey): CanvasEditCommand | null {
  const letter = key.key.toLowerCase();
  if (letter === 'z') return { type: 'history', action: key.shift ? 'redo' : 'undo' };
  if (letter === 'y' && !key.shift) return { type: 'history', action: 'redo' };
  return null;
}

function plainCommand(key: CanvasEditKey, state: CanvasEditState): CanvasEditCommand | null {
  if (key.ctrl || key.meta || key.alt) return null;
  if (key.key === 'Enter') return { type: 'commit' };
  if (key.key === 'Escape') return { type: 'cancel' };
  if (key.key === 'Delete') return state.selection ? { type: 'selection', action: 'clear' } : null;
  const nudge = NUDGES[key.key];
  if (!nudge || !nudges(state)) return null;
  const step = key.shift ? 10 : 1;
  return { type: 'nudge', dx: nudge[0] * step, dy: nudge[1] * step };
}

/** Nudge only where the host moves pixels: the Move tool and move or transform sessions. */
function nudges(state: CanvasEditState): boolean {
  return (
    state.tool === 'move' || state.session?.kind === 'move' || state.session?.kind === 'transform'
  );
}
