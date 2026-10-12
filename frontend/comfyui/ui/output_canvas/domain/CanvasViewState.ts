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
/** Describe authoritative selection and inspection intent without renderer state. */

import type { OutputSourceId } from '../../execution/domain/ExecutionIdentity.js';
import type { CanvasViewAddress } from './CanvasIdentity.js';

export interface CanvasSelection {
  readonly sourceId: OutputSourceId;
  readonly batchIndex: number;
}

export interface NormalizedInspection {
  readonly centerX: number;
  readonly centerY: number;
  readonly spanX: number;
  readonly spanY: number;
}

export interface CanvasComparison {
  readonly secondary: CanvasSelection;
  readonly orientation: 'vertical' | 'horizontal';
  readonly split: number;
}

export interface CanvasViewState extends CanvasViewAddress {
  readonly revision: number;
  readonly selection: CanvasSelection | null;
  readonly follow: 'auto' | 'manual';
  readonly mode: 'image' | 'overview';
  readonly linked: boolean;
  readonly comparison: CanvasComparison | null;
}

/** Create independent default state for an authority-owned view. */
export function initialCanvasViewState(address: CanvasViewAddress): CanvasViewState {
  return Object.freeze({
    ...address,
    revision: 0,
    selection: null,
    follow: 'auto',
    mode: 'image',
    linked: true,
    comparison: null,
  });
}

/** Copy nested coordination values so a caller cannot mutate a published state. */
export function freezeCanvasViewState(state: CanvasViewState, revision: number): CanvasViewState {
  return Object.freeze({
    ...state,
    revision,
    selection: state.selection ? Object.freeze({ ...state.selection }) : null,
    comparison: state.comparison
      ? Object.freeze({
          ...state.comparison,
          secondary: Object.freeze({ ...state.comparison.secondary }),
        })
      : null,
  });
}
