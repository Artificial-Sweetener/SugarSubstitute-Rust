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
/** Build deterministic invalidation keys for persisted Cube card visibility. */

import { parseCubeSurfaceState } from './CubeSurfaceState.js';

/** Describe only persisted state that changes which Cube cards are rendered. */
export function buildCubeCardVisibilitySignature(surface: unknown): string {
  const cards = parseCubeSurfaceState(surface).cards;
  return JSON.stringify(
    Object.entries(cards)
      .sort(([leftId], [rightId]) => leftId.localeCompare(rightId))
      .map(([nodeId, state]) => [nodeId, state.authoredBypass, state.revealed]),
  );
}
