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
/**
 * Own the SugarCubes dirty-state evaluation layer in
 * `frontend/comfyui/ui/graph/DirtyStateService.js`.
 */

/**
 * Coordinate typed dirty-state evaluation for managed cube instances.
 */
export type ImplementationDirtyReason = 'unknown' | 'missing-symbols' | 'hash-mismatch';

export interface DirtyStateRequest {
  implementationCurrentHash?: string | null;
  implementationBaselineHash?: string | null;
  cosmeticCurrentHash?: string | null;
  cosmeticBaselineHash?: string | null;
  surfaceCurrentHash?: string | null;
  surfaceBaselineHash?: string | null;
  isKnown?: boolean;
  missingSymbols?: boolean;
  previousDirtyAt?: string | null;
}

export interface DirtyStateResult {
  implementationDirty: boolean;
  implementationReasons: ImplementationDirtyReason[];
  dirtyAt: string | null;
  cosmeticDirty: boolean;
  surfaceValuesChanged: boolean;
}

/** Evaluate implementation, cosmetic, and surface dirty-state concerns. */
export class DirtyStateService {
  evaluate({
    implementationCurrentHash,
    implementationBaselineHash,
    cosmeticCurrentHash,
    cosmeticBaselineHash,
    surfaceCurrentHash,
    surfaceBaselineHash,
    isKnown,
    missingSymbols,
    previousDirtyAt,
  }: DirtyStateRequest = {}): DirtyStateResult {
    const implementationReasons: ImplementationDirtyReason[] = [];
    if (!isKnown) {
      implementationReasons.push('unknown');
    }
    if (missingSymbols) {
      implementationReasons.push('missing-symbols');
    }
    if (implementationCurrentHash !== implementationBaselineHash) {
      implementationReasons.push('hash-mismatch');
    }
    const implementationDirty = implementationReasons.length > 0;
    return {
      implementationDirty,
      implementationReasons,
      dirtyAt: implementationDirty ? previousDirtyAt || new Date().toISOString() : null,
      cosmeticDirty: cosmeticCurrentHash !== cosmeticBaselineHash,
      surfaceValuesChanged: surfaceCurrentHash !== surfaceBaselineHash,
    };
  }
}
