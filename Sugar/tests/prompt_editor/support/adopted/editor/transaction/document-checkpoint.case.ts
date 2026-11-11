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
/** Independent native document histories restore through current complete construction and authenticated host adoption. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, Transaction } from '@codemirror/state';
import { undo, redo, undoDepth } from '@codemirror/commands';
import type { EditorView } from '@codemirror/view';
import { checkpointFixture } from './document-checkpoint-fixture.ts';
import { exactReplacement } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  installAuthorship,
  authorship,
} from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import { preparedAuthorship } from '../../../../../../frontend/sugarprompt/editor/src/authorship-ranges.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { nativeDocumentSeed } from '../../../../../../frontend/sugarprompt/editor/src/document-state.ts';
import {
  loraMedia,
  installLoraMedia,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import type { DocumentCheckpoint } from '../../../../../../frontend/sugarprompt/editor/src/document-checkpoint.ts';

test('two retained native documents keep independent exact newline histories, directional selections and authorship under current configuration', async () => {
  const firstSource = '🙂\r\nold\rtail\n',
    firstEdit = 'edited\nvalue\r';
  const f = checkpointFixture(firstSource);
  f.transport.dispatch({
    selection: EditorSelection.range(6, 3),
    effects: installAuthorship.of(preparedAuthorship([], [{ from: 0, to: 2 }])),
    annotations: Transaction.addToHistory.of(false),
  });
  const firstAuthorship = f.state().field(authorship);
  f.transport.dispatch({
    ...exactReplacement(f.state(), 0, firstSource.length, firstEdit),
    userEvent: 'input',
  });
  const first = await f.capture();
  const firstHistory = f.state();
  assert.equal(first.source === firstEdit, true);
  assert.deepEqual(Object.keys(nativeDocumentSeed(f.state())).sort(), [
    'authorship',
    'doc',
    'endings',
    'history',
    'selection',
  ]);
  assert.equal(
    f.updates.apply({
      mode: 'baseline',
      source: 'second\r\n',
      configuration: {
        documentKind: 'wildcard-csv',
        richRendering: false,
        readOnly: false,
      },
    }).status,
    'applied',
  );
  f.transport.dispatch({
    ...exactReplacement(f.state(), 0, 'second\r\n'.length, 'second-edit\r\n'),
    selection: EditorSelection.range(5, 2),
    userEvent: 'input',
  });
  const second = await f.capture();
  const freshBefore = f.state().field(editorFreshness);
  assert.equal(f.updates.apply({ mode: 'restore-document', checkpoint: first }).status, 'applied');
  assert.equal(f.source() === firstEdit, true);
  assert.equal(f.state().facet(documentKind), 'wildcard-csv');
  assert.equal(f.state().facet(richRendering), false);
  assert.notEqual(f.state().field(editorFreshness), freshBefore);
  assert.notEqual(f.state(), firstHistory);
  assert.equal(undoDepth(f.state()), 1);
  const view = f.transport as unknown as EditorView;
  assert.equal(undo(view), true);
  assert.equal(f.source() === firstSource, true);
  assert.equal(f.state().field(authorship), firstAuthorship);
  assert.deepEqual([f.state().selection.main.anchor, f.state().selection.main.head], [6, 3]);
  assert.equal(redo(view), true);
  assert.equal(f.source() === firstEdit, true);
  assert.equal(
    f.updates.apply({
      mode: 'restore-document',
      checkpoint: second,
      configuration: { readOnly: true },
    }).status,
    'applied',
  );
  assert.equal(f.source() === 'second-edit\r\n', true);
  assert.deepEqual([f.state().selection.main.anchor, f.state().selection.main.head], [5, 2]);
  assert.equal(f.state().readOnly, true);
  assert.equal(undoDepth(f.state()), 1);
  assert.equal(
    f.updates.apply({ mode: 'preserve', configuration: { readOnly: false } }).status,
    'applied',
  );
  assert.equal(undo(view), true);
  assert.equal(f.source() === 'second\r\n', true);
  assert.equal(first.source === firstEdit, true);
  assert.equal(second.source === 'second-edit\r\n', true);
  first.release();
  second.release();
  f.dispose();
  assert.deepEqual(f.counts(), { finished: 2, cancelled: 2 });
});

test('foreign, released, disposed and composing restore admissions preserve the old document', async () => {
  const f = checkpointFixture(),
    other = checkpointFixture('other');
  const handle = await f.capture();
  const old = other.state();
  assert.deepEqual(other.updates.apply({ mode: 'restore-document', checkpoint: handle }), {
    status: 'rejected',
    reason: 'foreign-checkpoint',
  });
  assert.equal(other.state(), old);
  f.transport.compositionStarted = true;
  const ownState = f.state();
  assert.deepEqual(f.updates.apply({ mode: 'restore-document', checkpoint: handle }), {
    status: 'rejected',
    reason: 'composing',
  });
  assert.equal(f.state(), ownState);
  f.transport.compositionStarted = false;
  handle.release();
  handle.release();
  assert.deepEqual(f.updates.apply({ mode: 'restore-document', checkpoint: handle }), {
    status: 'rejected',
    reason: 'released-checkpoint',
  });
  assert.equal(f.state(), ownState);
  assert.throws(() => handle.source, /checkpoint unavailable/u);
  const live = await other.capture();
  other.dispose();
  assert.throws(() => live.source, /checkpoint unavailable/u);
  assert.deepEqual(other.updates.apply({ mode: 'restore-document', checkpoint: live }), {
    status: 'rejected',
    reason: 'disposed',
  });
  f.dispose();
});

test('configuration preparation reentry and handle release reject before stale document adoption', async () => {
  const f = checkpointFixture();
  const handle = await f.capture();
  const released = f.updates.apply({
    mode: 'restore-document',
    checkpoint: handle,
    configuration: {
      get placeholder() {
        handle.release();
        return 'fresh';
      },
    },
  });
  assert.deepEqual(released, {
    status: 'rejected',
    reason: 'released-checkpoint',
  });
  assert.equal(f.source() === 'initial', true);
  const current = await f.capture();
  let entered = false;
  const result = f.updates.apply({
    mode: 'restore-document',
    checkpoint: current,
    configuration: {
      get placeholder() {
        if (!entered) {
          entered = true;
          f.updates.apply({ mode: 'baseline', source: 'newer' });
        }
        return 'stale';
      },
    },
  });
  assert.deepEqual(result, { status: 'rejected', reason: 'superseded' });
  assert.equal(f.source() === 'newer', true);
  current.release();
  f.dispose();
});

test('explicit restored selection uses exact-source boundaries and rejects CRLF interiors before adoption', async () => {
  const f = checkpointFixture('a\r\nb');
  const handle = await f.capture();
  const state = f.state();
  assert.throws(
    () =>
      f.updates.apply({
        mode: 'restore-document',
        checkpoint: handle,
        selection: { anchor: 2, head: 2 },
      }),
    RangeError,
  );
  assert.equal(f.state(), state);
  assert.equal(
    f.updates.apply({
      mode: 'restore-document',
      checkpoint: handle,
      selection: { anchor: 4, head: 0 },
    }).status,
    'applied',
  );
  assert.deepEqual([f.state().selection.main.anchor, f.state().selection.main.head], [3, 0]);
  handle.release();
  f.dispose();
});

test('accepted host or source intent supersedes pending capture; queued measurements become inert', async () => {
  const f = checkpointFixture();
  const pending = f.updates.captureDocument();
  f.updates.apply({ mode: 'baseline', source: 'newer' });
  assert.deepEqual(await pending, { status: 'rejected', reason: 'superseded' });
  f.flush();
  assert.deepEqual(f.counts(), { finished: 0, cancelled: 1 });
  assert.equal(f.source() === 'newer', true);
  const disposed = f.updates.captureDocument();
  f.dispose();
  assert.deepEqual(await disposed, { status: 'rejected', reason: 'disposed' });
  f.flush();
  assert.deepEqual(f.counts(), { finished: 0, cancelled: 2 });
});

test('restoration constructs fresh derived media and rejects an old source-bound publication', async () => {
  const f = checkpointFixture();
  const previousFreshness = f.state().field(editorFreshness),
    previousMedia = f.state().field(loraMedia);
  const handle = await f.capture();
  assert.equal(f.updates.apply({ mode: 'baseline', source: 'other' }).status, 'applied');
  assert.equal(f.updates.apply({ mode: 'restore-document', checkpoint: handle }).status, 'applied');
  const currentMedia = f.state().field(loraMedia);
  assert.notEqual(currentMedia, previousMedia);
  assert.notEqual(f.state().field(editorFreshness), previousFreshness);
  f.transport.dispatch({
    effects: installLoraMedia.of({
      freshness: previousFreshness,
      media: previousMedia,
    }),
    annotations: Transaction.addToHistory.of(false),
  });
  assert.equal(f.state().field(loraMedia), currentMedia);
  assert.equal(f.source() === 'initial', true);
  handle.release();
  f.dispose();
});

test('forged checkpoint accessors are never evaluated during authentication', () => {
  const f = checkpointFixture();
  // This unknown external identity must be rejected before any source/release accessor is read.
  const checkpoint = {
    get source() {
      return assert.fail('FORGED_SOURCE_READ');
    },
    get release() {
      return assert.fail('FORGED_RELEASE_READ');
    },
  } as unknown as DocumentCheckpoint;
  const original = f.state();
  assert.deepEqual(f.updates.apply({ mode: 'restore-document', checkpoint }), {
    status: 'rejected',
    reason: 'foreign-checkpoint',
  });
  assert.equal(f.state(), original);
  f.dispose();
});
