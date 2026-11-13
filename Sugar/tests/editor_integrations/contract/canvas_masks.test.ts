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
/** Prove optional mask transport, coverage fidelity and operation lifetimes across capable and limited hosts. */
import { readFileSync } from 'node:fs';
import { jest } from '@jest/globals';
import { CanvasMaskClient } from '../../../integrations/shared/CanvasMaskClient.js';
import { nativeCanvasChanges } from '../../../integrations/shared/CanvasChangeRelay.js';
import { CanvasMaskServer } from '../../../integrations/shared/CanvasMaskServer.js';
import {
  encodeMaskPixels,
  decodeMaskPixels,
} from '../../../integrations/shared/CanvasMaskPixels.js';
import { masksBindings } from '../../../scripts/integrations/generate-masks-contract.js';
import type {
  CanvasMaskMessage,
  CanvasMaskCommand,
  CanvasMaskResult,
  CanvasMaskChanged,
} from '../../../integrations/contracts/CanvasMasksProtocol.generated.js';
const session = '00000000-0000-4000-8000-000000000001';

/** Connect a real shared client/server without browser or native editor dependencies. */
function fixture(supported = true, observesChanges = supported) {
  const sent: CanvasMaskMessage[] = [];
  const errors: unknown[] = [];
  const changes = new Set<(change: CanvasMaskChanged) => void>();
  const execute = jest.fn(
    async (_command: CanvasMaskCommand): Promise<CanvasMaskResult> => ({
      observesChanges,
      kind: 'context',
      supported,
      document: null,
    }),
  );
  const host = new CanvasMaskServer(session, {
    changes: observesChanges
      ? nativeCanvasChanges((listener) => {
          changes.add(listener);
          return () => {
            changes.delete(listener);
          };
        })
      : null,
    execute,
    send: (value) => client.receive(value),
    report: (error) => errors.push(error),
  });
  const client = new CanvasMaskClient(session, {
    report: (error) => errors.push(error),
    send: (value) => {
      sent.push(value);
      host.receive(value);
    },
    later: (ms, callback) => {
      const timer = setTimeout(callback, ms);
      return () => clearTimeout(timer);
    },
  });
  return { client, host, sent, execute, errors };
}

test.each([true, false])(
  'negotiates mask support explicitly without assuming capabilities from host identity: %s',
  async (supported) => {
    const f = fixture(supported);
    expect(await f.client.execute({ type: 'context' })).toEqual({
      observesChanges: supported,
      kind: 'context',
      supported,
      document: null,
    });
    f.client.dispose();
    f.host.dispose();
    expect(f.errors).toEqual([]);
  },
);

test('captures masks without native observation and negotiates the limitation independently', async () => {
  const f = fixture(true, false);
  const lease = f.client.changes.subscribe(jest.fn());
  expect(await lease.ready).toBe('unsupported');
  const mask = {
    canvasId: 'editor',
    document: { id: 'doc', geometryRevision: '1', width: 2, height: 1 },
    id: 'mask',
  };
  const pixels = encodeMaskPixels(2, 1, Uint8Array.from([64, 255]));
  f.execute.mockResolvedValueOnce({
    kind: 'captured',
    snapshots: [{ mask, revision: '2', pixels }],
  });
  expect(await f.client.execute({ type: 'capture', masks: [mask], maximumSize: 0 })).toEqual({
    kind: 'captured',
    snapshots: [{ mask, revision: '2', pixels }],
  });
  expect(f.execute).toHaveBeenCalledTimes(1);
  lease.stop();
  f.client.dispose();
  f.host.dispose();
  expect(f.errors).toEqual([]);
});

test('preserves all coverage values including feathering, holes and non-multiple-of-three rasters', () => {
  for (const size of [1, 2, 3, 256, 12287, 12288, 12289]) {
    const bytes = Uint8Array.from({ length: size }, (_, index) => index % 256);
    expect(decodeMaskPixels(encodeMaskPixels(size, 1, bytes))).toEqual(bytes);
  }
  expect(() => decodeMaskPixels({ width: 2, height: 1, data: 'AA==' })).toThrow();
  expect(() => decodeMaskPixels({ width: 1, height: 1, data: '!!!!' })).toThrow();
});

test('replays acknowledgements without repeating native operations and rejects request identity reuse', async () => {
  const f = fixture();
  await f.client.execute({ type: 'context' });
  f.host.receive(f.sent[0]);
  expect(f.execute).toHaveBeenCalledTimes(1);
  f.host.receive({ ...f.sent[0], command: { type: 'release', mask: null } });
  expect(f.execute).toHaveBeenCalledTimes(1);
  expect(f.host.receive({ ...f.sent[0], session: 'wrong' })).toBe(false);
  f.client.dispose();
  f.host.dispose();
});

test('settles pending work and prevents queued native calls after teardown', async () => {
  const f = fixture();
  const pending = f.client.execute({ type: 'context' });
  f.host.dispose();
  f.client.dispose();
  expect(await pending).toMatchObject({ kind: 'failure', status: 'unavailable' });
  await Promise.resolve();
  expect(f.execute).not.toHaveBeenCalled();
});

test('publishes only validated committed changes within the correct session', async () => {
  const f = fixture();
  const changed = jest.fn();
  const lease = f.client.changes.subscribe(changed);
  expect(await lease.ready).toBe('observing');
  const message = {
    protocol: 'sugarcubes.canvas.masks',
    version: 2,
    session,
    type: 'changed',
    documentId: 'doc',
    maskId: 'mask',
    revision: '2',
  };
  expect(f.client.receive({ ...message, extra: 'untrusted' })).toBe(false);
  expect(f.client.receive(message)).toBe(true);
  expect(changed).toHaveBeenCalledTimes(1);
  f.client.dispose();
  expect(f.client.receive(message)).toBe(false);
  f.host.dispose();
});

test('keeps masks bindings synchronized with the shared document schema', async () => {
  const files = await masksBindings(
    readFileSync('integrations/contracts/masks.schema.json', 'utf8'),
    readFileSync('integrations/contracts/canvas.schema.json', 'utf8'),
  );
  for (const [path, source] of Object.entries(files))
    expect(readFileSync(path, 'utf8').replaceAll('\r\n', '\n')).toBe(source);
});
