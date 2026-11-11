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
/** Actual public coordinate reads may publish nested native updates; continuation must revalidate before using their result. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, StateEffect } from '@codemirror/state';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { heldTokenFixture, heldTokenPointer } from './held-token-fixture.ts';
import {
  exactReplacement,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  richProjection,
  installProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { heldProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-exposure.ts';

for (const path of ['token', 'point-anchor'] as const)
  for (const boundary of [
    'source',
    'context',
    'read-only',
    'host-selection',
    'disposal',
    'derived',
  ] as const)
    test(`${path} coordinate ${boundary} reentry returns the current native selection or continues a live source-neutral gesture`, () => {
      const fixture = heldTokenFixture();
      try {
        const { event, style } = fixture.press(heldTokenPointer(8, path === 'token' ? 2 : 1));
        fixture.select(style, event);
        const before = fixture.state.selection;
        fixture.duringRead(() => {
          if (boundary === 'source')
            fixture.dispatch({
              ...exactReplacement(fixture.state, 0, fixture.state.doc.length, 'x'),
              selection: { anchor: 1 },
            });
          else if (boundary === 'context')
            fixture.dispatch({ effects: invalidateCommandContext.of(null) });
          else if (boundary === 'read-only')
            fixture.dispatch({
              effects: StateEffect.appendConfig.of(EditorState.readOnly.of(true)),
            });
          else if (boundary === 'host-selection') fixture.dispatch({ selection: { anchor: 0 } });
          else if (boundary === 'disposal') fixture.destroy();
          else
            fixture.dispatch({
              effects: installProjection.of(fixture.state.field(richProjection)),
            });
        });
        const result = style.get(heldTokenPointer(22), false, false);
        if (boundary === 'derived') {
          assert.deepEqual(
            [result.main.anchor, result.main.head],
            path === 'token' ? [6, 25] : [8, 22],
          );
          assert.equal(fixture.state.selection.eq(before), true);
          assert.equal(fixture.state.field(heldProjection) !== null, path === 'token');
        } else {
          assert.equal(result.eq(fixture.state.selection), true);
          assert.deepEqual(
            [result.main.anchor, result.main.head],
            boundary === 'source'
              ? [1, 1]
              : boundary === 'host-selection'
                ? [0, 0]
                : path === 'token'
                  ? [6, 19]
                  : [8, 8],
          );
          if (boundary !== 'disposal') assert.equal(fixture.state.field(heldProjection), null);
        }
        assert.equal(
          readExactSource(fixture.state),
          boundary === 'source' ? 'x' : 'alpha <lora:real:1>, tail',
        );
        assert.deepEqual(
          [undoDepth(fixture.state), redoDepth(fixture.state)],
          boundary === 'source' ? [1, 0] : [0, 0],
        );
      } finally {
        fixture.destroy();
      }
    });

for (const path of ['token', 'point-anchor'] as const)
  test(`${path} supported coordinate exceptions retain their identity and do not retire a live gesture`, () => {
    const fixture = heldTokenFixture();
    try {
      const { event, style } = fixture.press(heldTokenPointer(8, path === 'token' ? 2 : 1));
      fixture.select(style, event);
      const cause = Symbol('coordinate-failure'),
        original = fixture.state.selection;
      fixture.duringRead(() => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw cause;
      });
      let failure: unknown,
        failed = false;
      try {
        style.get(heldTokenPointer(22), false, false);
      } catch (error) {
        failed = true;
        failure = error;
      }
      assert.equal(failed, true);
      assert.equal(failure, cause);
      assert.equal(fixture.state.selection.eq(original), true);
      const fresh = style.get(heldTokenPointer(22), false, false);
      assert.deepEqual([fresh.main.anchor, fresh.main.head], path === 'token' ? [6, 25] : [8, 22]);
    } finally {
      fixture.destroy();
    }
  });
