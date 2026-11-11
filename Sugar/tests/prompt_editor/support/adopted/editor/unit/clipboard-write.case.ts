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
/** Controlled actual write outputs prove one bounded physical clipboard lane without a native browser or editable state. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { BrowserClipboardWriter } from '../../../../../../frontend/sugarprompt/editor/src/browser-clipboard-write.ts';
import type { ClipboardWriteResult } from '../../../../../../frontend/sugarprompt/services/src/clipboard/contracts.ts';

test('one active clipboard write admits only the newest pending payload and preserves the actual idle barrier', async () => {
  const written: string[] = [];
  let release!: (result: ClipboardWriteResult) => void;
  const writer = new BrowserClipboardWriter((text) => {
    written.push(text);
    return written.length === 1
      ? new Promise((resolve) => {
          release = resolve;
        })
      : Promise.resolve({ status: 'written' });
  });
  const first = writer.submit('first'),
    idle = writer.settled();
  const skipped = writer.submit('intermediate'),
    latest = writer.submit('latest');
  assert.equal(writer.settled(), idle);
  assert.deepEqual(await skipped, { status: 'unavailable' });
  assert.deepEqual(written, ['first']);
  release({ status: 'written' });
  assert.deepEqual(await first, { status: 'written' });
  assert.deepEqual(await latest, { status: 'written' });
  await idle;
  assert.deepEqual(written, ['first', 'latest']);
  assert.equal(writer.settled(), undefined);
});

test('current asynchronous write failure retains its cause and releases the newest accepted write', async () => {
  const cause = new Error('controlled clipboard write failure');
  const written: string[] = [];
  let fail!: (cause: unknown) => void;
  const writer = new BrowserClipboardWriter((text) => {
    written.push(text);
    return written.length === 1
      ? new Promise((_resolve, reject) => {
          fail = reject;
        })
      : Promise.resolve({ status: 'written' });
  });
  const first = writer.submit('first'),
    latest = writer.submit('latest'),
    idle = writer.settled();
  fail(cause);
  await assert.rejects(first, (failure) => failure === cause);
  assert.deepEqual(await latest, { status: 'written' });
  await idle;
  assert.deepEqual(written, ['first', 'latest']);
  assert.equal(writer.settled(), undefined);
});

test('a synchronous native write invariant failure releases its payload and permits a later deliberate invocation', async () => {
  const cause = new Error('controlled synchronous clipboard failure');
  let failed = false;
  const writer = new BrowserClipboardWriter(() => {
    if (!failed) {
      failed = true;
      throw cause;
    }
    return Promise.resolve({ status: 'written' });
  });
  await assert.rejects(writer.submit('first'), (failure) => failure === cause);
  assert.equal(writer.settled(), undefined);
  assert.deepEqual(await writer.submit('next'), { status: 'written' });
  await writer.settled();
  assert.equal(writer.settled(), undefined);
});
