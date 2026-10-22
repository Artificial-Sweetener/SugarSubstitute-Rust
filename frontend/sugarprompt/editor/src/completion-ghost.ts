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
/** Ghost projection reads the selected native completion's prepared text; it never participates in source, selection or history. */
import { selectedCompletion } from './completion-session.js';
import {
  Decoration,
  ViewPlugin,
  WidgetType,
  EditorView,
  type DecorationSet,
  type ViewUpdate,
} from '@codemirror/view';
import { featureEnabled } from '../../services/src/index.js';
import { featureProfile } from './features.js';
import { editorFreshness } from './freshness.js';

/** Prepared literal ghost text is noneditable, noninteractive and excluded from assistive/source text. */
class CompletionGhost extends WidgetType {
  private readonly text: string;
  /** Capture immutable prepared display text for one selected option. */
  constructor(text: string) {
    super();
    this.text = text;
  }
  /** Reuse the actual DOM widget while the selected completion produces the same suffix. */
  override eq(other: CompletionGhost): boolean {
    return this.text === other.text;
  }
  /** Safe text content preserves literal host metadata without HTML parsing. */
  override toDOM(): HTMLElement {
    const span = document.createElement('span');
    span.className = 'cm-tag-ghost';
    span.setAttribute('aria-hidden', 'true');
    span.textContent = this.text;
    return span;
  }
  /** Pointer events remain on native editable text. */
  override ignoreEvent(): boolean {
    return true;
  }
}

/** Only derived decorations are retained; CodeMirror remains the selected-row and editable-caret owner. */
class GhostProjection {
  decorations: DecorationSet = Decoration.none;
  private readonly view: EditorView;
  /** Initialize from already prepared native completion metadata. */
  constructor(view: EditorView) {
    this.view = view;
    this.refresh();
  }
  /** Source/context/selection/permission changes recompute a bounded projection from one selected row. */
  update(_update: ViewUpdate): void {
    this.refresh();
  }
  /** Composition/focus observations suppress ghost rendering without inspecting or preparing source. */
  refresh(): void {
    const state = this.view.state,
      selected = selectedCompletion(state);
    const prepared = selected;
    this.decorations =
      prepared?.ghost &&
      prepared.freshness === state.field(editorFreshness) &&
      prepared.caret === state.selection.main.head &&
      state.selection.main.empty &&
      !state.readOnly &&
      this.view.hasFocus &&
      !this.view.compositionStarted &&
      !this.view.composing &&
      featureEnabled(state.facet(featureProfile), 'autocomplete_ghost_text')
        ? Decoration.set([
            Decoration.widget({
              widget: new CompletionGhost(prepared.ghost),
              side: 1,
            }).range(prepared.caret),
          ])
        : Decoration.none;
  }
}

/** Native widget projection owns neither session selection nor logical editable content. */
export const completionGhostProjection = [
  ViewPlugin.fromClass(GhostProjection, {
    decorations: (value) => value.decorations,
  }),
  EditorView.baseTheme({
    '.cm-tag-ghost': {
      color: '#758098',
      opacity: '0.7',
      pointerEvents: 'none',
      userSelect: 'none',
    },
    '.cm-tooltip-autocomplete': { maxWidth: 'min(32rem, 95vw)' },
  }),
];
