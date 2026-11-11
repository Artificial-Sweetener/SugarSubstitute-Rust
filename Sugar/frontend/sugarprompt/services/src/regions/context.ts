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
/** Hosts classify one endpoint; prepared context contains no workflow graph, source or mutable topology. */
import type { ConditioningMode } from '../../../core/src/index.js';

/** A stable host key identifies the endpoint and current conditioning topology together. */
export interface ConditioningContextInput {
  readonly key: string;
  readonly mode: ConditioningMode;
}
/** Absent graph authority remains explicitly unresolved; captured host values are immutable. */
export interface ConditioningContext {
  readonly key: string | null;
  readonly mode: ConditioningMode;
}
/** One shared unavailable value reuses actual derived output across ordinary selection and baseline recreation. */
export const unresolvedConditioning: ConditioningContext = Object.freeze({
  key: null,
  mode: 'unresolved',
});

/** Validate unknown host classification before adapting it; source and graph discovery stay with their owners. */
export function captureConditioningContext(value: unknown): ConditioningContext {
  if (value === null || value === undefined) return unresolvedConditioning;
  if (
    !value ||
    typeof value !== 'object' ||
    Object.keys(value).sort().join() !== 'key,mode' ||
    !('key' in value) ||
    typeof value.key !== 'string' ||
    !value.key ||
    !('mode' in value) ||
    (value.mode !== 'unresolved' && value.mode !== 'independent' && value.mode !== 'regional')
  )
    throw new TypeError('Invalid conditioning context');
  return Object.freeze({ key: value.key, mode: value.mode });
}
