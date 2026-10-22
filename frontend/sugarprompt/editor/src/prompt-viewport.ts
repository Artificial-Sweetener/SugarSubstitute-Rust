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
/** Bound rendered prompt decoration to the visible text while canonical syntax remains source complete. */
import {
  StateEffect,
  StateField,
  Transaction,
  type EditorState,
  type Extension,
  type Range,
  type Text,
} from '@codemirror/state';
import {
  Decoration,
  ViewPlugin,
  type DecorationSet,
  type EditorView,
  type ViewUpdate,
} from '@codemirror/view';
import { snapshotPresentation } from './native-snapshot-policy.js';

interface PromptViewport {
  readonly document: Text;
  readonly from: number;
  readonly to: number;
}
const publishViewport = StateEffect.define<PromptViewport>();
const margin = 128;

/** Keep an eager keyboard destination available before native scroll geometry settles. */
function aroundSelection(state: EditorState): PromptViewport {
  const head = state.selection.main.head;
  return {
    document: state.doc,
    from: Math.max(0, head - 2048),
    to: Math.min(state.doc.length, head + 2048),
  };
}

/** Retain only native logical positions; this window confers no semantic or command authority. */
export const promptViewport = StateField.define<PromptViewport>({
  create: aroundSelection,
  update(value, transaction) {
    if (transaction.docChanged)
      value = {
        document: transaction.state.doc,
        from: transaction.changes.mapPos(value.from, -1),
        to: transaction.changes.mapPos(value.to, 1),
      };
    for (const effect of transaction.effects)
      if (effect.is(publishViewport) && effect.value.document === transaction.state.doc)
        value = effect.value;
    const head = transaction.state.selection.main.head;
    if (transaction.selection && (head < value.from || head > value.to))
      return aroundSelection(transaction.state);
    return value;
  },
});

/** Paint whole native decorations intersecting the window, including the enclosing logical line's attributes. */
export function viewportPromptDecorations(
  state: EditorState,
  decorations: DecorationSet,
): DecorationSet {
  const viewport = state.field(promptViewport, false);
  if (!viewport || (viewport.from === 0 && viewport.to === state.doc.length)) return decorations;
  const ranges: Range<Decoration>[] = [];
  const lineStart = state.doc.lineAt(viewport.from).from;
  if (lineStart < viewport.from)
    decorations.between(lineStart, lineStart, (from, to, value) => {
      if (from === lineStart && to === from && value.point && !value.spec.widget)
        ranges.push(value.range(from, to));
    });
  decorations.between(viewport.from, viewport.to, (from, to, value) => {
    // Deletion can collapse a mapped mark before semantic preparation replaces it; only point decorations retain empty ranges.
    if (from < to || value.point) ranges.push(value.range(from, to));
  });
  return Decoration.set(ranges, true);
}

/** Follow native viewport geometry without parsing text or creating an alternative editable surface. */
class PromptViewportLayout {
  private readonly view: EditorView;
  private disposed = false;
  /** Request the first geometry only after CodeMirror owns the mounted native document. */
  constructor(view: EditorView) {
    this.view = view;
    this.measure();
  }
  /** Geometry and native source navigation invalidate only this editor's presentation window. */
  update(update: ViewUpdate): void {
    if (
      update.docChanged ||
      update.viewportChanged ||
      update.geometryChanged ||
      update.selectionSet
    )
      this.measure();
  }
  /** Fence every queued layout publication at the view's finite lifetime boundary. */
  destroy(): void {
    this.disposed = true;
  }
  /** Derive source positions from real native pixel geometry, then publish beyond the measure write phase. */
  private measure(): void {
    this.view.requestMeasure({
      key: this,
      read: (view) => {
        if (!view.inView || this.disposed) return null;
        const rect = view.scrollDOM.getBoundingClientRect();
        const top = Math.max(0, rect.top) + 3,
          bottom =
            Math.min(view.dom.ownerDocument.defaultView?.innerHeight ?? rect.bottom, rect.bottom) -
            3;
        if (bottom <= top) return null;
        const first = view.posAtCoords({ x: rect.left + 15, y: top }, false);
        const last = view.posAtCoords({ x: rect.right - 5, y: bottom }, false);
        if (first === null || last === null) return null;
        return {
          document: view.state.doc,
          from: Math.max(0, Math.min(first, last) - margin),
          to: Math.min(view.state.doc.length, Math.max(first, last) + margin),
        };
      },
      write: (viewport, view) => {
        if (!viewport || this.disposed || viewport.document !== view.state.doc) return;
        const current = view.state.field(promptViewport);
        if (
          current.from <= viewport.from &&
          current.to >= viewport.to &&
          current.to - current.from <= (viewport.to - viewport.from) * 2
        )
          return;
        queueMicrotask(() => {
          if (
            this.disposed ||
            view.state.doc !== viewport.document ||
            view.state.field(promptViewport) !== current
          )
            return;
          view.dispatch({
            effects: publishViewport.of(viewport),
            annotations: [Transaction.addToHistory.of(false), snapshotPresentation.of(true)],
          });
        });
      },
    });
  }
}

/** Share one geometry window across rich syntax, native carriers and conditional wrapping. */
export const promptViewportExtension: Extension = [
  promptViewport,
  ViewPlugin.fromClass(PromptViewportLayout),
];
