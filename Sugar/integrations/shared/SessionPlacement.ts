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
/** Admit portable normal outer-window placement before native monitor matching and clamping. */
import type { SessionFailure } from '../contracts/SessionProtocol.generated.js';
import { isSessionPlacement } from './SessionSnapshotValidation.generated.js';
import { sessionJsonScalars } from './SessionPrimitives.js';

/** Reject empty/control-bearing native identifiers without acquiring any native window authority. */
export function sessionPlacementError(value: unknown): SessionFailure['code'] | null {
  return isSessionPlacement(value) &&
    sessionJsonScalars(value) &&
    value.monitor.length > 0 &&
    !/[\u0000-\u001f\u007f-\u009f]/.test(value.monitor)
    ? null
    : 'invalid';
}
