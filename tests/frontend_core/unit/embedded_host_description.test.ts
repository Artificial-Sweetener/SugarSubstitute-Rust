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
/** Prove the embedded page uses only what its host declares, and settles a declaration exactly once. */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  EmbeddedPanelHost,
  startEmbeddedPanel,
} from '../../../frontend/comfyui/ui/core/embedded/EmbeddedPanelHost.js';
import { EmbeddedPresentation } from '../../../frontend/comfyui/ui/core/embedded/EmbeddedPresentation.js';
import { EmbeddedCanvasConnection } from '../../../frontend/comfyui/ui/core/embedded/EmbeddedCanvasConnection.js';
import { HostDescriptionState } from '../../../frontend/comfyui/ui/core/embedded/HostDescriptionState.js';
import {
  DeclaredHost,
  HOST_CAPABILITIES,
  HOST_PROTOCOLS,
} from '../../../frontend/comfyui/ui/core/embedded/PanelRuntime.js';
import { MemoryBudgetRegistry } from '../../../frontend/comfyui/ui/core/memory/MemoryBudgetRegistry.js';
import { UpdateSourceSlot } from '../../../frontend/comfyui/ui/core/updates/UpdateSource.js';

const session = '00000000-0000-4000-8000-000000000001';

afterEach(() => {
  window.history.replaceState(null, '', '/');
  jest.useRealTimers();
});

/** Describe a fixture host with the given declarations. */
function description(
  protocols: readonly { name: string; version: number }[],
  capabilities: readonly string[] = [],
): unknown {
  return {
    protocol: 'sugarcubes.host',
    version: 1,
    session,
    type: 'description',
    protocols,
    capabilities: capabilities.map((name) => ({ name })),
  };
}

const configure = {
  protocol: 'sugarcubes.panel',
  version: 3,
  session,
  type: 'configure',
  presentation: {
    pluginMode: true,
    visualStyle: 'gimp',
    photoProPalette: 'dark-gray',
    accentColor: null,
    resizeCornerSize: 0,
  },
};

/** Run one embedded page against a synchronous fixture host transport. */
function page(options: { memory?: boolean } = {}) {
  const updates = new UpdateSourceSlot();
  jest.useFakeTimers();
  const sent: string[] = [];
  const listeners = new Set<(value: unknown) => void>();
  const host = new HostDescriptionState();
  const presentation = new EmbeddedPresentation();
  const canvas = new EmbeddedCanvasConnection();
  const report = jest.fn();
  const panel = new EmbeddedPanelHost(
    {
      window,
      host,
      presentation,
      canvas,
      updates,
      connected: () => true,
      report,
      ...(options.memory
        ? {
            memory: {
              registry: new MemoryBudgetRegistry(),
              preferences: () => ({
                read: () => ({ mode: 'share', percent: 25 }) as const,
                subscribe: () => () => undefined,
              }),
            },
          }
        : {}),
    },
    {
      session,
      send: (message) => {
        sent.push(message.protocol);
      },
      subscribe: (receive) => {
        listeners.add(receive);
        return () => listeners.delete(receive);
      },
    },
  );
  panel.setup();
  const receive = (value: unknown): void => {
    for (const listener of [...listeners]) listener(value);
  };
  const protocols = (): string[] => [...new Set(sent)];
  return { panel, host, presentation, canvas, updates, report, sent, receive, protocols };
}

test('a host that configures without describing itself serves only the panel', () => {
  const f = page({ memory: true });
  f.receive(configure);
  expect(f.host.read()).toBe(DeclaredHost.undeclared);
  expect(f.presentation.read()).toBeNull();
  expect(f.protocols()).toEqual(['sugarcubes.panel']);
  expect(f.canvas.masks).toBeNull();
  expect(f.report).not.toHaveBeenCalled();
  f.panel.dispose();
});

test('a described host gets its declared presentation, canvas, masks and memory budget', () => {
  const f = page({ memory: true });
  f.receive(
    description(
      [
        HOST_PROTOCOLS.panel,
        HOST_PROTOCOLS.canvas,
        HOST_PROTOCOLS.canvasMasks,
        HOST_PROTOCOLS.memory,
      ],
      [HOST_CAPABILITIES.hostStyles],
    ),
  );
  expect(f.protocols()).toEqual(['sugarcubes.panel', 'sugarcubes.memory']);
  f.receive(configure);
  expect(f.presentation.read()?.visualStyle).toBe('gimp');
  expect(f.protocols()).toEqual(['sugarcubes.panel', 'sugarcubes.memory', 'sugarcubes.canvas']);
  expect(f.canvas.masks).not.toBeNull();
  expect(f.report).not.toHaveBeenCalled();
  f.panel.dispose();
});

test('only a host that declares the update protocol supplies the Updates entry', () => {
  const plain = page();
  plain.receive(description([HOST_PROTOCOLS.panel]));
  plain.receive(configure);
  expect(plain.updates.read()).toBeNull();
  expect(plain.protocols()).toEqual(['sugarcubes.panel']);
  plain.panel.dispose();
  const updating = page();
  updating.receive(description([HOST_PROTOCOLS.panel, HOST_PROTOCOLS.updates]));
  expect(updating.updates.read()?.applies).toBe('host-restart');
  expect(updating.protocols()).toEqual(['sugarcubes.panel', 'sugarcubes.updates']);
  updating.panel.dispose();
  expect(updating.updates.read()).toBeNull();
  expect(updating.report).not.toHaveBeenCalled();
});

test('a canvas host that does not serve masks gets no mask requests', () => {
  const f = page();
  f.receive(description([HOST_PROTOCOLS.panel, HOST_PROTOCOLS.canvas]));
  f.receive(configure);
  expect(f.protocols()).toEqual(['sugarcubes.panel', 'sugarcubes.canvas']);
  expect(f.canvas.masks).toBeNull();
  f.panel.dispose();
});

test('a description that repeats a name is reported and settles on the panel alone', () => {
  const f = page({ memory: true });
  f.receive(description([HOST_PROTOCOLS.memory, HOST_PROTOCOLS.memory]));
  expect(f.report).toHaveBeenCalledWith(
    new Error('The host described itself with a repeated protocol or capability.'),
  );
  expect(f.host.read()).toBe(DeclaredHost.undeclared);
  expect(f.protocols()).toEqual(['sugarcubes.panel']);
  f.panel.dispose();
});

test('a repeated description is accepted silently and a contradicting one is reported', () => {
  const f = page();
  const first = description([HOST_PROTOCOLS.panel], [HOST_CAPABILITIES.hostStyles]);
  f.receive(first);
  f.receive(configure);
  f.receive(first);
  expect(f.report).not.toHaveBeenCalled();
  f.receive(description([HOST_PROTOCOLS.panel]));
  expect(f.report).toHaveBeenCalledWith(
    new Error('The host changed its description during the panel session.'),
  );
  expect(f.host.read()?.declares(HOST_CAPABILITIES.hostStyles)).toBe(true);
  f.panel.dispose();
});

test('a description for another session is not settled', () => {
  const f = page();
  f.receive({ ...(description([HOST_PROTOCOLS.panel]) as object), session: 'stale-session-00000' });
  expect(f.host.read()).toBeNull();
  f.panel.dispose();
});

test('a page outside any host settles on the standalone declaration at once', () => {
  const host = new HostDescriptionState();
  const changed = jest.fn();
  host.subscribe(changed);
  expect(
    startEmbeddedPanel({
      window,
      host,
      presentation: new EmbeddedPresentation(),
      connected: () => true,
      report: jest.fn(),
    }),
  ).toBeNull();
  expect(host.read()).toBe(DeclaredHost.standalone);
  expect(changed).toHaveBeenCalledTimes(1);
});

test('an ambiguous transport choice is treated as no host', () => {
  window.history.replaceState(null, '', '/?sc-panel-transport=native&sc-panel-transport=native');
  const host = new HostDescriptionState();
  startEmbeddedPanel({
    window,
    host,
    presentation: new EmbeddedPresentation(),
    connected: () => true,
    report: jest.fn(),
  });
  expect(host.read()).toBe(DeclaredHost.standalone);
});
