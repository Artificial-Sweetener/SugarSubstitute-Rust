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
/** Decide which editing chrome owns the canvas from authoritative host state. */
import type { CanvasEditState } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';

/** The contextual surface shown over the canvas; at most one is presented at a time. */
export type CanvasEditSurface =
  | { readonly kind: 'none' }
  | { readonly kind: 'coverage' }
  | { readonly kind: 'modify' }
  | { readonly kind: 'selection' }
  | { readonly kind: 'transform'; readonly target: 'selection' | 'layer' }
  | { readonly kind: 'session'; readonly tool: 'move' | 'shared-edges'; readonly armed: boolean };

/**
 * Resolve the surface in the reference precedence.
 *
 * The exclusive coverage panel wins, then a selection modification preview, then a non-affine
 * edit session (or the armed shared-edges tool), then an affine transform, then the selection
 * actions. `modifying` is the page's own preview request, which may precede the host's session.
 * The host only selects Transform when it has something to frame, and opens its session at the
 * first change, so the selected tool alone arms the affine page: rotate and flip act at once,
 * while Apply waits for a change and Cancel leaves the tool.
 */
export function canvasEditSurface(state: CanvasEditState, modifying: boolean): CanvasEditSurface {
  const session = state.session;
  if (state.mode === 'coverage') return { kind: 'coverage' };
  if (modifying || session?.kind === 'modify') return { kind: 'modify' };
  if (session?.kind === 'move' || session?.kind === 'shared-edges')
    return { kind: 'session', tool: session.kind, armed: false };
  if (!session && state.tool === 'shared-edges')
    return { kind: 'session', tool: 'shared-edges', armed: true };
  if (session?.kind === 'transform' || (!session && state.tool === 'transform'))
    return { kind: 'transform', target: state.selection ? 'selection' : 'layer' };
  if (state.selection) return { kind: 'selection' };
  return { kind: 'none' };
}

/** Hide the rail and tool options while an exclusive editor owns canvas input. */
export function canvasEditChromeSuppressed(surface: CanvasEditSurface): boolean {
  return surface.kind === 'coverage' || surface.kind === 'modify' || surface.kind === 'transform';
}
