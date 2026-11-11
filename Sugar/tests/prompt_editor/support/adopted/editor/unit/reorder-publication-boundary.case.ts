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
/** The preview owner publishes actual native transactions without acquiring gesture, source or retirement authority. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Transaction } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { moveReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  installReorderPreview,
  publishReorderPreview,
  reorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { captureReorder } from '../transaction/reorder-fixture.ts';
import { fixture } from './reorder-session-fixture.ts';

for (const mode of ['native', 'pointer', 'moved'] as const)
  test(`complete preview publication owns preparation and source-neutral ${mode} paint`, () => {
    const f = fixture();
    try {
      const before = f.state,
        capture = captureReorder(before),
        token = {};
      const order =
        mode === 'moved'
          ? moveReorderChip(capture.prepared, capture.prepared.initial, 0, {
              kind: 'row',
              row: 0,
              insertion: 2,
            })!
          : capture.prepared.initial;
      let focused = 0,
        published = 0;
      f.view.focus = () => {
        focused++;
        assert.equal(published, 1);
      };
      f.observe((tx) => {
        if (!tx.effects.some((effect) => effect.is(installReorderPreview))) return;
        published++;
        assert.equal(tx.docChanged, false);
        assert.equal(tx.selection, undefined);
        assert.equal(tx.annotation(Transaction.addToHistory), false);
      });
      assert.equal(
        publishReorderPreview(
          f.view,
          { token, capture, order, activeChip: 0 },
          mode === 'pointer' ? 'pointer' : 'complete',
          null,
          () => true,
        ),
        'published',
      );
      const paint = f.state.field(reorderPreview);
      assert.ok(paint.value);
      assert.equal(paint.value.token, token);
      assert.equal(paint.value.capture, capture);
      assert.equal(paint.value.order, order);
      assert.equal(paint.value.output.text, mode === 'moved' ? 'b, c, a' : 'a, b, c');
      assert.equal(paint.native, mode !== 'moved');
      assert.equal(focused, mode !== 'moved' ? 1 : 0);
      assert.equal(f.state.doc, before.doc);
      assert.equal(f.state.selection, before.selection);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(undoDepth(f.state), 0);
      assert.equal(f.owner.blocksInput(), false);
    } finally {
      f.destroy();
    }
  });

for (const mode of ['stale', 'unavailable'] as const)
  test(`preview ${mode} outcome acquires no gesture or retirement authority`, () => {
    const f = fixture();
    try {
      const before = f.state,
        capture = captureReorder(before);
      const order =
        mode === 'unavailable'
          ? { ...capture.prepared.initial, order: [999] }
          : capture.prepared.initial;
      f.view.dispatch = () => {
        throw new Error('rejected-publication-dispatched');
      };
      f.view.focus = () => {
        throw new Error('rejected-publication-focused');
      };
      assert.equal(
        publishReorderPreview(
          f.view,
          { token: {}, capture, order, activeChip: 0 },
          'complete',
          null,
          () => mode !== 'stale',
        ),
        mode,
      );
      assert.equal(f.state, before);
      assert.equal(f.owner.blocksInput(), false);
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(undoDepth(f.state), 0);
    } finally {
      f.destroy();
    }
  });

test("preview outcome uses the gesture's live currentness after actual dispatch and never restores focus", () => {
  const f = fixture();
  try {
    const capture = captureReorder(f.state);
    let current = true;
    f.observe(() => {
      current = false;
    });
    f.view.focus = () => {
      throw new Error('stale-publication-focused');
    };
    assert.equal(
      publishReorderPreview(
        f.view,
        { token: {}, capture, order: capture.prepared.initial, activeChip: 0 },
        'complete',
        null,
        () => current,
      ),
      'stale',
    );
    assert.ok(f.state.field(reorderPreview).value);
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

for (const boundary of ['dispatch', 'focus'] as const)
  test(`preview ${boundary} failure remains observable to the owning gesture`, () => {
    const f = fixture(),
      cause = new Error('preview-publication-boundary-failed');
    try {
      const capture = captureReorder(f.state);
      if (boundary === 'dispatch')
        f.view.dispatch = () => {
          throw cause;
        };
      else
        f.view.focus = () => {
          throw cause;
        };
      assert.throws(
        () =>
          publishReorderPreview(
            f.view,
            {
              token: {},
              capture,
              order: capture.prepared.initial,
              activeChip: 0,
            },
            'complete',
            null,
            () => true,
          ),
        (error) => error === cause,
      );
      assert.equal(f.owner.blocksInput(), false);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(undoDepth(f.state), 0);
    } finally {
      f.destroy();
    }
  });
