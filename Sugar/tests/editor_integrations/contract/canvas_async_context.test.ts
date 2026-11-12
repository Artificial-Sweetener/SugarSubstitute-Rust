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
/** @jest-environment node */
/** Prove native context queries can cross processes without weakening mutation lifetime ownership. */
import { jest } from '@jest/globals';
import { CanvasServer } from '../../../integrations/shared/CanvasServer.js';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import type {
  CanvasContext,
  CanvasMessage,
  CanvasInsertResult,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';

const session = '00000000-0000-4000-8000-000000000001';
const context: CanvasContext = {
  canvas: {
    id: 'native-editor',
    label: 'Native editor',
    capabilities: { displayOutput: false, insertImageLayer: true, positionImageLayer: true },
  },
  document: { id: 'document', geometryRevision: 'geometry', width: 8, height: 6 },
};

/** Hold one native response without timers or a browser dependency. */
function deferred<T>() {
  let resolve: (value: T) => void = () => {
    throw new Error('The deferred response is not initialized.');
  };
  let reject: (reason: unknown) => void = () => {
    throw new Error('The deferred response is not initialized.');
  };
  const promise = new Promise<T>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  // Observe deliberate fixture rejection even when characterizing a host that never awaits it.
  void promise.catch(() => {});
  return { promise, resolve, reject };
}

/** Observe dispatch without a second client masking malformed host responses. */
function host() {
  const pending = deferred<CanvasContext>();
  const replies: CanvasMessage[] = [];
  const read = jest.fn(() => pending.promise);
  const report = jest.fn();
  const insert = jest.fn(
    async (): Promise<CanvasInsertResult> => ({
      status: 'cancelled',
      message: 'Fixture insertion complete.',
    }),
  );
  const server = new CanvasServer(session, {
    context: read,
    insert,
    send: (reply) => replies.push(reply),
    report,
  });
  return { pending, replies, read, insert, report, server };
}

/** Allow dispatch continuations to finish after resolving or rejecting their native response. */
async function settle(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

/** Keep requests on the existing authenticated, correlated wire contract. */
function request(type: 'context.request' | 'source.request' | 'image.insert'): CanvasMessage {
  const envelope = { protocol: 'sugarcubes.canvas', version: 3, session, requestId: 1 } as const;
  if (type === 'context.request') return { ...envelope, type };
  if (type === 'source.request') return { ...envelope, type, command: { type: 'context' } };
  return {
    ...envelope,
    type,
    image: {
      document: { id: 'document', geometryRevision: 'geometry', width: 8, height: 6 },
      image: { format: 'png', encoding: 'base64', data: 'fixture-png' },
      layerName: 'Result',
      position: { left: 2, top: 1 },
    },
  };
}

test('waits for fresh native context and coalesces a duplicate pending request', async () => {
  const f = host();
  try {
    f.server.receive(request('context.request'));
    f.server.receive(request('context.request'));
    expect(f.read).toHaveBeenCalledTimes(1);
    expect(f.replies).toEqual([]);
    f.pending.resolve(context);
    await settle();
    expect(f.replies).toEqual([{ ...request('context.request'), type: 'context.reply', context }]);
  } finally {
    f.server.dispose();
  }
});

test('awaits native document identity when source capture is unsupported', async () => {
  const f = host();
  try {
    f.server.receive(request('source.request'));
    expect(f.replies).toEqual([]);
    f.pending.resolve(context);
    await settle();
    expect(f.replies[0]).toMatchObject({
      type: 'source.reply',
      result: { kind: 'context', supported: false, document: context.document },
    });
  } finally {
    f.server.dispose();
  }
});

test.each([true, false])(
  'checks resolved insertion capabilities before mutating: %s',
  async (supported) => {
    const f = host();
    try {
      f.server.receive(request('image.insert'));
      expect(f.insert).not.toHaveBeenCalled();
      f.pending.resolve({
        ...context,
        canvas: {
          ...context.canvas,
          capabilities: { ...context.canvas.capabilities, positionImageLayer: supported },
        },
      });
      await settle();
      expect(f.insert).toHaveBeenCalledTimes(supported ? 1 : 0);
      if (!supported)
        expect(f.replies[0]).toMatchObject({
          type: 'image.reply',
          result: { status: 'unsupported' },
        });
    } finally {
      f.server.dispose();
    }
  },
);

test.each(['cancel', 'dispose'] as const)(
  'never begins insertion after %s during context lookup',
  async (action) => {
    const f = host();
    try {
      f.server.receive(request('image.insert'));
      if (action === 'dispose') f.server.dispose();
      else f.server.receive({ ...request('context.request'), type: 'cancel' });
      f.pending.resolve(context);
      await settle();
      expect(f.insert).not.toHaveBeenCalled();
      expect(f.replies).toEqual(
        action === 'dispose'
          ? []
          : [
              expect.objectContaining({
                type: 'image.reply',
                result: { status: 'cancelled', message: 'Canvas insertion cancelled.' },
              }),
            ],
      );
    } finally {
      f.server.dispose();
    }
  },
);

test.each(['context.request', 'source.request', 'image.insert'] as const)(
  'contains a rejected native context for %s',
  async (type) => {
    const f = host();
    try {
      f.server.receive(request(type));
      const failure = new Error('Native process disconnected.');
      f.pending.reject(failure);
      await settle();
      expect(f.report).toHaveBeenCalledWith(failure);
      expect(f.insert).not.toHaveBeenCalled();
      expect(f.replies).toHaveLength(1);
      expect(f.replies[0]).toMatchObject(
        type === 'context.request'
          ? { type: 'error.reply', failure: { status: 'error' } }
          : {
              type: type === 'source.request' ? 'source.reply' : 'image.reply',
              result: { status: 'error' },
            },
      );
    } finally {
      f.server.dispose();
    }
  },
);

test('does not roll cached capabilities back when older native context resolves last', async () => {
  const sent: CanvasMessage[] = [];
  const client = new CanvasClient(session, {
    send: (message) => sent.push(message),
    later: () => () => {},
    report: () => {},
  });
  try {
    const first = client.readContext(),
      second = client.readContext();
    const newer: CanvasContext = { ...context, canvas: { ...context.canvas, id: 'newer-editor' } };
    client.receive({ ...sent[1], type: 'context.reply', context: newer });
    await expect(second).resolves.toEqual(newer);
    client.receive({ ...sent[0], type: 'context.reply', context });
    await expect(first).resolves.toEqual(context);
    expect(client.readCanvas()?.id).toBe('newer-editor');
  } finally {
    client.dispose();
  }
});

test('does not restore cached native context after disposal between reply and continuation', async () => {
  const sent: CanvasMessage[] = [];
  const client = new CanvasClient(session, {
    send: (message) => sent.push(message),
    later: () => () => {},
    report: () => {},
  });
  const pending = client.readContext();
  client.receive({ ...sent[0], type: 'context.reply', context });
  client.dispose();
  await expect(pending).rejects.toThrow('closed');
  expect(client.readCanvas()).toBeNull();
});
