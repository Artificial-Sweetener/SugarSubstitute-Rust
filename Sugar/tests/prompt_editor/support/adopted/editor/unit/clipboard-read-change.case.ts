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
/** A system clipboard that changes during an asynchronous read never drops the trusted paste text. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { browserClipboard } from '../../../../../../frontend/sugarprompt/editor/src/browser-clipboard.ts';

/** Build a window whose clipboard items reject reads the way Chromium does after a clipboard change. */
function changingClipboard(failure: Error): Window & typeof globalThis {
  const item = {
    types: ['text/plain'],
    getType: () => Promise.reject(failure),
  };
  return {
    DOMException,
    navigator: { clipboard: { read: () => Promise.resolve([item]) } },
  } as unknown as Window & typeof globalThis;
}

test('a clipboard change during the read falls back to the trusted paste event text', async () => {
  const gateway = browserClipboard(
    changingClipboard(new DOMException('Clipboard data has changed', 'InvalidStateError')),
  );
  const signal = new AbortController().signal;
  assert.deepEqual(await gateway.read(signal, { kind: 'plain', text: 'blue eyes' }), {
    status: 'read',
    value: { kind: 'plain', text: 'blue eyes' },
  });
  assert.deepEqual(await gateway.read(signal), { status: 'unavailable' });
});

test('an unexpected clipboard read failure stays observable', async () => {
  const cause = new TypeError('controlled clipboard failure');
  const gateway = browserClipboard(changingClipboard(cause));
  await assert.rejects(
    gateway.read(new AbortController().signal, { kind: 'plain', text: 'blue eyes' }),
    (failure) => failure === cause,
  );
});
