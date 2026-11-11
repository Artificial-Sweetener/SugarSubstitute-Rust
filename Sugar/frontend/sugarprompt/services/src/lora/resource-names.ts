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
/** Resource requests and effective schedule preparation share exact parsed authored-name identity. */
import type { PromptDocument } from '../../../core/src/index.js';

/** Return unique exact source names in first occurrence order; parser ranges use exact-source UTF-16. */
export function loraResourceNames(document: PromptDocument): readonly string[] {
  return Object.freeze([
    ...new Set(
      document.loraSpans.map((span) =>
        document.sourceText.slice(span.nameRange.from, span.nameRange.to),
      ),
    ),
  ]);
}
