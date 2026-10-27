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
/** Prove panel admission recovery retains loaded guests and fences retired renderer callbacks. */
import { jest } from '@jest/globals';
import { randomUUID } from 'node:crypto';
import type { ConnectionState } from '../../../integrations/gimp/src/application/CompanionConnection.js';
import { GimpPanelConnection } from '../../../integrations/gimp/src/infrastructure/GimpPanelConnection.js';
import type { PanelMessage } from '../../../integrations/contracts/PanelProtocol.generated.js';
import { isPanelMessage } from '../../../integrations/shared/PanelValidation.generated.js';

/** Carry an external Tauri channel callback without reproducing transport behavior. */
class Channel {
  onmessage: (value: unknown) => void = () => {};
}

/** Replace only native relay commands while keeping production connection and heartbeat owners. */
function fixture() {
  const channels: { session: string; channel: Channel }[] = [];
  const visible: boolean[] = [];
  const open = jest.fn<() => Promise<void>>().mockResolvedValue(undefined);
  const probe = jest.fn<() => Promise<ConnectionState>>().mockResolvedValue({
    phase: 'connecting',
    address: 'http://127.0.0.1:8226/',
    message: 'Waiting for SugarCubes…',
  });
  const report = jest.fn();
  const native = { execute: jest.fn(async () => ({ kind: 'released' as const })) };
  /** Deliver schema-owned panel replies to one explicitly selected renderer generation. */
  function receive(
    message: Pick<PanelMessage, 'type'> & { readonly connected?: boolean },
    index = channels.length - 1,
  ): void {
    const target = channels[index];
    if (!target) throw new Error('Missing admitted panel callback.');
    target.channel.onmessage({
      protocol: 'sugarcubes.panel',
      version: 3,
      session: target.session,
      ...message,
    });
  }
  /** Model fixed native commands and correlate relay responses without implementing application policy. */
  async function invoke(
    command: string,
    args: Readonly<Record<string, unknown>> = {},
  ): Promise<unknown> {
    if (command === 'panel_open') {
      if (typeof args.session !== 'string' || !(args.channel instanceof Channel))
        throw new Error('Invalid panel fixture admission.');
      channels.push({ session: args.session, channel: args.channel });
      return open();
    }
    if (command === 'connect_comfy') return probe();
    if (command === 'panel_visible') {
      if (typeof args.visible !== 'boolean') throw new Error('Invalid fixture visibility.');
      visible.push(args.visible);
      return;
    }
    if (command === 'panel_send' && isPanelMessage(args.message)) {
      if (args.message.type === 'configure') receive({ type: 'applied' });
      else if (args.message.type === 'ping') receive({ type: 'pong', connected: true });
      return;
    }
    throw new Error('Unexpected panel fixture command: ' + command);
  }
  const timers = new Map<number, ReturnType<typeof setInterval>>();
  let nextTimer = 0;
  /** Match the browser's numeric timer handle while retaining the Node fixture's cleanup owner. */
  function startHeartbeat(callback: () => void, milliseconds: number): number {
    const id = ++nextTimer;
    timers.set(id, setInterval(callback, milliseconds));
    return id;
  }
  /** Remove exactly the heartbeat owned by the retired renderer generation. */
  function stopHeartbeat(id: number): void {
    const timer = timers.get(id);
    if (timer !== undefined) clearInterval(timer);
    timers.delete(id);
  }
  const renderer = {
    crypto: { randomUUID },
    performance: { now: () => performance.now() },
    setInterval: startHeartbeat,
    clearInterval: stopHeartbeat,
    __TAURI__: { core: { Channel, invoke } },
  };
  const owner = new GimpPanelConnection(renderer, native, report);
  const present = jest.fn<(state: ConnectionState) => void>();
  /** Complete actual shared negotiation after the native guest is admitted. */
  async function handshake(): Promise<void> {
    receive({ type: 'ready' });
    await jest.advanceTimersByTimeAsync(0);
  }
  /** Dispose the fixture's owned callbacks and heartbeat timers. */
  function close(): void {
    owner.dispose();
  }
  return {
    owner,
    open,
    probe,
    native,
    present,
    report,
    channels,
    visible,
    receive,
    handshake,
    close,
  };
}

beforeEach(() => {
  jest.useFakeTimers();
});
afterEach(() => {
  jest.useRealTimers();
});

test('retries a failed native panel admission immediately with a fresh session', async () => {
  const f = fixture();
  const failure = new Error('Native panel admission failed.');
  f.open.mockRejectedValueOnce(failure);
  try {
    await expect(f.owner.connect(f.present)).rejects.toBe(failure);
    await expect(f.owner.connect(f.present)).resolves.toBeUndefined();
    await f.handshake();
    expect(f.open).toHaveBeenCalledTimes(2);
    expect(f.probe).toHaveBeenCalledTimes(1);
    expect(f.channels[0]?.session).not.toBe(f.channels[1]?.session);
    expect(f.present).toHaveBeenLastCalledWith(expect.objectContaining({ phase: 'connected' }));
    expect(f.report).toHaveBeenCalledWith(failure);
  } finally {
    f.close();
  }
  expect(jest.getTimerCount()).toBe(0);
});

test('keeps the loaded guest and native adapter through a backend disconnect and retry', async () => {
  const f = fixture();
  try {
    await f.owner.connect(f.present);
    await f.handshake();
    f.receive({ type: 'pong', connected: false });
    expect(f.present).toHaveBeenLastCalledWith(expect.objectContaining({ phase: 'disconnected' }));
    expect(f.visible.at(-1)).toBe(false);
    await f.owner.connect(f.present);
    f.receive({ type: 'pong', connected: true });
    expect(f.open).toHaveBeenCalledTimes(1);
    expect(f.probe).toHaveBeenCalledTimes(2);
    expect(f.channels).toHaveLength(1);
    expect(f.native.execute).not.toHaveBeenCalled();
    expect(f.present).toHaveBeenLastCalledWith(expect.objectContaining({ phase: 'connected' }));
    expect(f.visible.at(-1)).toBe(true);
  } finally {
    f.close();
  }
  expect(jest.getTimerCount()).toBe(0);
});

test('retains a loaded guest when a retry probe rejects and recovers on the next attempt', async () => {
  const f = fixture();
  const failure = new Error('Native readiness worker failed.');
  try {
    await f.owner.connect(f.present);
    await f.handshake();
    f.probe.mockRejectedValueOnce(failure);
    await expect(f.owner.connect(f.present)).rejects.toBe(failure);
    await f.owner.connect(f.present);
    expect(f.open).toHaveBeenCalledTimes(1);
    expect(f.channels).toHaveLength(1);
    expect(jest.getTimerCount()).toBe(1);
    expect(f.present).toHaveBeenLastCalledWith(expect.objectContaining({ phase: 'connected' }));
  } finally {
    f.close();
  }
  expect(jest.getTimerCount()).toBe(0);
});

test('starts a fresh session after handshake timeout and ignores the retired callback', async () => {
  const f = fixture();
  try {
    await f.owner.connect(f.present);
    await jest.advanceTimersByTimeAsync(30_000);
    expect(f.present).toHaveBeenLastCalledWith(expect.objectContaining({ phase: 'disconnected' }));
    await f.owner.connect(f.present);
    await f.handshake();
    f.present.mockClear();
    f.receive({ type: 'pong', connected: false }, 0);
    expect(f.present).not.toHaveBeenCalled();
    expect(f.open).toHaveBeenCalledTimes(2);
    expect(f.channels[0]?.session).not.toBe(f.channels[1]?.session);
    expect(jest.getTimerCount()).toBe(1);
  } finally {
    f.close();
  }
  expect(jest.getTimerCount()).toBe(0);
});

test('does not retire a replacement when an older native admission rejects late', async () => {
  const f = fixture();
  const failure = new Error('Retired native admission failed.');
  let rejectInitial: (error: Error) => void = () => {
    throw new Error('Missing initial admission.');
  };
  f.open.mockImplementationOnce(
    () =>
      new Promise<void>((_resolve, reject) => {
        rejectInitial = reject;
      }),
  );
  try {
    const first = f.owner.connect(f.present);
    const rejected = expect(first).rejects.toBe(failure);
    await jest.advanceTimersByTimeAsync(30_000);
    await f.owner.connect(f.present);
    await f.handshake();
    f.present.mockClear();
    rejectInitial(failure);
    await rejected;
    expect(f.present).not.toHaveBeenCalled();
    expect(jest.getTimerCount()).toBe(1);
    f.receive({ type: 'pong', connected: false });
    expect(f.present).toHaveBeenLastCalledWith(expect.objectContaining({ phase: 'disconnected' }));
    expect(f.open).toHaveBeenCalledTimes(2);
  } finally {
    f.close();
  }
  expect(jest.getTimerCount()).toBe(0);
});

test('closing during admission prevents a late setup from probing or reviving the renderer', async () => {
  const f = fixture();
  let admit: () => void = () => {
    throw new Error('Missing pending admission.');
  };
  f.open.mockImplementationOnce(
    () =>
      new Promise<void>((resolve) => {
        admit = resolve;
      }),
  );
  try {
    const pending = f.owner.connect(f.present);
    const rejected = expect(pending).rejects.toThrow('closed');
    f.owner.dispose();
    f.present.mockClear();
    admit();
    await rejected;
    await f.owner.connect(f.present);
    expect(f.probe).not.toHaveBeenCalled();
    expect(f.present).not.toHaveBeenCalled();
    expect(f.open).toHaveBeenCalledTimes(1);
    expect(jest.getTimerCount()).toBe(0);
  } finally {
    f.close();
  }
});
