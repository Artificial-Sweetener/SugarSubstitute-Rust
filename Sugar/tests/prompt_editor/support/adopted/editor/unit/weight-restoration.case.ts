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
/** Spatial batch commands retain source, native caret/history and original focus intent. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, StateEffect } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { control, disposeQueueFixture } from './weight-command-queue-fixture.ts';
for (const specimen of [
  {
    kind: 'emphasis',
    source: '(cat:0.60), dog',
    from: 1,
    to: 4,
    expected: '(cat:0.75) , dog',
    head: 11,
  },
  {
    kind: 'lora',
    source: '<lora:missing:0.60>, tail',
    from: 0,
    to: 19,
    expected: '<lora:missing:0.75> , tail',
    head: 20,
  },
] as const)
  test(`${specimen.kind} Space publishes after-token source before restoring its original focus intent`, async () => {
    const native = control(specimen.source);
    try {
      assert.equal(
        native.owner.request(
          { kind: 'set', weight: '0.75' },
          {
            owner: 'exact',
            completion: 'space',
            neutral: false,
            kind: specimen.kind,
            range: { from: specimen.from, to: specimen.to },
            freshness: native.view.state.field(editorFreshness),
          },
          native.handoff,
        ),
        true,
      );
      await native.flush();
      assert.equal(readExactSource(native.view.state), specimen.expected);
      assert.equal(native.view.state.selection.main.head, specimen.head);
      assert.equal(undoDepth(native.view.state), 2);
      assert.equal(native.restored(), 1);
      assert.deepEqual(native.viewport, ['preserve-viewport']);
    } finally {
      native.dispose();
    }
  });

test('rapid spatial repeats publish the complete batch before one viewport-preserving restoration', async () => {
  const native = control('(cat:1.05), dog');
  try {
    const request = {
      owner: 'pointer',
      kind: 'emphasis',
      range: { from: 1, to: 4 },
      freshness: native.view.state.field(editorFreshness),
    } as const;
    for (let step = 0; step < 2; step++)
      assert.equal(
        native.owner.request(
          { kind: 'step', direction: 'decrease' },
          request,
          step === 0 ? native.handoff : native.repeatHandoff(),
        ),
        true,
      );
    await native.flush();
    assert.equal(readExactSource(native.view.state), '(cat:0.95), dog');
    assert.equal(undoDepth(native.view.state), 2);
    assert.equal(native.restored(), 1);
    assert.deepEqual(native.viewport, ['preserve-viewport']);
    assert.deepEqual(native.anchors, [
      { position: 8, offset: -13 },
      { position: 15, offset: -13 },
    ]);
  } finally {
    native.dispose();
  }
});

test('a spatial edit maps its captured native anchor through a length change without changing pixel displacement', async () => {
  const native = control('(cat:1.2), dog');
  try {
    native.owner.request(
      { kind: 'set', weight: '0.75' },
      {
        owner: 'exact',
        completion: 'keyboard',
        neutral: false,
        kind: 'emphasis',
        range: { from: 1, to: 4 },
        freshness: native.view.state.field(editorFreshness),
      },
      native.handoff,
    );
    await native.flush();
    assert.equal(readExactSource(native.view.state), '(cat:0.75), dog');
    assert.deepEqual(native.anchors, [{ position: 15, offset: -13 }]);
    assert.deepEqual(native.viewport, ['preserve-viewport']);
  } finally {
    native.dispose();
  }
});

test("the Space publication's newer reverse selection wins before any final focus dispatch", async () => {
  const native = control('(cat:0.60), dog');
  try {
    native.reenter(() => native.view.dispatch({ selection: { anchor: 4, head: 1 } }), 2);
    native.owner.request(
      { kind: 'set', weight: '0.75' },
      {
        owner: 'exact',
        completion: 'space',
        neutral: false,
        kind: 'emphasis',
        range: { from: 1, to: 4 },
        freshness: native.view.state.field(editorFreshness),
      },
      native.handoff,
    );
    await native.flush();
    assert.equal(readExactSource(native.view.state), '(cat:0.75) , dog');
    assert.deepEqual(
      [native.view.state.selection.main.anchor, native.view.state.selection.main.head],
      [4, 1],
    );
    assert.equal(native.restored(), 0);
    assert.equal(undoDepth(native.view.state), 2);
  } finally {
    native.dispose();
  }
});

test('teardown between the command task and native measurement performs no retired geometry read or publication', async () => {
  const native = control('(cat:1.20), dog');
  try {
    native.owner.request(
      { kind: 'step', direction: 'decrease' },
      {
        owner: 'pointer',
        kind: 'emphasis',
        range: { from: 1, to: 4 },
        freshness: native.view.state.field(editorFreshness),
      },
      native.handoff,
    );
    native.task();
    native.owner.destroy();
    await native.flush();
    assert.equal(native.snapshotReads(), 0);
    assert.equal(native.restored(), 0);
    assert.equal(readExactSource(native.view.state), '(cat:1.20), dog');
    assert.equal(undoDepth(native.view.state), 0);
  } finally {
    native.dispose();
  }
});

for (const later of ['intent', 'selection', 'source', 'baseline', 'permission', 'destroy'] as const)
  test(`exact Space stops after publication when a callback chooses newer ${later}`, async () => {
    const native = control('(cat:0.60), dog');
    try {
      const request = {
        owner: 'exact',
        completion: 'space',
        neutral: false,
        kind: 'emphasis',
        range: { from: 1, to: 4 },
        freshness: native.view.state.field(editorFreshness),
      } as const;
      native.reenter(() => {
        if (later === 'intent') native.supersede();
        else if (later === 'selection') native.view.dispatch({ selection: { anchor: 4, head: 1 } });
        else if (later === 'source')
          native.view.dispatch({
            changes: {
              from: 0,
              to: native.view.state.doc.length,
              insert: 'host',
            },
          });
        else if (later === 'baseline') native.baseline();
        else if (later === 'permission')
          native.view.dispatch({
            effects: StateEffect.appendConfig.of(EditorState.readOnly.of(true)),
          });
        else native.owner.destroy();
      });
      native.owner.request({ kind: 'set', weight: '0.75' }, request, native.handoff);
      await native.flush();
      assert.equal(native.restored(), 0);
      if (later === 'selection')
        assert.deepEqual(
          [native.view.state.selection.main.anchor, native.view.state.selection.main.head],
          [4, 1],
        );
      if (later === 'source') assert.equal(readExactSource(native.view.state), 'host');
      else if (later === 'baseline') assert.equal(readExactSource(native.view.state), 'baseline');
      else assert.equal(readExactSource(native.view.state), '(cat:0.75), dog');
      if (later === 'permission') assert.equal(native.view.state.readOnly, true);
    } finally {
      native.dispose();
    }
  });

test.after(disposeQueueFixture);
