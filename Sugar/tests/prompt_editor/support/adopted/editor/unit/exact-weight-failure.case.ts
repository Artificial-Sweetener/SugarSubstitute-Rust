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
/** Actual queue phase failures preserve ordered causes and cannot retire newer reentrant commands. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect, Transaction } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  exactWeightDraft,
  setExactWeightDraft,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-weight-draft.ts';
import { nativeInputSettlement } from '../../../../../../frontend/sugarprompt/editor/src/input-settlement.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { control, disposeQueueFixture } from './weight-command-queue-fixture.ts';

/** Install actual draft metadata through its native state owner, with no draft text buffer. */
function open(native: ReturnType<typeof control>) {
  const key = Symbol('finite draft'),
    request = {
      owner: 'exact',
      completion: 'keyboard',
      neutral: false,
      kind: 'emphasis',
      range: { from: 1, to: 4 },
      freshness: native.view.state.field(editorFreshness),
    } as const;
  native.view.dispatch({
    effects: setExactWeightDraft.of({ ...request, key, phase: 'editing' }),
    annotations: Transaction.addToHistory.of(false),
  });
  return { key, request };
}

for (const reentry of [false, true])
  for (const cleanupFails of [false, true])
    test(`phase failure retains its cause with cleanup failure ${cleanupFails} and newer queue reentry ${reentry}`, async () => {
      const native = control('(cat:1.20), tail'),
        primary = new Error('phase publication failure'),
        secondary = new Error('settlement cleanup failure');
      let cancellations = 0,
        finishes = 0,
        reservations = 0;
      let newer: ReturnType<typeof open> | null = null;
      try {
        native.view.dispatch({
          effects: StateEffect.appendConfig.of(
            nativeInputSettlement.of({
              hasCompositionInput: () => false,
              cancelCompositionInput: () => {},
              hasHeldKey: () => false,
              reserveCommand: () => {
                reservations++;
                return {
                  finish: () => {
                    finishes++;
                  },
                  cancel: () => {
                    cancellations++;
                    if (reentry && cancellations === 1) {
                      newer = open(native);
                      assert.equal(
                        native.owner.exact(
                          { kind: 'set', weight: '222.33' },
                          { ...newer.request, completion: 'outside' },
                          native.handoff,
                          newer.key,
                        ),
                        true,
                      );
                    }
                    if (cleanupFails) throw secondary;
                  },
                };
              },
            }),
          ),
        });
        const { key, request } = open(native),
          initialSelection = native.view.state.selection;
        native.reenter(() => {
          throw primary;
        });
        assert.throws(
          () => native.owner.exact({ kind: 'set', weight: '111.22' }, request, native.handoff, key),
          (cause) => {
            if (!cleanupFails) return cause === primary;
            assert.equal(cause instanceof AggregateError, true);
            assert.deepEqual((cause as AggregateError).errors, [primary, secondary]);
            return true;
          },
        );
        assert.equal(readExactSource(native.view.state), '(cat:1.20), tail');
        assert.equal(native.view.state.selection.eq(initialSelection), true);
        assert.equal(undoDepth(native.view.state), 0);
        assert.equal(native.restored(), 0);
        assert.equal(cancellations, 1);
        await native.flush();
        if (reentry) {
          assert.notEqual(newer, null);
          assert.equal(native.view.state.field(exactWeightDraft)?.key, newer!.key);
          assert.equal(native.view.state.field(exactWeightDraft)?.phase, 'awaiting-commit');
          assert.equal(readExactSource(native.view.state), '(cat:1.20), tail');
          await native.flush();
          assert.equal(readExactSource(native.view.state), '(cat:222.33), tail');
          // Finishing by clicking elsewhere keeps the caret the click placed, mapped past the edit.
          assert.equal(native.view.state.selection.main.head, 18);
          assert.equal(undoDepth(native.view.state), 1);
          assert.equal(native.restored(), 0);
          assert.equal(reservations, 2);
          assert.equal(finishes, 1);
        } else {
          assert.equal(readExactSource(native.view.state), '(cat:1.20), tail');
          assert.equal(undoDepth(native.view.state), 0);
          assert.equal(finishes, 0);
          assert.equal(reservations, 1);
        }
        assert.equal(native.view.state.field(exactWeightDraft), null);
        await native.flush();
        assert.equal(undoDepth(native.view.state), reentry ? 1 : 0);
        assert.equal(cancellations, 1);
      } finally {
        native.dispose();
      }
    });

test.after(disposeQueueFixture);
