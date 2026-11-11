/** @jest-environment node */
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
/** Qualify bounded Unicode chunking and cancellation against an admitted portable checkpoint. */
import { createHash } from 'node:crypto';
import {
  DesktopSessionTransfer,
  desktopSessionChunks,
} from '../../../frontend/comfyui/ui/workflow/application/DesktopSessionTransfer.js';
import { decodeDesktopCheckpoint } from '../../../integrations/shared/SessionDecode.js';
import { utf8Bytes } from '../../../integrations/shared/SessionPrimitives.js';
import type { SessionTransfer } from '../../../integrations/contracts/SessionProtocol.generated.js';
import { desktopSessionFixture } from '../support/desktop_session.js';

/** Hash encoded fixture bytes as the external cryptography boundary does. */
async function digest(text: string): Promise<string> {
  return createHash('sha256').update(text, 'utf8').digest('hex');
}

test('chunking preserves Unicode scalars and admits exact ordered bytes and identity', async () => {
  const text = JSON.stringify(desktopSessionFixture().checkpoint);
  const checkpoint = decodeDesktopCheckpoint(text)!;
  const chunks = desktopSessionChunks(text);
  const descriptor: SessionTransfer = {
    id: '00000000-0000-4000-8000-000000000099',
    checkpointId: checkpoint.id,
    revision: checkpoint.revision,
    bytes: utf8Bytes(text),
    chunks: chunks.length,
    digest: await digest(text),
    direction: 'materialize',
  };
  const transfer = new DesktopSessionTransfer(digest);
  transfer.begin('request', descriptor);
  expect(() => transfer.begin('another', descriptor)).toThrow('already active');
  expect(() => transfer.chunk('request', descriptor.id, 1, chunks[0])).toThrow('order');
  chunks.forEach((chunk, index) => transfer.chunk('request', descriptor.id, index, chunk));
  transfer.commit('request', descriptor.id);
  expect(await transfer.take('request', descriptor.id)).toEqual(checkpoint);
  const unicode = '🍬'.repeat(40000);
  const split = desktopSessionChunks(unicode);
  expect(split.every((chunk) => utf8Bytes(chunk) <= 65536)).toBe(true);
  expect(split.join('')).toBe(unicode);
});

test('cancelled delayed hashing cannot revive a materialization transfer', async () => {
  const text = JSON.stringify(desktopSessionFixture().checkpoint);
  const checkpoint = decodeDesktopCheckpoint(text)!;
  let finish: ((hash: string) => void) | null = null;
  const transfer = new DesktopSessionTransfer(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const chunks = desktopSessionChunks(text);
  const descriptor: SessionTransfer = {
    id: '00000000-0000-4000-8000-000000000099',
    checkpointId: checkpoint.id,
    revision: checkpoint.revision,
    bytes: utf8Bytes(text),
    chunks: chunks.length,
    digest: await digest(text),
    direction: 'materialize',
  };
  transfer.begin('request', descriptor);
  chunks.forEach((chunk, index) => transfer.chunk('request', descriptor.id, index, chunk));
  transfer.commit('request', descriptor.id);
  const pending = transfer.take('request', descriptor.id);
  transfer.cancel('request');
  if (!finish) throw new Error('Digest fixture was not entered.');
  const resolveDigest: (hash: string) => void = finish;
  resolveDigest(await digest(text));
  await expect(pending).rejects.toThrow('cancelled');
});
