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
/** Prove viewport paint stays bounded while canonical syntax and native source remain complete. */
import { EditorState } from '@codemirror/state';
import { Decoration } from '@codemirror/view';
import {
  promptViewport,
  viewportPromptDecorations,
} from '../../../frontend/sugarprompt/editor/src/prompt-viewport.js';

test('retain intersecting marks and logical-line attributes without altering canonical ranges', () => {
  const source = 'a'.repeat(100_000);
  const state = EditorState.create({
    doc: source,
    selection: { anchor: 50_000 },
    extensions: [promptViewport],
  });
  const canonical = Decoration.set(
    [
      Decoration.line({ class: 'logical-line' }).range(0),
      Decoration.mark({ class: 'whole-line' }).range(0, 100_000),
      Decoration.mark({ class: 'outside' }).range(0, 100),
      Decoration.mark({ class: 'visible' }).range(50_000, 50_020),
      Decoration.mark({ class: 'outside' }).range(99_900, 100_000),
    ],
    true,
  );
  const clipped = viewportPromptDecorations(state, canonical);
  const classes: string[] = [];
  clipped.between(0, source.length, (_from, _to, value) => {
    classes.push(String(value.spec.class));
  });
  expect(classes).toEqual(['logical-line', 'whole-line', 'visible']);
  expect(canonical.size).toBe(5);
  expect(state.doc.toString()).toBe(source);
  const next = state.update({ selection: { anchor: 5 } }).state;
  expect(next.field(promptViewport).from).toBe(0);
  expect(state.field(promptViewport).from).toBeGreaterThan(40_000);
});

test('map native viewport positions through source edits without retaining another editable buffer', () => {
  const state = EditorState.create({
    doc: 'x'.repeat(10_000),
    selection: { anchor: 5_000 },
    extensions: [promptViewport],
  });
  const before = state.field(promptViewport);
  const next = state.update({ changes: { from: 0, insert: 'prefix ' } }).state;
  expect(next.field(promptViewport)).toEqual({
    document: next.doc,
    from: before.from + 7,
    to: before.to + 7,
  });
  expect(viewportPromptDecorations(EditorState.create({ doc: 'plain' }), Decoration.none)).toBe(
    Decoration.none,
  );
});
