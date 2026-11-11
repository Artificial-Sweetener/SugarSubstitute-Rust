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
/** Measure public JSON messages against native transport bounds without adapter framing. */

/** The native host's decoded surface message ceiling, in UTF-8 bytes of the JSON text. */
export const CANVAS_SURFACE_MESSAGE_BYTES = 32768;

/**
 * Report whether a message's serialized JSON fits a UTF-8 byte budget.
 *
 * Counting stops at the first byte over the budget, so oversized values cost no more than the
 * budget to reject after serialization.
 */
export function canvasMessageFits(message: unknown, bytes: number): boolean {
  const encoded = JSON.stringify(message);
  let size = 0;
  for (const character of encoded) {
    const point = character.codePointAt(0) ?? 0;
    size += point <= 0x7f ? 1 : point <= 0x7ff ? 2 : point <= 0xffff ? 3 : 4;
    if (size > bytes) return false;
  }
  return true;
}
