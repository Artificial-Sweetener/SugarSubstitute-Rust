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
/** One native presentation intent shared by current rendering owners, separate from source and history. */
import { Facet } from '@codemirror/state';

/** Reject malformed host intent before changing the native presentation owner. */
export function validateRichRendering(value: unknown): asserts value is boolean {
  if (typeof value !== 'boolean') throw new TypeError('Rich rendering requires a boolean');
}

/** Rich presentation defaults on; the component supplies one reconfigurable mode input. */
export const richRendering = Facet.define<boolean, boolean>({
  /** Reject competing presentation owners and use the donor's rich default. */
  combine(values) {
    if (values.length > 1) throw new Error('Multiple rich presentation owners');
    return values[0] ?? true;
  },
});
