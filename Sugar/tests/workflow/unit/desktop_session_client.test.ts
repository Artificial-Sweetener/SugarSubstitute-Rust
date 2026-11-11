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
/** Exercise actual protocol capture/materialization through an in-memory native transport boundary. */
import { createHash } from 'node:crypto';
import { DesktopSessionClient } from '../../../frontend/comfyui/ui/workflow/application/DesktopSessionClient.js';
import { desktopSessionChunks } from '../../../frontend/comfyui/ui/workflow/application/DesktopSessionTransfer.js';
import type {
  DesktopSessionRuntime,
  DesktopSessionTransport,
} from '../../../frontend/comfyui/ui/workflow/domain/DesktopSession.js';
import { DesktopSessionRefusal } from '../../../frontend/comfyui/ui/workflow/domain/DesktopSession.js';
import type {
  SessionMessage,
  SessionOperation,
} from '../../../integrations/contracts/SessionProtocol.generated.js';
import { utf8Bytes } from '../../../integrations/shared/SessionPrimitives.js';
import { desktopSessionFixture } from '../support/desktop_session.js';

/** Supply deterministic hashing at the external cryptography port. */
async function digest(text: string): Promise<string> {
  return createHash('sha256').update(text, 'utf8').digest('hex');
}

/** Compose real protocol behavior with a fake native boundary and semantic-owner effects. */
function fixture() {
  const shared = desktopSessionFixture();
  const sent: SessionMessage[] = [];
  const acknowledged: number[] = [];
  const captures: number[] = [];
  const materialized: string[] = [];
  const errors: unknown[] = [];
  const listeners = new Set<() => void>();
  let sequence = 100;
  let fence = shared.fence;
  let committed = true;
  let persistence = true;
  const transport: DesktopSessionTransport = {
    scope: shared.checkpoint.scope,
    origin: shared.origin,
    webview: fence.webview,
    send: async (message) => {
      sent.push(message);
    },
    subscribe: () => () => {},
  };
  const runtime: DesktopSessionRuntime = {
    fence: () => fence,
    readiness: () => ({ committed, workflowPersistence: persistence }),
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    capture: async (id, revision) => {
      captures.push(revision);
      return { ...shared.checkpoint, id, revision };
    },
    materialize: async (checkpoint) => {
      materialized.push(checkpoint.id);
      return fence.bindings;
    },
    acknowledged: (_id, revision) => {
      acknowledged.push(revision);
    },
  };
  const client = new DesktopSessionClient({
    transport,
    runtime,
    nextId: () => `00000000-0000-4000-8000-${(++sequence).toString(16).padStart(12, '0')}`,
    digest,
    report: (error) => errors.push(error),
  });
  const requestId = '00000000-0000-4000-8000-0000000000ff';
  const message = (operation: SessionOperation): SessionMessage => ({
    protocol: 'sugarcubes.desktop-session',
    version: 1,
    requestId,
    scope: transport.scope,
    origin: transport.origin,
    operation,
  });
  return {
    ...shared,
    client,
    sent,
    acknowledged,
    captures,
    materialized,
    errors,
    runtime,
    transport,
    message,
    setFence: (value: typeof fence) => {
      fence = value;
    },
    setCommitted: (value: boolean) => {
      committed = value;
    },
    setPersistence: (value: boolean) => {
      persistence = value;
    },
  };
}

test('capture uses host durable revision and cannot acknowledge durability before the host', async () => {
  const host = fixture();
  host.client.install();
  await host.client.receive(
    host.message({
      kind: 'capture',
      fence: host.fence,
      checkpointId: host.checkpoint.id,
      revision: 77,
    }),
  );
  expect(host.captures).toEqual([77]);
  const begin = host.sent.find((message) => message.operation.kind === 'begin');
  expect(begin?.operation.kind === 'begin' ? begin.operation.transfer.revision : null).toBe(77);
  expect(host.sent.at(-1)?.operation).toMatchObject({
    kind: 'acknowledged',
    phase: 'captured',
    revision: 77,
  });
  expect(host.acknowledged).toEqual([]);
  await host.client.receive(
    host.message({
      kind: 'acknowledged',
      checkpointId: host.checkpoint.id,
      revision: 77,
      phase: 'durable',
      bindings: host.fence.bindings,
    }),
  );
  expect(host.acknowledged).toEqual([77]);
  expect(host.sent.at(-1)?.operation).toEqual({
    kind: 'acknowledged',
    checkpointId: host.checkpoint.id,
    revision: 77,
    phase: 'durable',
    bindings: host.fence.bindings,
  });
  const count = host.sent.length;
  await host.client.receive(
    host.message({
      kind: 'acknowledged',
      checkpointId: host.checkpoint.id,
      revision: 77,
      phase: 'durable',
      bindings: host.fence.bindings,
    }),
  );
  expect(host.sent).toHaveLength(count);
  expect(host.acknowledged).toEqual([77]);
  host.client.dispose();
});

test('unsolicited host acknowledgements cannot advance durable state or claim close delivery', async () => {
  const host = fixture();
  await host.client.receive(
    host.message({
      kind: 'acknowledged',
      checkpointId: host.checkpoint.id,
      revision: 777,
      phase: 'durable',
      bindings: host.fence.bindings,
    }),
  );
  expect(host.acknowledged).toEqual([]);
  expect(host.sent).toEqual([]);
});

test('startup loading persistence opt-out and stale fences refuse effects', async () => {
  const host = fixture();
  const capture = host.message({
    kind: 'capture',
    fence: host.fence,
    checkpointId: host.checkpoint.id,
    revision: 10,
  });
  host.setCommitted(false);
  await host.client.receive(capture);
  expect(host.sent.at(-1)?.operation).toMatchObject({
    kind: 'error',
    failure: { code: 'not-committed' },
  });
  host.setCommitted(true);
  host.setPersistence(false);
  await host.client.receive(capture);
  expect(host.sent.at(-1)?.operation).toMatchObject({
    kind: 'error',
    failure: { code: 'opted-out' },
  });
  host.setPersistence(true);
  host.setFence({ ...host.fence, revision: host.fence.revision + 1 });
  await host.client.receive(capture);
  expect(host.sent.at(-1)?.operation).toMatchObject({ kind: 'error', failure: { code: 'stale' } });
  expect(host.captures).toEqual([]);
});

test('a graph load that begins during admitted capture returns a retryable refusal without reporting a canvas failure', async () => {
  const host = fixture();
  let release: () => void = () => {};
  let entered: () => void = () => {};
  const boundary = new Promise<void>((resolve) => {
    release = resolve;
  });
  const captureEntered = new Promise<void>((resolve) => {
    entered = resolve;
  });
  const capture = host.runtime.capture;
  host.runtime.capture = async (...args) => {
    entered();
    await boundary;
    if (!host.runtime.readiness().committed)
      throw new DesktopSessionRefusal(
        'not-committed',
        'Comfy workflow capture requires a committed graph.',
      );
    return capture(...args);
  };
  const request = host.message({
    kind: 'capture',
    fence: host.fence,
    checkpointId: host.checkpoint.id,
    revision: 10,
  });
  const pending = host.client.receive(request);
  await captureEntered;
  host.setCommitted(false);
  host.setFence({ ...host.runtime.fence(), revision: host.runtime.fence().revision + 1 });
  release();
  await pending;
  expect(
    host.sent.find(
      (message) => message.requestId === request.requestId && message.operation.kind === 'error',
    ),
  ).toEqual({
    ...request,
    operation: {
      kind: 'error',
      failure: { code: 'not-committed', retryable: true, entryId: null, outputId: null },
    },
  });
  expect(
    host.sent.some(
      (message) =>
        message.operation.kind === 'begin' ||
        (message.operation.kind === 'acknowledged' && message.operation.phase === 'captured'),
    ),
  ).toBe(false);
  expect(host.errors).toEqual([]);
  expect(host.captures).toEqual([]);
  host.setCommitted(true);
  host.setFence({ ...host.runtime.fence(), revision: host.runtime.fence().revision + 1 });
  host.runtime.capture = capture;
  const subsequent = '00000000-0000-4000-8000-0000000000ab';
  await host.client.receive({
    ...host.message({
      kind: 'capture',
      fence: host.runtime.fence(),
      checkpointId: host.checkpoint.id,
      revision: 11,
    }),
    requestId: subsequent,
  });
  expect(host.captures).toEqual([11]);
  expect(
    host.sent.find(
      (message) => message.requestId === subsequent && message.operation.kind === 'begin',
    ),
  ).toBeDefined();
  expect(
    host.sent.find(
      (message) => message.requestId === subsequent && message.operation.kind === 'acknowledged',
    )?.operation,
  ).toMatchObject({ kind: 'acknowledged', phase: 'captured', revision: 11 });
  expect(host.acknowledged).toEqual([]);
  expect(host.errors).toEqual([]);
  host.client.dispose();
});

test('a capture the workspace outran is refused as retryable without reporting a canvas failure', async () => {
  const host = fixture();
  host.runtime.capture = async () => {
    throw new DesktopSessionRefusal(
      'stale',
      'Desktop workflow changed while its checkpoint was captured.',
    );
  };
  const request = host.message({
    kind: 'capture',
    fence: host.fence,
    checkpointId: host.checkpoint.id,
    revision: 12,
  });
  await host.client.receive(request);
  expect(host.errors).toEqual([]);
  expect(
    host.sent.find(
      (message) => message.requestId === request.requestId && message.operation.kind === 'error',
    )?.operation,
  ).toMatchObject({ kind: 'error', failure: { code: 'stale', retryable: true } });
  host.client.dispose();
});

test('an unexpected capture error is reported with its original cause and correlated invalid reply', async () => {
  const host = fixture();
  const error = new Error('Unexpected document serialization failure.');
  host.runtime.capture = async () => {
    throw error;
  };
  const request = host.message({
    kind: 'capture',
    fence: host.fence,
    checkpointId: host.checkpoint.id,
    revision: 77,
  });
  await host.client.receive(request);
  expect(host.errors).toEqual([error]);
  expect(
    host.sent.find(
      (message) => message.requestId === request.requestId && message.operation.kind === 'error',
    )?.operation,
  ).toMatchObject({ kind: 'error', failure: { code: 'invalid', retryable: false } });
  expect(host.sent.some((message) => message.operation.kind === 'begin')).toBe(false);
  host.client.dispose();
});

test('materializes only after exact chunks digest and full checkpoint admission', async () => {
  const host = fixture();
  const transferId = '00000000-0000-4000-8000-0000000000fe';
  const text = JSON.stringify(host.checkpoint);
  const chunks = desktopSessionChunks(text);
  await host.client.receive(
    host.message({
      kind: 'begin',
      transfer: {
        id: transferId,
        checkpointId: host.checkpoint.id,
        revision: host.checkpoint.revision,
        bytes: utf8Bytes(text),
        chunks: chunks.length,
        digest: await digest(text),
        direction: 'materialize',
      },
    }),
  );
  for (const [index, text] of chunks.entries())
    await host.client.receive(host.message({ kind: 'chunk', transferId, index, text }));
  await host.client.receive(host.message({ kind: 'commit', transferId }));
  expect(host.materialized).toEqual([]);
  await host.client.receive(host.message({ kind: 'materialize', transferId, fence: host.fence }));
  expect(host.materialized).toEqual([host.checkpoint.id]);
  expect(
    host.sent.find((message) => message.operation.kind === 'acknowledged')?.operation,
  ).toMatchObject({ kind: 'acknowledged', phase: 'materialized' });
  expect(host.acknowledged).toEqual([]);
  expect(host.errors).toEqual([]);
});

/** Deliver one saved workspace and ask the page to materialize it. */
async function requestMaterialization(host: ReturnType<typeof fixture>): Promise<SessionMessage> {
  const transferId = '00000000-0000-4000-8000-0000000000fe';
  const text = JSON.stringify(host.checkpoint);
  const chunks = desktopSessionChunks(text);
  await host.client.receive(
    host.message({
      kind: 'begin',
      transfer: {
        id: transferId,
        checkpointId: host.checkpoint.id,
        revision: host.checkpoint.revision,
        bytes: utf8Bytes(text),
        chunks: chunks.length,
        digest: await digest(text),
        direction: 'materialize',
      },
    }),
  );
  for (const [index, text] of chunks.entries())
    await host.client.receive(host.message({ kind: 'chunk', transferId, index, text }));
  await host.client.receive(host.message({ kind: 'commit', transferId }));
  return host.message({ kind: 'materialize', transferId, fence: host.fence });
}

test('a graph load that supersedes a restore ends the host request as cancelled without reporting a canvas failure', async () => {
  const host = fixture();
  let entered: () => void = () => {};
  let supersede: () => void = () => {};
  const restoring = new Promise<void>((resolve) => {
    entered = resolve;
  });
  host.runtime.materialize = async (_checkpoint, cancellation) => {
    supersede = () => cancellation.cancel();
    entered();
    // The page's own graph load cancels the restore; its pending asset read then aborts.
    await new Promise<void>((resolve) => cancellation.subscribe(resolve));
    throw new DOMException('signal is aborted without reason', 'AbortError');
  };
  const request = await requestMaterialization(host);
  const pending = host.client.receive(request);
  await restoring;
  supersede();
  await pending;
  expect(host.errors).toEqual([]);
  expect(
    host.sent.filter(
      (message) => message.requestId === request.requestId && message.operation.kind === 'error',
    ),
  ).toEqual([
    {
      ...request,
      operation: {
        kind: 'error',
        failure: { code: 'cancelled', retryable: false, entryId: null, outputId: null },
      },
    },
  ]);
  expect(host.sent.some((message) => message.operation.kind === 'acknowledged')).toBe(false);
  host.client.dispose();
});

test('a restore the host cancels is answered once, by the cancellation itself', async () => {
  const host = fixture();
  let entered: () => void = () => {};
  const restoring = new Promise<void>((resolve) => {
    entered = resolve;
  });
  host.runtime.materialize = async (_checkpoint, cancellation) => {
    entered();
    await new Promise<void>((resolve) => cancellation.subscribe(resolve));
    throw new DOMException('signal is aborted without reason', 'AbortError');
  };
  const request = await requestMaterialization(host);
  const pending = host.client.receive(request);
  await restoring;
  await host.client.receive({
    ...host.message({ kind: 'cancel', targetRequestId: request.requestId }),
    requestId: '00000000-0000-4000-8000-0000000000ac',
  });
  await pending;
  expect(host.errors).toEqual([]);
  expect(
    host.sent.filter(
      (message) => message.requestId === request.requestId && message.operation.kind === 'error',
    ),
  ).toHaveLength(1);
  host.client.dispose();
});

test('stale capture retains operation ownership until native error admission completes', async () => {
  const host = fixture();
  let release: () => void = () => {};
  let entered: () => void = () => {};
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  const staleEntered = new Promise<void>((resolve) => {
    entered = resolve;
  });
  const capture = host.runtime.capture;
  host.runtime.capture = async (...args) => {
    const checkpoint = await capture(...args);
    host.setFence({ ...host.fence, revision: host.fence.revision + 1 });
    return checkpoint;
  };
  host.transport.send = async (message) => {
    host.sent.push(message);
    if (message.operation.kind === 'error' && message.operation.failure.code === 'stale') {
      entered();
      await gate;
    }
  };
  const first = host.client.receive(
    host.message({
      kind: 'capture',
      fence: host.fence,
      checkpointId: host.checkpoint.id,
      revision: 10,
    }),
  );
  await staleEntered;
  try {
    const next = '00000000-0000-4000-8000-0000000000ab';
    await host.client.receive({
      ...host.message({
        kind: 'capture',
        fence: host.runtime.fence(),
        checkpointId: host.checkpoint.id,
        revision: 11,
      }),
      requestId: next,
    });
    expect(host.captures).toEqual([10]);
    expect(host.sent.find((message) => message.requestId === next)?.operation).toMatchObject({
      kind: 'error',
      failure: { code: 'unavailable' },
    });
  } finally {
    release();
    await first;
    host.client.dispose();
  }
});

test('native send admission applies backpressure and cancellation stops remaining capture chunks', async () => {
  const host = fixture();
  let release: () => void = () => {};
  let entered: () => void = () => {};
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  const beginEntered = new Promise<void>((resolve) => {
    entered = resolve;
  });
  host.transport.send = async (message) => {
    host.sent.push(message);
    if (message.operation.kind === 'begin') {
      entered();
      await gate;
    }
  };
  const capture = host.message({
    kind: 'capture',
    fence: host.fence,
    checkpointId: host.checkpoint.id,
    revision: 77,
  });
  const pending = host.client.receive(capture);
  await beginEntered;
  expect(host.sent.some((message) => message.operation.kind === 'chunk')).toBe(false);
  await host.client.receive(host.message({ kind: 'cancel', targetRequestId: capture.requestId }));
  release();
  await pending;
  expect(
    host.sent.some(
      (message) => message.operation.kind === 'chunk' || message.operation.kind === 'commit',
    ),
  ).toBe(false);
  expect(
    host.sent.some(
      (message) =>
        message.operation.kind === 'error' && message.operation.failure.code === 'cancelled',
    ),
  ).toBe(true);
});

test('artifact or storage refusal preserves the owner code and original cause', async () => {
  const host = fixture();
  const cause = new Error('Recorded artifact is missing.');
  const refusal = new DesktopSessionRefusal('unavailable', 'Exact final artifact is unavailable.', {
    cause,
  });
  host.runtime.capture = async () => {
    throw refusal;
  };
  await host.client.receive(
    host.message({
      kind: 'capture',
      fence: host.fence,
      checkpointId: host.checkpoint.id,
      revision: 77,
    }),
  );
  expect(
    host.sent.some(
      (message) =>
        message.operation.kind === 'error' && message.operation.failure.code === 'unavailable',
    ),
  ).toBe(true);
  expect(host.errors).toEqual([refusal]);
  expect(refusal.cause).toBe(cause);
});
