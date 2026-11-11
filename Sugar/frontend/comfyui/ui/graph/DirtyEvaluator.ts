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
 * Own the SugarCubes graph integration layer in `frontend/comfyui/ui/graph/DirtyEvaluator.js`.
 */

/**
 * Coordinate dirty evaluator behavior for the SugarCubes UI.
 */
export type DirtyReason = 'unknown' | 'missing-symbols' | 'hash-mismatch';

export interface DirtyEvaluationOptions {
  currentHash?: string | null;
  baselineHash?: string | null;
  isKnown?: boolean;
  missingSymbols?: boolean;
  previousDirtyAt?: string | null;
}

export interface DirtyEvaluation {
  dirty: boolean;
  reasons: DirtyReason[];
  dirtyAt: string | null;
}

/** Evaluate implementation dirtiness against an authoritative baseline. */
export class DirtyEvaluator {
  evaluate({
    currentHash,
    baselineHash,
    isKnown,
    missingSymbols,
    previousDirtyAt,
  }: DirtyEvaluationOptions = {}): DirtyEvaluation {
    const reasons: DirtyReason[] = [];
    if (!isKnown) {
      reasons.push('unknown');
    }
    if (missingSymbols) {
      reasons.push('missing-symbols');
    }
    if (currentHash !== baselineHash) {
      reasons.push('hash-mismatch');
    }
    const dirty = reasons.length > 0;
    const dirtyAt = dirty ? previousDirtyAt || new Date().toISOString() : null;
    return { dirty, reasons, dirtyAt };
  }
}
