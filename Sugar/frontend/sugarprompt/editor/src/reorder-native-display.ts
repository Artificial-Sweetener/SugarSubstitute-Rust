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
/** Mapped inert native decoration lifetime is independent of current semantic/capture authority. */
import { StateField } from '@codemirror/state';
import { Decoration, type DecorationSet } from '@codemirror/view';
import { semanticStructures } from './structure.js';
import { documentKind } from './document-kind.js';
import { nativeReorderRecipeDecorations } from './reorder-native-recipe.js';

/** Only presentation ranges survive pending preparation; no source, semantic entry or command capability is retained. */
export const nativeReorderDisplay = StateField.define<DecorationSet>({
  /** Fresh editor lifetimes borrow only an already-current prepared output. */
  create(state) {
    return nativeReorderRecipeDecorations(
      state.field(semanticStructures, false)?.native?.nativeValues,
    );
  },
  /** Native source edits map inert wrappers until current preparation replaces them; document-kind/empty-source boundaries retire them. */
  update(value, transaction) {
    if (
      transaction.startState.facet(documentKind) !== transaction.state.facet(documentKind) ||
      transaction.state.doc.length === 0
    )
      return Decoration.none;
    const values = transaction.state.field(semanticStructures).native?.nativeValues;
    if (values) return nativeReorderRecipeDecorations(values);
    return transaction.docChanged ? value.map(transaction.changes) : value;
  },
});
