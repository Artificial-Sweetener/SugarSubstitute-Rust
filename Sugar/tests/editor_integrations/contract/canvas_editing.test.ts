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
/** Prove editing commands, results and pushed state across the shared canvas session. */
import { jest } from '@jest/globals';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { CanvasServer } from '../../../integrations/shared/CanvasServer.js';
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';
import { isCanvasMessage } from '../../../integrations/shared/CanvasValidation.generated.js';
import type {
  CanvasCapabilities,
  CanvasEditState,
  CanvasMessage,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
import {
  canvasWireMessage,
  invalidCanvasWireMessages,
  readCanvasWireVectors,
} from '../support/canvas_wire_vectors.js';

const vectors = readCanvasWireVectors('canvas_editing_vectors.json');
const session = 'session-12345678';
const editing: CanvasCapabilities = {
  displayOutput: false,
  insertImageLayer: false,
  positionImageLayer: false,
  geometry: true,
  editing: true,
  workflowSurfaces: true,
};

/** Read the pushed state carried by the valid notification vector. */
function state(revision: number): CanvasEditState {
  const message = canvasWireMessage(vectors, 'changed');
  if (!isCanvasMessage(message) || message.type !== 'edit.changed')
    throw new Error('Invalid notification vector.');
  return { ...message.state, revision };
}

/** Wrap a state in the host's notification envelope. */
function changed(revision: number): CanvasMessage {
  return {
    protocol: 'sugarcubes.canvas',
    version: 3,
    session,
    requestId: 0,
    type: 'edit.changed',
    state: state(revision),
  };
}

/** Connect a client whose transport records requests and answers context reads. */
async function connected(capabilities: CanvasCapabilities = editing) {
  const sent: CanvasMessage[] = [];
  const errors: unknown[] = [];
  const client = new CanvasClient(session, {
    send: (message) => {
      sent.push(message);
      if (message.type === 'context.request')
        queueMicrotask(() =>
          client.receive({
            protocol: 'sugarcubes.canvas',
            version: 3,
            session,
            requestId: message.requestId,
            type: 'context.reply',
            context: { canvas: { id: 'native', label: 'Native', capabilities }, document: null },
          }),
        );
    },
    later: (milliseconds, callback) => {
      const timer = setTimeout(callback, milliseconds);
      return () => clearTimeout(timer);
    },
    report: (error) => errors.push(error),
  });
  await client.readContext();
  return { client, sent, errors };
}

test('admits every valid editing message and refuses every invalid mutation', () => {
  for (const [name, message] of Object.entries(vectors.valid))
    expect([name, isCanvasMessage(message)]).toEqual([name, true]);
  for (const { name, message } of invalidCanvasWireMessages(vectors))
    expect([name, isCanvasMessage(message)]).toEqual([name, false]);
});

test('correlates a command with its typed state result through the session transport', async () => {
  const { client, sent } = await connected();
  expect(client.editing.canvasId).toBe('native');
  const pending = client.editing.execute({ type: 'nudge', dx: -1, dy: 10 });
  const request = sent.at(-1);
  if (request?.type !== 'edit.request') throw new Error('No edit request was sent.');
  expect(request.command).toEqual({ type: 'nudge', dx: -1, dy: 10 });
  const { protocol, version, requestId } = request;
  const envelope = { protocol, version, session, requestId };
  // A reply of another kind for the same identity is not this request's answer.
  expect(client.receive({ ...envelope, type: 'workflow.reply', result: { kind: 'applied' } })).toBe(
    false,
  );
  expect(
    client.receive({ ...envelope, type: 'edit.reply', result: { kind: 'state', state: state(3) } }),
  ).toBe(true);
  expect(await pending).toEqual({ kind: 'state', state: state(3) });
  const refused = client.editing.execute({ type: 'state' });
  const next = sent.at(-1);
  if (next?.type !== 'edit.request') throw new Error('No edit request was sent.');
  client.receive({
    protocol,
    version,
    session,
    requestId: next.requestId,
    type: 'error.reply',
    failure: { status: 'unavailable', message: 'No document.' },
  });
  await expect(refused).rejects.toThrow('No document.');
  client.dispose();
});

test('publishes pushed states in revision order only while the canvas advertises editing', async () => {
  const { client } = await connected();
  const received: CanvasEditState[] = [];
  const subscription = client.editing.changes.subscribe((value) => received.push(value));
  await expect(subscription.ready).resolves.toBe('observing');
  const duplicate = client.editing.changes.subscribe((value) => received.push(value));
  expect(client.receive(changed(5))).toBe(true);
  expect(client.receive(changed(5))).toBe(false);
  expect(client.receive(changed(4))).toBe(false);
  expect(received.map(({ revision }) => revision)).toEqual([5, 5]);
  expect(Object.isFrozen(received[0])).toBe(true);
  expect(Object.isFrozen(received[0]?.selection?.bounds)).toBe(true);
  expect(Object.isFrozen(received[0]?.tools[0])).toBe(true);
  duplicate.stop();
  expect(client.receive(changed(6))).toBe(true);
  expect(received.map(({ revision }) => revision)).toEqual([5, 5, 6]);
  subscription.stop();
  client.dispose();
  expect(client.receive(changed(7))).toBe(false);
  expect(received).toHaveLength(3);
});

test('refuses pushed states from a canvas that does not advertise editing', async () => {
  const { client } = await connected({ ...editing, editing: false });
  const listener = jest.fn();
  client.editing.changes.subscribe(listener);
  expect(client.receive(changed(1))).toBe(false);
  expect(listener).not.toHaveBeenCalled();
  client.dispose();
});

test('answers caller cancellation locally and tells the host to stop', async () => {
  const { client, sent } = await connected();
  const cancellation = new CanvasCancellation();
  const pending = client.editing.execute(
    { type: 'modify', target: 'selection', operation: 'expand', pixels: 4, phase: 'apply' },
    cancellation,
  );
  const request = sent.at(-1);
  cancellation.cancel();
  expect(await pending).toEqual({
    kind: 'failure',
    status: 'cancelled',
    message: 'Canvas edit cancelled.',
  });
  expect(sent.at(-1)).toMatchObject({ type: 'cancel', requestId: request?.requestId });
  const count = sent.length;
  expect(await client.editing.execute({ type: 'commit' }, cancellation)).toMatchObject({
    status: 'cancelled',
  });
  expect(sent).toHaveLength(count);
  client.dispose();
});

test('refuses an out-of-contract command before it reaches the transport', async () => {
  const { client, sent } = await connected();
  const count = sent.length;
  await expect(client.editing.execute({ type: 'nudge', dx: 500, dy: 0 })).rejects.toThrow(
    'Canvas edit request is invalid.',
  );
  expect(sent).toHaveLength(count);
  client.dispose();
});

test('bounds every edit by the session deadline and cancels it without replay', async () => {
  jest.useFakeTimers();
  try {
    const sent: CanvasMessage[] = [];
    const client = new CanvasClient(session, {
      send: (message) => sent.push(message),
      later: (milliseconds, callback) => {
        const timer = setTimeout(callback, milliseconds);
        return () => clearTimeout(timer);
      },
      report: () => {},
    });
    const timed = expect(
      client.editing.execute({ type: 'history', action: 'undo' }),
    ).rejects.toThrow('timed out');
    jest.advanceTimersByTime(30000);
    await timed;
    expect(sent.map(({ type }) => type)).toEqual(['edit.request', 'cancel']);
    client.dispose();
    await expect(client.editing.execute({ type: 'state' })).resolves.toMatchObject({
      status: 'unavailable',
    });
    expect(jest.getTimerCount()).toBe(0);
  } finally {
    jest.useRealTimers();
  }
});

test('editor ports answer editing and workflow requests with explicit unsupported results', () => {
  const replies: CanvasMessage[] = [];
  const server = new CanvasServer(session, {
    context: () => Promise.reject(new Error('unused')),
    insert: () => Promise.reject(new Error('unused')),
    send: (message) => replies.push(message),
    report: () => {},
  });
  expect(server.receive(canvasWireMessage(vectors, 'tool'))).toBe(true);
  expect(
    server.receive({
      protocol: 'sugarcubes.canvas',
      version: 3,
      session,
      requestId: 8,
      type: 'workflow.request',
      command: { type: 'hover', workflow: 'w', surface: 's', mask: null },
    }),
  ).toBe(true);
  expect(replies).toEqual([
    expect.objectContaining({
      type: 'edit.reply',
      requestId: 7,
      result: expect.objectContaining({ kind: 'failure', status: 'unsupported' }),
    }),
    expect.objectContaining({
      type: 'workflow.reply',
      requestId: 8,
      result: expect.objectContaining({ kind: 'failure', status: 'unsupported' }),
    }),
  ]);
  expect(replies.every(isCanvasMessage)).toBe(true);
  server.dispose();
});
