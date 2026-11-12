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
/** Literal canonical glyphs share the wildcard's native source token and preserve CodeMirror input ownership. */
import { SourceWidget, type SourceToken } from './projection-widgets.js';

/** One changed scalar group renders catalog spelling without rewriting the authored column. */
export class WildcardColumnWidget extends SourceWidget {
  readonly text: string;
  /** Capture prepared literal glyphs outside input and paint; source bounds belong to the parent projection. */
  constructor(token: SourceToken, text: string) {
    super(token);
    this.text = text;
  }
  /** Equal catalog glyphs retain actual native DOM across prepared source refreshes. */
  override eq(other: WildcardColumnWidget): boolean {
    return this.text === other.text;
  }
  /** CodeMirror owns native selection and widget buffers around this literal source-backed display. */
  override toDOM(): HTMLElement {
    const glyph = document.createElement('span');
    glyph.className = 'sp-wildcard-column-alias';
    glyph.textContent = this.text;
    return glyph;
  }
}
