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
/** Native state consumes the host's captured classification without owning workflow discovery. */
import { Facet } from '@codemirror/state';
import {
  unresolvedConditioning,
  type ConditioningContext,
} from '../../services/src/regions/context.js';

/** One finite facade compartment owns conditioning authority and command-context retirement. */
export const conditioningContext = Facet.define<ConditioningContext, ConditioningContext>({
  /** Duplicate endpoint authority is an observable configuration invariant. */
  combine(values) {
    if (values.length > 1) throw new Error('Multiple conditioning context owners');
    return values[0] ?? unresolvedConditioning;
  },
});
