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
/** Require source-current native decorations before a view exists, independent of caret or viewport position. */
import { EditorView } from '@codemirror/view';
import { createNativeState } from '../../../frontend/sugarprompt/editor/src/native-state.js';
import { richProjection } from '../../../frontend/sugarprompt/editor/src/rich-projection.js';
import { editorFreshness } from '../../../frontend/sugarprompt/editor/src/freshness.js';
import { EmphasisWidget } from '../../../frontend/sugarprompt/editor/src/projection-widgets.js';
import type { EditorState } from '@codemirror/state';

/** Create an initial native document without permitting layout, timers or a view to supply its syntax. */
function initialState(source: string): EditorState {
  return createNativeState(source, {
    changeRichRendering: () => {
      throw new Error('Unexpected presentation intent');
    },
    label: 'Prompt',
    placeholder: '',
    configuration: [],
    view: () => {
      throw new Error('Initial syntax cannot require a mounted view');
    },
    publish: () => undefined,
    isAlive: () => true,
    lexicon: () => undefined,
    update: () => undefined,
    onLayout: undefined,
    onSourceLines: undefined,
    revision: () => 0,
  });
}

test('provides complete rich presentation before CodeMirror constructs its first DOM', () => {
  const source = 'portrait, (blue eyes:1.25), (pink hair:1.40)';
  const state = initialState(source);
  expect(state.field(richProjection).sourceIdentity).toBe(state.field(editorFreshness).source);
  expect(state.field(richProjection).decorations.size).toBeGreaterThan(0);
  expect(state.doc.toString()).toBe(source);
});

test('supplies emphasis outside the caret window to native virtualization without needing focus or pixel geometry', () => {
  const source = '(first:1.25)\n' + 'plain words, '.repeat(1000) + '\n(last:1.40)';
  const state = initialState(source);
  const weights: string[] = [];
  for (const decoration of state.facet(EditorView.decorations)) {
    if (typeof decoration === 'function') continue;
    decoration.between(0, state.doc.length, (_from, _to, value) => {
      const widget: unknown = value.spec.widget;
      if (widget instanceof EmphasisWidget && widget.part === 'suffix') weights.push(widget.weight);
    });
  }
  expect(weights).toEqual(['1.25', '1.40']);
  expect(state.doc.toString()).toBe(source);
});
