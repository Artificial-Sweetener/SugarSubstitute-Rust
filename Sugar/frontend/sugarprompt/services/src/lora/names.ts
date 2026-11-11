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
/** Model comparison spellings are derived from literal host identities, never written back to source. */
import { caseFold } from '../../../core/src/index.js';

/** Strip only the donor's supported installed-model extensions; unknown suffixes remain identity. */
export function withoutLoraExtension(value: string): string {
  const folded = caseFold(value);
  for (const suffix of ['.safetensors', '.ckpt', '.pt'])
    if (folded.endsWith(suffix)) return value.slice(0, -suffix.length);
  return value;
}

/** Normalize separators and full Unicode case folding without trimming or changing authored host text. */
export function loraNameKey(value: string, extensionless = false): string {
  return caseFold((extensionless ? withoutLoraExtension(value) : value).replaceAll('\\', '/'));
}
