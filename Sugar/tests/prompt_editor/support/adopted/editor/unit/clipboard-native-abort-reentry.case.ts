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
/** A real host abort reentry must preserve the newer clipboard request across an older native blur. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Transaction, type TransactionSpec } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { undoDepth, redoDepth } from '@codemirror/commands';
import {
  NativeClipboard,
  clipboardCommands,
  type ClipboardCommandOutcome,
} from '../../../../../../frontend/sugarprompt/editor/src/native-clipboard.ts';
import { booruPostLookup } from '../../../../../../frontend/sugarprompt/editor/src/booru-paste-host.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import { authorshipExtension } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import {
  promptHistory,
  historyTransactions,
} from '../../../../../../frontend/sugarprompt/editor/src/prompt-history.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  defaultPreferences,
  resolveFeatureProfile,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import type { BooruPostLookup } from '../../../../../../frontend/sugarprompt/services/src/booru/links.ts';
import { structureState } from '../transaction/document-structure-fixture.ts';
import { booruReply } from '../transaction/booru-paste-import-fixture.ts';

/** This finite boundary delegates actual ingestion/history and requests; only native I/O and focus are controlled. */
function abortReentryFixture() {
  let lookupEntered!: () => void,
    readEntered!: () => void,
    releaseLookup!: (value: unknown) => void,
    releaseRead!: () => void;
  const enteredLookup = new Promise<void>((resolve) => {
      lookupEntered = resolve;
    }),
    enteredRead = new Promise<void>((resolve) => {
      readEntered = resolve;
    }),
    lookupGate = new Promise<unknown>((resolve) => {
      releaseLookup = resolve;
    }),
    readGate = new Promise<void>((resolve) => {
      releaseRead = resolve;
    });
  const reported: unknown[] = [],
    tasks: Promise<unknown>[] = [];
  let focused = true,
    disposed = false,
    aborts = 0,
    reads = 0,
    delivered = false;
  let latest: Promise<
    | { readonly status: 'value'; readonly value: ClipboardCommandOutcome }
    | { readonly status: 'error'; readonly cause: unknown }
  > | null = null;
  const lookup: BooruPostLookup = (_key, signal) => {
    signal.addEventListener(
      'abort',
      () => {
        aborts++;
        latest = owner.execute('paste').then(
          (value) => ({ status: 'value' as const, value }),
          (cause: unknown) => ({ status: 'error' as const, cause }),
        );
      },
      { once: true },
    );
    tasks.push(lookupGate);
    lookupEntered();
    return lookupGate;
  };
  const clipboard = {
    /** Deliberately ignore abort, exercising the actual session's finite retirement outcome. */
    read() {
      reads++;
      const task = (async () => {
        readEntered();
        await readGate;
        return [
          {
            types: ['text/plain'],
            getType: () => Promise.resolve(new Blob(['late replacement'])),
          },
        ];
      })();
      tasks.push(task);
      return task;
    },
  };
  let state = structureState('', 'prompt', [
    authorshipExtension,
    promptHistory,
    richRendering.of(false),
    featureProfile.of(resolveFeatureProfile(defaultPreferences, { mode: 'library' })),
    booruPostLookup.of(lookup),
    clipboardCommands(() => undefined, {
      observe() {},
      error(cause) {
        reported.push(cause);
      },
    }),
  ]).state;
  const boundary: unknown = Object.create(EditorView.prototype);
  if (!(boundary instanceof EditorView))
    throw new Error('Clipboard native view fixture unavailable');
  Object.defineProperties(boundary, {
    state: { get: () => state },
    dom: {
      value: {
        ownerDocument: {
          defaultView: { navigator: { clipboard }, DOMException },
        },
      },
    },
    composing: { value: false },
    compositionStarted: { value: false },
    hasFocus: { get: () => focused },
    dispatch: {
      value: (input: Transaction | TransactionSpec) => {
        const transaction = input instanceof Transaction ? input : state.update(input);
        assert.equal(transaction.startState, state);
        state = transaction.state;
        // Only public ViewUpdate observations consumed by NativeClipboard are supplied at this fixture boundary.
        owner.update({
          startState: transaction.startState,
          state,
          transactions: [transaction],
          focusChanged: false,
        });
        for (const followup of historyTransactions([transaction]).slice(1)) {
          assert.equal(followup.startState, state);
          state = followup.state;
          owner.update({
            startState: followup.startState,
            state,
            transactions: [followup],
            focusChanged: false,
          });
        }
      },
    },
  });
  const owner = new NativeClipboard(boundary);
  return {
    owner,
    enteredLookup,
    enteredRead,
    state: () => state,
    aborts: () => aborts,
    reads: () => reads,
    /** Native blur precedes delayed CM focus update and must keep the captured older command identity. */
    blur() {
      focused = false;
      owner.blur();
    },
    /** Observe the one synchronously reentered public command without exposing its private request lane. */
    async newerOutcome(): Promise<ClipboardCommandOutcome> {
      assert.ok(latest);
      const result = await latest;
      delivered = true;
      if (result.status === 'error') throw result.cause;
      return result.value;
    },
    /** Retire the public owner before releasing all ignored-abort work; every teardown attempt retains ordered raw causes. */
    async dispose() {
      if (disposed) return;
      disposed = true;
      const causes: unknown[] = [];
      try {
        owner.destroy();
      } catch (cause) {
        causes.push(cause);
      }
      try {
        releaseLookup(booruReply());
      } catch (cause) {
        causes.push(cause);
      }
      try {
        releaseRead();
      } catch (cause) {
        causes.push(cause);
      }
      for (const task of tasks) {
        try {
          await task;
        } catch (cause) {
          causes.push(cause);
        }
      }
      if (latest) {
        const result = await latest;
        if (result.status === 'error' && !delivered) causes.push(result.cause);
      }
      causes.push(...reported);
      if (causes.length === 1) throw causes[0];
      if (causes.length)
        throw new AggregateError(causes, 'Clipboard abort reentry fixture cleanup failed');
    },
  };
}

test('host import abort reentry preserves the newer ignored-abort paste until native destruction', async () => {
  const c = abortReentryFixture(),
    causes: unknown[] = [];
  const literal = 'https://danbooru.donmai.us/posts/123';
  try {
    assert.equal(c.owner.fromPasteImport(literal), true);
    await c.enteredLookup;
    const accepted = c.state();
    assert.equal(readExactSource(accepted) === literal, true);
    assert.deepEqual([undoDepth(accepted), redoDepth(accepted)], [1, 0]);
    c.blur();
    assert.equal(c.aborts(), 1);
    await c.enteredRead;
    assert.equal(c.reads(), 1);
    assert.equal(c.state(), accepted);
    c.owner.destroy();
    assert.deepEqual(await c.newerOutcome(), { status: 'disposed' });
    assert.equal(c.state(), accepted);
    assert.deepEqual([undoDepth(c.state()), redoDepth(c.state())], [1, 0]);
    await c.dispose();
    assert.equal(c.state(), accepted);
  } catch (cause) {
    causes.push(cause);
  } finally {
    try {
      await c.dispose();
    } catch (cause) {
      causes.push(cause);
    }
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'Clipboard abort reentry control failed');
});
