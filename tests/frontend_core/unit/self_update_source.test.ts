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
/** Prove SugarCubes' own updater feeds the Updates entry only where no host updater exists. */
import { expect, test } from '@jest/globals';
import { DeclaredHost } from '../../../integrations/shared/HostDescription.js';
import { HOST_PROTOCOLS } from '../../../frontend/comfyui/ui/core/embedded/PanelRuntime.js';
import { HostDescriptionState } from '../../../frontend/comfyui/ui/core/embedded/HostDescriptionState.js';
import { UpdateSourceSlot } from '../../../frontend/comfyui/ui/core/updates/UpdateSource.js';
import {
  attachSelfUpdateWhenUnhosted,
  comfySelfUpdateTransport,
  decodeSelfUpdateState,
  selfUpdateSnapshot,
  type SelfUpdateReply,
  type SelfUpdateTransport,
} from '../../../frontend/comfyui/ui/core/updates/SelfUpdateSource.js';

function state(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    schema: 1,
    currentVersion: '0.15.3',
    state: 'up-to-date',
    availableVersion: null,
    failure: null,
    notManaged: null,
    lastCheckedAt: 1762876800000,
    revision: 1,
    notice: null,
    ...overrides,
  };
}

/** A backend fixture that answers each request with the next queued state. */
function backend(replies: Record<string, unknown>[]) {
  const requests: { method: string; path: string; body: unknown }[] = [];
  const pushed = new Set<(payload: unknown) => void>();
  const reconnects = new Set<() => void>();
  let next = 0;
  const transport: SelfUpdateTransport = {
    request: (method, path, body): Promise<SelfUpdateReply> => {
      requests.push({ method, path, body });
      const reply = replies[Math.min(next, replies.length - 1)];
      next += 1;
      return Promise.resolve({ status: 200, body: reply });
    },
    subscribe: (receive) => {
      pushed.add(receive);
      return () => pushed.delete(receive);
    },
    subscribeReconnect: (reconnected) => {
      reconnects.add(reconnected);
      return () => reconnects.delete(reconnected);
    },
  };
  return {
    transport,
    requests,
    push: (payload: unknown) => {
      for (const receive of [...pushed]) receive(payload);
    },
    reconnect: () => {
      for (const reconnected of [...reconnects]) reconnected();
    },
    listening: () => pushed.size,
  };
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

test('attaches only after a standalone description settles, then reads and checks', async () => {
  const host = new HostDescriptionState();
  const updates = new UpdateSourceSlot();
  const fixture = backend([state({ state: 'idle', lastCheckedAt: null }), state({ revision: 2 })]);
  attachSelfUpdateWhenUnhosted({ host, updates, transport: fixture.transport, report: () => {} });
  expect(updates.read()).toBeNull();
  host.settle(DeclaredHost.standalone);
  const source = updates.read();
  expect(source?.applies).toBe('comfyui-restart');
  await flush();
  expect(fixture.requests.map((request) => request.path)).toEqual([
    '/sugarcubes/self-update',
    '/sugarcubes/self-update/check',
  ]);
  expect(fixture.requests[1]?.body).toEqual({ force: false });
  expect(source?.read()?.status).toEqual({ state: 'up-to-date' });
});

test('never competes with a host that serves the updates protocol', () => {
  const host = new HostDescriptionState();
  const updates = new UpdateSourceSlot();
  const fixture = backend([state()]);
  const declared = DeclaredHost.read({ protocols: [HOST_PROTOCOLS.updates], capabilities: [] });
  expect(declared).not.toBeNull();
  if (declared) host.settle(declared);
  attachSelfUpdateWhenUnhosted({ host, updates, transport: fixture.transport, report: () => {} });
  expect(updates.read()).toBeNull();
  expect(fixture.requests).toEqual([]);
});

test('follows pushed states in revision order and applies the ready release', async () => {
  const host = new HostDescriptionState();
  host.settle(DeclaredHost.standalone);
  const updates = new UpdateSourceSlot();
  const fixture = backend([
    state({ state: 'ready', availableVersion: '0.16.0', revision: 3 }),
    state({ state: 'ready', availableVersion: '0.16.0', revision: 3 }),
    state({ state: 'scheduled', availableVersion: '0.16.0', revision: 4 }),
  ]);
  attachSelfUpdateWhenUnhosted({ host, updates, transport: fixture.transport, report: () => {} });
  await flush();
  const source = updates.read();
  fixture.push(state({ state: 'checking', revision: 2 }));
  expect(source?.read()?.status).toEqual({ state: 'ready', version: '0.16.0' });
  await expect(source?.apply('now')).resolves.toEqual({ accepted: true });
  expect(fixture.requests.at(-1)).toEqual({
    method: 'POST',
    path: '/sugarcubes/self-update/install',
    body: { version: '0.16.0' },
  });
  expect(source?.read()?.status).toEqual({ state: 'restart-comfyui', version: '0.16.0' });
  await expect(source?.apply('now')).resolves.toEqual({ accepted: false, reason: 'not-ready' });
});

test('maps every folder the updater leaves alone to a plain reason', () => {
  const cases: [string, string][] = [
    ['git-checkout', 'git-checkout'],
    ['changed-files', 'local-changes'],
    ['linked-folder', 'linked-folder'],
    ['linked-content', 'linked-folder'],
    ['not-registry-install', 'not-registry-install'],
    ['invalid-tracking', 'not-registry-install'],
    ['missing', 'not-registry-install'],
  ];
  for (const [backendReason, reason] of cases) {
    const decoded = decodeSelfUpdateState(
      state({ state: 'not-managed', notManaged: backendReason }),
    );
    expect(decoded && selfUpdateSnapshot(decoded)?.status).toEqual({ state: 'unmanaged', reason });
  }
});

test('reports a start that put the previous version back', () => {
  const decoded = decodeSelfUpdateState(
    state({
      notice: {
        kind: 'restored',
        reason: 'did-not-start',
        version: '0.16.0',
        fromVersion: '0.15.3',
        message: "SugarCubes 0.16.0 didn't start properly, so the previous version was put back.",
      },
    }),
  );
  const snapshot = decoded ? selfUpdateSnapshot(decoded) : null;
  expect(snapshot?.previousVersionRunning).toBe(true);
  expect(snapshot?.report).toEqual({
    version: '0.16.0',
    phase: 'start',
    causes: [
      { message: "SugarCubes 0.16.0 didn't start properly, so the previous version was put back." },
    ],
  });
});

test('rejects malformed states whole and refetches after a reconnect', async () => {
  for (const bad of [
    state({ schema: 2 }),
    state({ currentVersion: '../x' }),
    state({ revision: 0 }),
    state({ failure: 'boom' }),
    state({ state: 'ready', availableVersion: null }),
    state({ state: 'not-managed', notManaged: 'unknown' }),
  ]) {
    const decoded = decodeSelfUpdateState(bad);
    expect(decoded === null || selfUpdateSnapshot(decoded) === null).toBe(true);
  }
  const host = new HostDescriptionState();
  host.settle(DeclaredHost.standalone);
  const updates = new UpdateSourceSlot();
  const fixture = backend([state()]);
  const release = attachSelfUpdateWhenUnhosted({
    host,
    updates,
    transport: fixture.transport,
    report: () => {},
  });
  await flush();
  const before = fixture.requests.length;
  fixture.reconnect();
  expect(fixture.requests.length).toBe(before + 1);
  release();
  expect(updates.read()).toBeNull();
  expect(fixture.listening()).toBe(0);
});

test('a failing connection settles requests as unavailable and is reported', async () => {
  const reported: unknown[] = [];
  const host = new HostDescriptionState();
  host.settle(DeclaredHost.standalone);
  const updates = new UpdateSourceSlot();
  const transport: SelfUpdateTransport = {
    request: () => Promise.reject(new Error('offline')),
    subscribe: () => () => {},
    subscribeReconnect: () => () => {},
  };
  attachSelfUpdateWhenUnhosted({
    host,
    updates,
    transport,
    report: (error) => reported.push(error),
  });
  await expect(updates.read()?.check()).resolves.toEqual({
    accepted: false,
    reason: 'unavailable',
  });
  expect(reported.length).toBeGreaterThan(0);
});

test('adapts the ComfyUI api object and its websocket event detail', async () => {
  const listeners = new Map<string, (event: unknown) => void>();
  const api = {
    fetchApi: (path: string, init: RequestInit) =>
      Promise.resolve({ status: 200, json: () => Promise.resolve({ path, init }) }),
    addEventListener: (type: string, listener: (event: unknown) => void) =>
      listeners.set(type, listener),
    removeEventListener: (type: string) => listeners.delete(type),
  };
  const transport = comfySelfUpdateTransport(api);
  expect(comfySelfUpdateTransport({})).toBeNull();
  const reply = await transport?.request('POST', '/sugarcubes/self-update/check', { force: true });
  expect(reply?.status).toBe(200);
  const received: unknown[] = [];
  const release = transport?.subscribe((payload) => received.push(payload));
  listeners.get('sugarcubes.self_update')?.({ detail: { revision: 9 } });
  expect(received).toEqual([{ revision: 9 }]);
  release?.();
  expect(listeners.has('sugarcubes.self_update')).toBe(false);
});
