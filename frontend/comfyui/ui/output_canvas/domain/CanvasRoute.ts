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
/** Describe the Input/Output route a canvas shows, independently of who provisions its inputs. */

export type CanvasRouteId = 'input' | 'output';

/** One route control's state with its literal labels. */
export interface CanvasRouteState {
  readonly route: CanvasRouteId;
  readonly inputAvailable: boolean;
  /** Why Input is disabled, or an empty string. */
  readonly reason: string;
  readonly labels: { readonly input: string; readonly output: string; readonly group: string };
}

/** Read and choose the canvas route; null state means the canvas has no Input route. */
export interface CanvasRoutePort {
  read(): CanvasRouteState | null;
  subscribe(listener: () => void): () => void;
  choose(route: CanvasRouteId): void;
  /** Observe requests for the canvas to take keyboard focus, as when a card opens Input. */
  subscribeFocus(listener: () => void): () => void;
}
