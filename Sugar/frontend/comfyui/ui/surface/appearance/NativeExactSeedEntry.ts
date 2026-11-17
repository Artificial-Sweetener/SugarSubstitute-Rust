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
/** Show and accept a seed's exact digits in a number box that holds only their rounding. */
import type { ExactSeedText } from '../../execution/domain/WorkflowSeed.js';
import type { NativeNumericField } from './ComfyNativeNumericField.js';

const UNSIGNED_64 = 18446744073709551615n;

/** The parts of a seed field the exact entry reads; a redraw may replace them. */
export interface ExactSeedEntryField {
  readonly numeric: NativeNumericField;
  readonly exact: ExactSeedText | undefined;
}

/**
 * Return the digits of a typed seed that a page number cannot hold, or null for any other text.
 *
 * Only whole numbers above the exact range and within the field's own maximum qualify; every
 * other entry is an ordinary number and stays the number box's own business.
 */
export function exactSeedDigits(text: string, maximum: number): string | null {
  const cleaned = text.replace(/[\s,]/gu, '');
  if (!/^[0-9]{16,20}$/u.test(cleaned)) return null;
  const value = BigInt(cleaned);
  const ceiling = maximum >= Number(UNSIGNED_64) ? UNSIGNED_64 : BigInt(Math.trunc(maximum));
  return value > BigInt(Number.MAX_SAFE_INTEGER) && value <= ceiling ? value.toString() : null;
}

/**
 * Keep one seed box truthful about the seed a run will receive.
 *
 * While the box holds the rounding of recorded digits it shows the digits. A typed seed above
 * the exact range is recorded as typed and its rounding is given to the number box. Any other
 * entry is an ordinary number with no record, and the number box keeps every other behaviour.
 */
export function bindExactSeedEntry(
  input: HTMLInputElement,
  field: () => ExactSeedEntryField,
): {
  synchronize(): void;
  release(): void;
} {
  const view = input.ownerDocument.defaultView;
  let edited = false;
  const show = (): void => {
    const { numeric, exact } = field();
    const value = numeric.readValue();
    const digits = value === null ? null : (exact?.read(value) ?? null);
    if (digits !== null && !edited && input.value !== digits) input.value = digits;
  };
  // The number box rewrites its own text after a commit; the digits are shown once it has.
  const showAfterHost = (): void => {
    queueMicrotask(show);
    view?.setTimeout(show, 0);
  };
  const commit = (): void => {
    if (!edited) return;
    edited = false;
    const { numeric, exact } = field();
    if (!exact || numeric.disabled()) return;
    const digits = exactSeedDigits(input.value, numeric.maximum);
    if (digits === null) return;
    exact.write(digits);
    numeric.writeValue(Number(digits));
  };
  const onInput = (): void => {
    edited = true;
  };
  const onKey = (event: KeyboardEvent): void => {
    if (event.key !== 'Enter') return;
    commit();
    showAfterHost();
  };
  const onBlur = (): void => {
    commit();
    showAfterHost();
  };
  input.addEventListener('input', onInput);
  input.addEventListener('keydown', onKey, true);
  input.addEventListener('blur', onBlur, true);
  input.addEventListener('focus', show);
  return {
    synchronize: show,
    release: () => {
      input.removeEventListener('input', onInput);
      input.removeEventListener('keydown', onKey, true);
      input.removeEventListener('blur', onBlur, true);
      input.removeEventListener('focus', show);
    },
  };
}
