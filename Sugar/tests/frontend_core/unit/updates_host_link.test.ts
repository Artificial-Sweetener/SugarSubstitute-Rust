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
/** Prove the page follows a host updater and settles every request on the host's own answer. */
import { expect, jest, test } from '@jest/globals';
import { isUpdatesClientMessage } from '../../../integrations/shared/UpdatesValidation.generated.js';
import { UpdatesHostLink } from '../../../frontend/comfyui/ui/core/updates/UpdatesHostLink.js';
import { UpdateSourceSlot } from '../../../frontend/comfyui/ui/core/updates/UpdateSource.js';
import type { UpdatesClientMessage } from '../../../frontend/comfyui/ui/core/embedded/UpdatesRuntime.js';

const session = 'panel-0123456789abcdef';
const envelope = { protocol: 'sugarcubes.updates', version: 1, session } as const;
const ready = {
  currentVersion: '0.4.2',
  channel: 'beta',
  lastSuccessfulCheck: { completedAt: 1762876800000 },
  status: { state: 'ready', version: '0.5.0' },
  previousVersionRunning: false,
  workflowRunning: true,
  scheduledRestart: 'none',
} as const;

/** A synchronous fixture host channel that records what the page sends. */
function channel(options: { failSend?: boolean } = {}) {
  const sent: UpdatesClientMessage[] = [];
  const listeners = new Set<(value: unknown) => void>();
  return {
    sent,
    transport: {
      session,
      send: (message: UpdatesClientMessage) => {
        if (options.failSend) throw new Error('Host channel closed.');
        sent.push(message);
      },
      subscribe: (receive: (value: unknown) => void) => {
        listeners.add(receive);
        return () => listeners.delete(receive);
      },
    },
    receive(value: unknown) {
      for (const listener of [...listeners]) listener(value);
    },
    listening: () => listeners.size,
  };
}

test('watches on start and follows only newer, schema-valid state for its own session', () => {
  const host = channel();
  const link = new UpdatesHostLink(host.transport, jest.fn());
  const changed = jest.fn();
  link.subscribe(changed);
  link.start();
  expect(host.sent).toEqual([{ ...envelope, type: 'watch' }]);
  expect(host.sent.every(isUpdatesClientMessage)).toBe(true);
  expect(link.read()).toBeNull();
  host.receive({ ...envelope, type: 'state', revision: 2, update: ready });
  expect(link.read()).toEqual({
    currentVersion: '0.4.2',
    channel: 'beta',
    lastSuccessfulCheck: 1762876800000,
    status: { state: 'ready', version: '0.5.0' },
    report: null,
    previousVersionRunning: false,
    workflowRunning: true,
    restartAfterWorkflow: false,
  });
  host.receive({
    ...envelope,
    type: 'state',
    revision: 1,
    update: { ...ready, status: { state: 'checking' } },
  });
  host.receive({
    ...envelope,
    session: 'panel-ffffffffffffffff',
    type: 'state',
    revision: 3,
    update: { ...ready, status: { state: 'checking' } },
  });
  host.receive({
    ...envelope,
    type: 'state',
    revision: 4,
    update: { ...ready, status: { state: 'trial' } },
  });
  expect(link.read()?.status).toEqual({ state: 'ready', version: '0.5.0' });
  expect(changed).toHaveBeenCalledTimes(1);
  const { lastSuccessfulCheck: _never, ...unchecked } = ready;
  host.receive({
    ...envelope,
    type: 'state',
    revision: 5,
    update: { ...unchecked, status: { state: 'up-to-date' }, scheduledRestart: 'after-workflow' },
  });
  expect(link.read()).toMatchObject({ lastSuccessfulCheck: null, restartAfterWorkflow: true });
  link.dispose();
  expect(host.listening()).toBe(0);
});

test('each request settles on the host answer that names it', async () => {
  const host = channel();
  const link = new UpdatesHostLink(host.transport, jest.fn());
  link.start();
  const check = link.check();
  const restart = link.apply('after-workflow');
  const [, checkMessage, restartMessage] = host.sent;
  expect(checkMessage).toEqual({ ...envelope, type: 'check', request: 'check-1' });
  expect(restartMessage).toEqual({
    ...envelope,
    type: 'restart',
    request: 'restart-2',
    when: 'after-workflow',
  });
  expect(host.sent.every(isUpdatesClientMessage)).toBe(true);
  host.receive({ ...envelope, type: 'rejected', request: 'restart-2', reason: 'not-ready' });
  host.receive({ ...envelope, type: 'accepted', request: 'check-9' });
  host.receive({ ...envelope, type: 'accepted', request: 'check-1' });
  await expect(restart).resolves.toEqual({ accepted: false, reason: 'not-ready' });
  await expect(check).resolves.toEqual({ accepted: true });
  link.dispose();
});

test('requests waiting at retirement, and requests after it, settle as unavailable', async () => {
  const host = channel();
  const link = new UpdatesHostLink(host.transport, jest.fn());
  await expect(link.check()).resolves.toEqual({ accepted: false, reason: 'unavailable' });
  link.start();
  const waiting = link.apply('now');
  link.dispose();
  await expect(waiting).resolves.toEqual({ accepted: false, reason: 'unavailable' });
  await expect(link.check()).resolves.toEqual({ accepted: false, reason: 'unavailable' });
});

test('a failing host channel is reported once and retires the link', async () => {
  const host = channel({ failSend: true });
  const report = jest.fn();
  const link = new UpdatesHostLink(host.transport, report);
  link.start();
  expect(report).toHaveBeenCalledTimes(1);
  await expect(link.check()).resolves.toEqual({ accepted: false, reason: 'unavailable' });
  expect(report).toHaveBeenCalledTimes(1);
  expect(host.listening()).toBe(0);
});

test('the slot holds one source at a time and announces attach and retire', () => {
  const slot = new UpdateSourceSlot();
  const changed = jest.fn();
  slot.subscribe(changed);
  const first = new UpdatesHostLink(channel().transport, jest.fn());
  const release = slot.attach(first);
  expect(slot.read()).toBe(first);
  expect(() => slot.attach(new UpdatesHostLink(channel().transport, jest.fn()))).toThrow(
    'already attached',
  );
  release();
  release();
  expect(slot.read()).toBeNull();
  expect(changed).toHaveBeenCalledTimes(2);
});
