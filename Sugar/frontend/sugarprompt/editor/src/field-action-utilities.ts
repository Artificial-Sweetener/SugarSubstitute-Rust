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
/** Prepared host utility transports the original native focus capability into the existing picker owner. */
import { Facet } from '@codemirror/state';
import type { FocusHandoff } from './focus-handoff-contracts.js';
import { captureRecord } from '../../services/src/saved-segments/capture.js';
/** The host's finite picker owns its request, reservation and result; this binding supplies no competing state. */
export interface FieldActionUtilities {
  scheduleLora(handoff: FocusHandoff): Promise<void>;
}
/** Mounted capability identity participates in native menu freshness. */
export const fieldActionUtilities = Facet.define<
  FieldActionUtilities | null,
  FieldActionUtilities | null
>({ combine: (values) => values.at(-1) ?? null });
/** Validate the small injected transport without invoking picker work. */
export function validateFieldActionUtilities(value: FieldActionUtilities | null | undefined): void {
  if (value == null) return;
  const record = captureRecord(value, ['scheduleLora']);
  if (typeof record.scheduleLora !== 'function')
    throw new TypeError('invalid-field-action-utilities');
}
/** Capture the admitted callable and receiver; later property replacement/accessors are never consulted by activation. */
export function captureFieldActionUtilities(
  value: FieldActionUtilities | null,
): FieldActionUtilities | null {
  if (value === null) return null;
  const record = captureRecord(value, ['scheduleLora']);
  const schedule = record.scheduleLora;
  if (typeof schedule !== 'function') throw new TypeError('invalid-field-action-utilities');
  return Object.freeze({
    scheduleLora: (handoff: FocusHandoff) =>
      Reflect.apply(schedule, value, [handoff]) as Promise<void>,
  });
}
