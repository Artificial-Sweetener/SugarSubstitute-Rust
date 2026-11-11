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
/** Regional draft proof observes both native CodeMirror decoration precedence layers. */
import type { EditorState } from '@codemirror/state';
import { EditorView, type DecorationSet } from '@codemirror/view';

/** Resolve actual state facets; dynamic providers require the caller's existing native fixture view. */
export function regionDraftDecorationSets(
  state: EditorState,
  view?: EditorView,
): readonly DecorationSet[] {
  if (view && view.state !== state) throw new Error('Regional decoration fixture state mismatch');
  return [...state.facet(EditorView.decorations), ...state.facet(EditorView.outerDecorations)].map(
    (provider) => {
      if (typeof provider !== 'function') return provider;
      if (!view) throw new Error('Regional decoration provider requires view');
      return provider(view);
    },
  );
}
