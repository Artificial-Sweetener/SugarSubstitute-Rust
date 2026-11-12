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
/** Full-source reveal retirement follows actual native endpoints, including directional crossing and multiple ranges. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, EditorState, StateEffect, Transaction } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import {
  richProjection,
  revealProjectionToken,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { tokenPointerFixture } from './token-pointer-fixture.ts';

for (const [kind, token] of [
  ['lora', '<lora:real:1>'],
  ['wildcard', '{animal|2}'],
  ['emphasis', '(cat:1.20)'],
  ['scene', '** Scene'],
] as const)
  for (const position of [
    'opening',
    'interior',
    'trailing',
    'outside',
    'cross-forward',
    'cross-reverse',
    'surrounding',
    'secondary-inside',
  ] as const)
    test(`${kind} full-source reveal lifetime follows ${position} native selection endpoints`, () => {
      const source = `prefix\r\n${token}\r\ntail`,
        f = tokenPointerFixture(source),
        from = 7,
        to = from + token.length;
      f.dispatch({
        effects: StateEffect.appendConfig.of(EditorState.allowMultipleSelections.of(true)),
      });
      f.dispatch({
        selection: { anchor: from, head: to },
        effects: revealProjectionToken.of({
          sourceIdentity: f.state.field(editorFreshness).source,
          range: { from, to },
        }),
        annotations: Transaction.addToHistory.of(false),
      });
      assert.deepEqual(f.state.field(richProjection).revealed, { from, to });
      const prepared = f.state.field(richProjection).all;
      const selection =
        position === 'opening'
          ? EditorSelection.cursor(from)
          : position === 'interior'
            ? EditorSelection.cursor(from + 1)
            : position === 'trailing'
              ? EditorSelection.cursor(to)
              : position === 'outside'
                ? EditorSelection.cursor(to + 1)
                : position === 'cross-forward'
                  ? EditorSelection.range(0, from + 1)
                  : position === 'cross-reverse'
                    ? EditorSelection.range(from + 1, 0)
                    : position === 'surrounding'
                      ? EditorSelection.range(0, to + 1)
                      : null;
      f.dispatch({
        selection: selection
          ? EditorSelection.create([selection])
          : EditorSelection.create(
              [EditorSelection.cursor(0), EditorSelection.cursor(from + 1)],
              0,
            ),
      });
      const retained = !['opening', 'outside', 'surrounding'].includes(position);
      assert.deepEqual(f.state.field(richProjection).revealed, retained ? { from, to } : null);
      assert.strictEqual(f.state.field(richProjection).all, prepared);
      assert.equal(readExactSource(f.state) === source, true);
      assert.equal(undoDepth(f.state), 0);
    });
