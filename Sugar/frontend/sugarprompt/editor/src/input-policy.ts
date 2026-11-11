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
/** Native deletion resolves logical Unicode source ranges and prepared structural boundaries through CodeMirror transactions. */
import { EditorSelection } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { isolateHistory } from '@codemirror/commands';
import { editorFreshness } from './freshness.js';
import { EmphasisWidget, type SourceToken } from './projection-widgets.js';
import { LoraWidget } from './lora-widget.js';
import { revealProjectionToken, currentInputProjection } from './rich-projection.js';
import type { SourceRange } from '../../core/src/offsets.js';
import { nativeGraphemeRange } from './native-graphemes.js';
import { RegionWidget } from './region-widget.js';

/** Current projected structure chooses source deletion or a source-neutral reveal; bounds use logical-document UTF-16. */
type StructuralDeletion =
  | { readonly kind: 'delete'; readonly range: SourceRange }
  | { readonly kind: 'reveal'; readonly range: SourceRange };

/** Current logical UTF-16 wrappers reveal emphasis/LoRA source or remove one regional bracket, including a Delete immediately before its line. */
function structuralEdge(view: EditorView, forward: boolean): StructuralDeletion | null {
  const state = view.state,
    prepared = currentInputProjection(state);
  if (!prepared) return null;
  const position = state.selection.main.head;
  let token: SourceToken | null = null,
    region: SourceRange | null = null;
  prepared.decorations.between(position, position, (from, to, decoration) => {
    if (decoration.spec.widget instanceof RegionWidget) {
      if (forward && position === from) region = { from, to: from + 1 };
      else if (!forward && position === to) region = { from: to - 1, to };
    }
    if (
      (decoration.spec.widget instanceof EmphasisWidget ||
        decoration.spec.widget instanceof LoraWidget) &&
      (forward ? from <= position && position < to : from < position && position <= to)
    )
      token = decoration.spec.widget.token;
  });
  if (forward && !region && position < state.doc.length)
    prepared.decorations.between(position + 1, position + 1, (from, _to, decoration) => {
      if (from === position + 1 && decoration.spec.widget instanceof RegionWidget)
        region = { from, to: from + 1 };
    });
  if (region) return { kind: 'delete', range: region };
  let target: SourceRange | null = null;
  if (token)
    prepared.tokens.between(position, position, (from, to, value) => {
      if (value === token) target = { from, to };
    });
  return target ? { kind: 'reveal', range: target } : null;
}

/** Delete the exact selected source, a prepared structural bracket or a complete native grapheme; concealed emphasis/LoRA edges reveal first. */
export function deleteGrapheme(view: EditorView, forward: boolean): boolean {
  if (view.state.readOnly || view.compositionStarted) return false;
  const selection = view.state.selection.main;
  let from = selection.from,
    to = selection.to;
  if (selection.empty) {
    const intent = structuralEdge(view, forward);
    if (intent?.kind === 'reveal') {
      const reveal = intent.range;
      view.dispatch({
        selection: EditorSelection.single(reveal.from, reveal.to),
        effects: revealProjectionToken.of({
          sourceIdentity: view.state.field(editorFreshness).source,
          range: reveal,
        }),
        annotations: isolateHistory.of('full'),
        scrollIntoView: true,
        userEvent: 'select.structural',
      });
      return true;
    }
    const range = intent?.range ?? nativeGraphemeRange(view, selection.head, forward);
    if (!range) return false;
    from = range.from;
    to = range.to;
    const prepared = currentInputProjection(view.state);
    for (const ranges of intent
      ? []
      : prepared
        ? [() => prepared.atomic]
        : view.state.facet(EditorView.atomicRanges)) {
      const atomic = ranges(view);
      atomic.between(from, from, (start, end) => {
        if (start < from && from < end) from = start;
      });
      atomic.between(to, to, (start, end) => {
        if (start < to && to < end) to = end;
      });
    }
  }
  if (from === to) return false;
  view.dispatch({
    changes: { from, to },
    selection: { anchor: from },
    scrollIntoView: true,
    userEvent: forward ? 'delete.forward' : 'delete.backward',
  });
  return true;
}
