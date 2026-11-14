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
/** Prove the page reads a host's release notes and settles every read on the host's own answer. */
import { expect, jest, test } from '@jest/globals';
import { isReleaseNotesClientMessage } from '../../../integrations/shared/ReleaseNotesValidation.generated.js';
import { HostReleaseNotesLink } from '../../../frontend/comfyui/ui/core/release_notes/HostReleaseNotesLink.js';
import type { ReleaseNotesClientMessage } from '../../../frontend/comfyui/ui/core/embedded/ReleaseNotesRuntime.js';

const session = 'panel-0123456789abcdef';
const envelope = { protocol: 'sugarcubes.release-notes', version: 1, session } as const;

/** A synchronous fixture host channel that records what the page sends. */
function channel(options: { failSend?: boolean } = {}) {
  const sent: ReleaseNotesClientMessage[] = [];
  const listeners = new Set<(value: unknown) => void>();
  return {
    sent,
    transport: {
      session,
      send: (message: ReleaseNotesClientMessage) => {
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

test('watches on start and follows only newer valid state for its own session', () => {
  const host = channel();
  const link = new HostReleaseNotesLink(host.transport, jest.fn());
  const changed = jest.fn();
  link.subscribe(changed);
  link.start();
  expect(host.sent).toEqual([{ ...envelope, type: 'watch' }]);
  const state = { ...envelope, type: 'state', product: 'SugarSubstitute', currentVersion: '0.4.2' };
  host.receive({ ...state, revision: 2 });
  expect(link.read()).toEqual({ product: 'SugarSubstitute', currentVersion: '0.4.2' });
  host.receive({ ...state, revision: 2, currentVersion: '9.9.9' });
  host.receive({ ...state, revision: 3, session: 'panel-other-session-0000' });
  host.receive({ ...state, revision: 3, currentVersion: 'V1' });
  expect(link.read()?.currentVersion).toBe('0.4.2');
  expect(changed).toHaveBeenCalledTimes(1);
});

test('each read settles on the answer that names it, notes or failure', async () => {
  const host = channel();
  const link = new HostReleaseNotesLink(host.transport, jest.fn());
  link.start();
  const first = link.load('0.3.0');
  const second = link.load('0.4.2');
  expect(host.sent.slice(1)).toEqual([
    { ...envelope, type: 'load', request: 'load-1', since: '0.3.0' },
    { ...envelope, type: 'load', request: 'load-2', since: '0.4.2' },
  ]);
  expect(host.sent.every(isReleaseNotesClientMessage)).toBe(true);
  host.receive({ ...envelope, type: 'failed', request: 'load-2', reason: 'unavailable' });
  host.receive({
    ...envelope,
    type: 'notes',
    request: 'load-1',
    releases: [{ version: '0.4.2', notes: '## 0.4.2\n' }],
    omitted: 0,
  });
  host.receive({ ...envelope, type: 'failed', request: 'load-9', reason: 'unavailable' });
  await expect(first).resolves.toEqual({
    kind: 'notes',
    releases: [{ version: '0.4.2', notes: '## 0.4.2\n' }],
    omitted: 0,
  });
  await expect(second).resolves.toEqual({ kind: 'failed', reason: 'unavailable' });
});

test('the host asking to show what is new reaches every open listener', () => {
  const host = channel();
  const link = new HostReleaseNotesLink(host.transport, jest.fn());
  const open = jest.fn();
  const stop = link.onOpen(open);
  link.start();
  host.receive({ ...envelope, type: 'open' });
  expect(open).toHaveBeenCalledTimes(1);
  stop();
  host.receive({ ...envelope, type: 'open' });
  expect(open).toHaveBeenCalledTimes(1);
});

test('dispose stops listening and settles waiting reads as unavailable', async () => {
  const host = channel();
  const link = new HostReleaseNotesLink(host.transport, jest.fn());
  link.start();
  const waiting = link.load('0.3.0');
  link.dispose();
  expect(host.listening()).toBe(0);
  await expect(waiting).resolves.toEqual({ kind: 'failed', reason: 'unavailable' });
  await expect(link.load('0.3.0')).resolves.toEqual({ kind: 'failed', reason: 'unavailable' });
});

test('a closed channel retires the link once and reads as unavailable', async () => {
  const host = channel({ failSend: true });
  const report = jest.fn();
  const link = new HostReleaseNotesLink(host.transport, report);
  link.start();
  expect(report).toHaveBeenCalledTimes(1);
  await expect(link.load('0.3.0')).resolves.toEqual({ kind: 'failed', reason: 'unavailable' });
});
