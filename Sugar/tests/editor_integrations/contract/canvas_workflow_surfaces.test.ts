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
/** Prove workflow surface provisioning commands and results across the shared canvas session. */
import { jest } from '@jest/globals';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';
import { isCanvasMessage } from '../../../integrations/shared/CanvasValidation.generated.js';
import type {
  CanvasMessage,
  CanvasWorkflowCommand,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
import {
  canvasWireMessage,
  invalidCanvasWireMessages,
  readCanvasWireVectors,
} from '../support/canvas_wire_vectors.js';

const vectors = readCanvasWireVectors('canvas_workflow_vectors.json');
const session = 'session-12345678';

/** Read the command carried by one valid request vector. */
function command(name: string): CanvasWorkflowCommand {
  const message = canvasWireMessage(vectors, name);
  if (!isCanvasMessage(message) || message.type !== 'workflow.request')
    throw new Error('Invalid workflow request vector.');
  return message.command;
}

/** Build a client whose transport records every message it sends. */
function client() {
  const sent: CanvasMessage[] = [];
  const canvas = new CanvasClient(session, {
    send: (message) => sent.push(message),
    later: (milliseconds, callback) => {
      const timer = setTimeout(callback, milliseconds);
      return () => clearTimeout(timer);
    },
    report: () => {},
  });
  return { canvas, sent };
}

test('admits every valid workflow message and refuses every invalid mutation', () => {
  for (const [name, message] of Object.entries(vectors.valid))
    expect([name, isCanvasMessage(message)]).toEqual([name, true]);
  for (const { name, message } of invalidCanvasWireMessages(vectors))
    expect([name, isCanvasMessage(message)]).toEqual([name, false]);
});

test.each([
  ['sync', 'synced'],
  ['activateInput', 'activated'],
  ['appearance', 'applied'],
  ['resizeScale', 'synced'],
  ['hover', 'failure'],
])('correlates %s with the host result it answers', async (request, reply) => {
  const { canvas, sent } = client();
  const pending = canvas.workflow.execute(command(request));
  const message = sent.at(-1);
  if (message?.type !== 'workflow.request') throw new Error('No workflow request was sent.');
  expect(message.command).toEqual(command(request));
  const answer = canvasWireMessage(vectors, reply);
  if (!isCanvasMessage(answer) || answer.type !== 'workflow.reply')
    throw new Error('Invalid workflow reply vector.');
  expect(canvas.receive({ ...answer, requestId: message.requestId + 1 })).toBe(false);
  expect(canvas.receive({ ...answer, requestId: message.requestId })).toBe(true);
  expect(await pending).toEqual(answer.result);
  canvas.dispose();
});

test('answers caller cancellation locally without replaying the provisioning command', async () => {
  const { canvas, sent } = client();
  const cancellation = new CanvasCancellation();
  const pending = canvas.workflow.execute(command('sync'), cancellation);
  const request = sent.at(-1);
  cancellation.cancel();
  expect(await pending).toEqual({
    kind: 'failure',
    status: 'cancelled',
    message: 'Canvas workflow request cancelled.',
  });
  expect(sent.map(({ type }) => type)).toEqual(['workflow.request', 'cancel']);
  expect(sent.at(-1)).toMatchObject({ requestId: request?.requestId });
  canvas.dispose();
});

test('refuses an out-of-contract sync before it reaches the transport', async () => {
  const { canvas, sent } = client();
  const sync = command('sync');
  const template = sync.type === 'sync' ? sync.surfaces[1] : undefined;
  if (!template) throw new Error('Invalid sync vector.');
  const oversized: CanvasWorkflowCommand = {
    type: 'sync',
    workflow: 'workflow:a',
    surfaces: Array.from({ length: 65 }, (_, index) => ({
      ...template,
      key: `authority:${String(index)}`,
    })),
  };
  await expect(canvas.workflow.execute(oversized)).rejects.toThrow(
    'Canvas workflow request is invalid.',
  );
  expect(sent).toEqual([]);
  canvas.dispose();
});

test('rejects pending provisioning when the session closes and cancels the host work', async () => {
  jest.useFakeTimers();
  try {
    const { canvas, sent } = client();
    const pending = expect(canvas.workflow.execute(command('emptySync'))).rejects.toThrow(
      'Canvas connection closed.',
    );
    canvas.dispose();
    await pending;
    expect(sent.map(({ type }) => type)).toEqual(['workflow.request', 'cancel']);
    expect(jest.getTimerCount()).toBe(0);
  } finally {
    jest.useRealTimers();
  }
});
