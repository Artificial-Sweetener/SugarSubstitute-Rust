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
/** Prepared missing-reference metadata is mapped by CodeMirror and interpreted only in a source-current projection. */
import { RangeValue } from '@codemirror/state';
import type { WildcardDiagnostic } from '../../services/src/index.js';

/** One immutable finding retains exact-source payload and prepared logical-document UTF-16 body bounds. */
export class WildcardDiagnosticValue extends RangeValue {
  readonly diagnostic: WildcardDiagnostic;
  readonly bodyFrom: number;
  readonly bodyTo: number;
  readonly committedAtEnd: boolean;

  /** Capture semantic preparation once; source mapping retires these body bounds until the next current publication. */
  constructor(
    diagnostic: WildcardDiagnostic,
    bodyFrom: number,
    bodyTo: number,
    committedAtEnd: boolean,
  ) {
    super();
    this.diagnostic = diagnostic;
    this.bodyFrom = bodyFrom;
    this.bodyTo = bodyTo;
    this.committedAtEnd = committedAtEnd;
  }
}
