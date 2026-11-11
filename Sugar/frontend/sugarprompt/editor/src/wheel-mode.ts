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
/** Validated preference input configures native wheel authorization without owning persistence. */
import { Facet } from '@codemirror/state';
import type { FeaturePreferences } from '../../services/src/index.js';

/** Share the service-owned closed preference contract at the native editor boundary. */
export type WheelMode = FeaturePreferences['wheelMode'];

/** Reject malformed host intent before native configuration changes. */
export function validateWheelMode(value: unknown): asserts value is WheelMode {
  if (value !== 'hover_dwell' && value !== 'focus_required')
    throw new TypeError('Invalid wheel mode');
}

/** Missing initialization uses the default; explicitly malformed host values reject. */
export function initialWheelMode(value: unknown): WheelMode {
  if (value === undefined) return 'hover_dwell';
  validateWheelMode(value);
  return value;
}

/** One component input supplies the mode; a standalone editor uses the donor's dwell default. */
export const wheelMode = Facet.define<WheelMode, WheelMode>({
  /** Multiple configuration owners are an invariant failure. */
  combine(values) {
    if (values.length > 1) throw new Error('Multiple wheel mode owners');
    return values[0] ?? 'hover_dwell';
  },
});
