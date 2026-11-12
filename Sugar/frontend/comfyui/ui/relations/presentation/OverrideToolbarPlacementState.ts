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
/** Define the user-owned override placement independently of available native chrome. */
export interface OverrideToolbarPosition {
  x: number;
  y: number;
}

/** Retain the nearest edge and proportional position along it through viewport changes. */
export interface OverrideToolbarFloatingAnchor {
  edge: 'left' | 'right' | 'top' | 'bottom';
  distance: number;
  horizontalRatio: number;
  verticalRatio: number;
}

export interface OverrideToolbarPlacementState {
  docked: boolean;
  floating: OverrideToolbarPosition | null;
  anchor: OverrideToolbarFloatingAnchor | null;
}

export interface OverrideToolbarPlacementStore {
  read(): OverrideToolbarPlacementState;
  write(state: OverrideToolbarPlacementState): void;
}
