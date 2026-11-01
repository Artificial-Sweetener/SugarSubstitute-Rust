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
/** Distinguish a recoverable preparation deadline from other attachment failures. */

/**
 * A surface whose host did not finish preparing within its deadline.
 *
 * The host may still be working (for example under load), so the attachment owner can retry
 * with a fresh attachment instead of treating the surface as permanently failed.
 */
export class CanvasPreparationTimeout extends Error {
  /** Name the elapsed deadline for diagnostics. */
  constructor(message = 'Native canvas preparation timed out.') {
    super(message);
    this.name = 'CanvasPreparationTimeout';
  }
}
