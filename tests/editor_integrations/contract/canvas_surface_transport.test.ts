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
/** Exercise one shared canvas session against capable and limited surface hosts. */

import { jest } from '@jest/globals';
import { CanvasClient } from '../../../integrations/shared/CanvasClient.js';
import { CanvasServer } from '../../../integrations/shared/CanvasServer.js';
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';
import { isCanvasMessage } from '../../../integrations/shared/CanvasValidation.generated.js';
import type { CanvasMessage } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import type {
  CanvasSurfaceCommand,
  CanvasSurfaceResult,
} from '../../../integrations/contracts/CanvasSurface.js';

const session = 'canvas-surface-session';
const address = {
  sessionId: 'canvas:workflow',
  viewId: 'view:one',
  windowId: 'window:one',
  attachmentId: 'attachment:one',
  generation: 1,
} as const;
const geometry = {
  coordinateSpace: 'client',
  units: 'css-pixels',
  origin: { x: 10, y: 20 },
  viewport: { width: 640, height: 480 },
  clip: { x: 10, y: 20, width: 640, height: 480 },
  physicalPixelsPerCssPixel: 1.5,
  visible: true,
  focused: false,
} as const;

/** Exercise real client/server correlation while faking only the native surface. */
function fixture(capable: boolean) {
  const sent: CanvasMessage[] = [];
  const errors: unknown[] = [];
  const execute = jest.fn(
    async (command: CanvasSurfaceCommand): Promise<CanvasSurfaceResult> => ({
      status: command.type === 'surface.prepare' ? 'ready' : 'applied',
      message: '',
    }),
  );
  const server = new CanvasServer(session, {
    ...(capable ? { surface: { execute } } : {}),
    context: async () => ({
      canvas: {
        id: 'native',
        label: 'Native',
        capabilities: {
          displayOutput: capable,
          insertImageLayer: false,
          positionImageLayer: false,
        },
      },
      document: null,
    }),
    insert: async () => ({ status: 'unsupported', message: 'No layers.' }),
    send: (message) => client.receive(message),
    report: (error) => errors.push(error),
  });
  const client = new CanvasClient(session, {
    send: (message) => {
      sent.push(message);
      server.receive(message);
    },
    later: () => () => {},
    report: (error) => errors.push(error),
  });
  return { client, server, execute, sent, errors };
}

test.each([true, false])(
  'negotiates a surface against a capable or limited host: %s',
  async (capable) => {
    const f = fixture(capable);
    try {
      const prepared = await f.client.surface.execute({
        type: 'surface.prepare',
        address,
        revision: 1,
      });
      expect(prepared.status).toBe(capable ? 'ready' : 'unavailable');
      expect(f.execute).toHaveBeenCalledTimes(capable ? 1 : 0);
      if (!capable) return;
      const commands: CanvasSurfaceCommand[] = [
        {
          type: 'surface.select',
          address,
          revision: 2,
          itemId: 'result:one',
          file: { filename: 'résumé.png', subfolder: 'batch', type: 'output' },
        },
        { type: 'surface.commit', address, revision: 3 },
        { type: 'surface.geometry', address, revision: 4, geometry },
        { type: 'surface.release', address, revision: 5 },
      ];
      for (const command of commands)
        expect((await f.client.surface.execute(command)).status).toBe('applied');
      expect(f.execute.mock.calls.map(([command]) => command)).toEqual([
        { type: 'surface.prepare', address, revision: 1 },
        ...commands,
      ]);
      expect(f.sent.map((message) => message.requestId)).toEqual([1, 2, 3, 4, 5]);
      f.server.receive(f.sent[1]);
      expect(f.execute).toHaveBeenCalledTimes(5);
      expect(f.errors).toEqual([]);
    } finally {
      f.client.dispose();
      f.server.dispose();
    }
  },
);

test.each([
  { ...address, viewId: 'view:other' },
  { ...address, generation: 2 },
])('rejects a reply for another attachment %#', async (foreign) => {
  const sent: CanvasMessage[] = [];
  const client = new CanvasClient(session, {
    send: (message) => sent.push(message),
    later: () => () => {},
    report: () => {},
  });
  const pending = client.surface.execute({ type: 'surface.prepare', address, revision: 1 });
  expect(
    client.receive({
      protocol: 'sugarcubes.canvas',
      version: 3,
      session,
      requestId: 1,
      type: 'surface.reply',
      address: foreign,
      revision: 1,
      status: 'ready',
      message: '',
    }),
  ).toBe(true);
  await expect(pending).rejects.toThrow('does not match');
  client.dispose();
});

test('surface cancellation waits for the correlated host acknowledgement', async () => {
  const sent: CanvasMessage[] = [];
  const cancellation = new CanvasCancellation();
  const client = new CanvasClient(session, {
    send: (message) => sent.push(message),
    later: () => () => {},
    report: () => {},
  });
  const pending = client.surface.execute(
    { type: 'surface.prepare', address, revision: 1 },
    cancellation,
  );
  let settled = false;
  void pending.then(() => {
    settled = true;
  });
  cancellation.cancel();
  await Promise.resolve();
  expect(settled).toBe(false);
  expect(sent.map(({ type, requestId }) => ({ type, requestId }))).toEqual([
    { type: 'surface.prepare', requestId: 1 },
    { type: 'cancel', requestId: 1 },
  ]);
  expect(
    client.receive({
      protocol: 'sugarcubes.canvas',
      version: 3,
      session,
      requestId: 1,
      type: 'surface.reply',
      address,
      revision: 1,
      status: 'cancelled',
      message: '',
    }),
  ).toBe(true);
  await expect(pending).resolves.toMatchObject({ status: 'cancelled' });
  expect(
    client.receive({
      protocol: 'sugarcubes.canvas',
      version: 3,
      session,
      requestId: 1,
      type: 'surface.reply',
      address,
      revision: 1,
      status: 'ready',
      message: '',
    }),
  ).toBe(false);
  client.dispose();
});

test('an already cancelled surface request remains local', async () => {
  const sent: CanvasMessage[] = [];
  const cancellation = new CanvasCancellation();
  cancellation.cancel();
  const client = new CanvasClient(session, {
    send: (message) => sent.push(message),
    later: () => () => {},
    report: () => {},
  });
  await expect(
    client.surface.execute({ type: 'surface.prepare', address, revision: 1 }, cancellation),
  ).resolves.toMatchObject({ status: 'cancelled' });
  expect(sent).toEqual([]);
  client.dispose();
});

test('rejects invalid selected-result identities and malformed geometry before dispatch', async () => {
  const f = fixture(true);
  try {
    await expect(
      f.client.surface.execute({
        type: 'surface.select',
        address,
        revision: 1,
        itemId: 'result:',
        file: { filename: 'a.png', subfolder: '', type: 'output' },
      }),
    ).rejects.toThrow('invalid');
    expect(f.sent).toEqual([]);
    expect(f.execute).not.toHaveBeenCalled();
    expect(
      isCanvasMessage({
        protocol: 'sugarcubes.canvas',
        version: 3,
        session,
        requestId: 2,
        type: 'surface.geometry',
        address,
        revision: 2,
        geometry: { ...geometry, physicalPixelsPerCssPixel: 0 },
      }),
    ).toBe(false);
  } finally {
    f.client.dispose();
    f.server.dispose();
  }
});

test('keeps the admitted surface request unchanged while the host context is pending', async () => {
  let release = (): void => {
    throw new Error('Context gate was not initialized.');
  };
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  let completed = (): void => {
    throw new Error('Completion gate was not initialized.');
  };
  const done = new Promise<void>((resolve) => {
    completed = resolve;
  });
  const commands: CanvasSurfaceCommand[] = [];
  const server = new CanvasServer(session, {
    surface: {
      execute: async (command) => {
        commands.push(command);
        if (commands.length === 2) completed();
        return { status: 'applied', message: '' };
      },
    },
    context: async () => {
      await gate;
      return {
        canvas: {
          id: 'native',
          label: 'Native',
          capabilities: { displayOutput: true, insertImageLayer: false, positionImageLayer: false },
        },
        document: null,
      };
    },
    insert: async () => ({ status: 'unsupported', message: 'No layers.' }),
    send: () => {},
    report: (error) => {
      throw error;
    },
  });
  const selected = {
    protocol: 'sugarcubes.canvas',
    version: 3,
    session,
    requestId: 1,
    type: 'surface.select',
    address: { ...address, viewId: String(address.viewId) },
    revision: 1,
    itemId: 'result:one',
    file: { filename: 'first.png', subfolder: '', type: 'output' },
  };
  const positioned = {
    protocol: 'sugarcubes.canvas',
    version: 3,
    session,
    requestId: 2,
    type: 'surface.geometry',
    address: { ...address },
    revision: 2,
    geometry: {
      ...geometry,
      origin: { ...geometry.origin, x: Number(geometry.origin.x) },
      viewport: { ...geometry.viewport, width: Number(geometry.viewport.width) },
      clip: { ...geometry.clip, width: Number(geometry.clip.width) },
    },
  };
  try {
    expect(server.receive(selected)).toBe(true);
    expect(server.receive(positioned)).toBe(true);
    selected.address.viewId = 'view:other';
    selected.file.filename = 'other.png';
    positioned.geometry.origin.x = 100;
    positioned.geometry.viewport.width = 1;
    positioned.geometry.clip.width = 1;
    release();
    await done;
    expect(commands).toEqual([
      {
        type: 'surface.select',
        address,
        revision: 1,
        itemId: 'result:one',
        file: { filename: 'first.png', subfolder: '', type: 'output' },
      },
      { type: 'surface.geometry', address, revision: 2, geometry },
    ]);
  } finally {
    server.dispose();
  }
});
