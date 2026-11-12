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
/** Prove the conformance kit passes a capable host and a limited one, and fails a broken one at once. */
import { expect, test } from '@jest/globals';
import {
  CONFORMANCE_CHECKS,
  HostConformanceFailure,
  runHostConformance,
  type ConformanceHostTransport,
} from '../../../integrations/shared/conformance/HostConformance.js';
import { inMemoryHostChannel } from '../../../integrations/shared/conformance/InMemoryHostChannel.js';
import { CanvasCancellation } from '../../../integrations/shared/CanvasCancellation.js';
import { EMPTY_CANVAS_SURFACE_CAPABILITIES } from '../../../integrations/shared/CanvasSurfaceDiscovery.js';
import {
  HOST_CAPABILITIES,
  HOST_PROTOCOLS,
} from '../../../integrations/contracts/HostDescription.js';
import {
  canvasWithoutSurface,
  conformanceSession as session,
  memoryHost,
  updatesHost,
} from '../support/ConformanceHosts.js';

const presentation = {
  pluginMode: true,
  visualStyle: 'fluent',
  accentColor: '#0078d4',
  resizeCornerSize: 8,
} as const;

/** Read each check's outcome by name. */
function outcomes(results: readonly { check: string; outcome: string }[]): Record<string, string> {
  return Object.fromEntries(results.map(({ check, outcome }) => [check, outcome]));
}

test('a host serving every protocol passes every check it declares', async () => {
  const report = await runHostConformance(
    inMemoryHostChannel({
      session,
      description: {
        protocols: [
          HOST_PROTOCOLS.panel,
          HOST_PROTOCOLS.canvas,
          HOST_PROTOCOLS.memory,
          HOST_PROTOCOLS.updates,
        ],
        capabilities: [
          { name: HOST_CAPABILITIES.hostStyles },
          { name: HOST_CAPABILITIES.systemAccent },
        ],
      },
      presentation,
      handlers: (send) => [canvasWithoutSurface(send), memoryHost(send), updatesHost(send)],
    }),
    { session },
  );
  expect(report.results.map(({ check }) => check)).toEqual(CONFORMANCE_CHECKS);
  expect(outcomes(report.results)).toEqual({
    'host-description': 'passed',
    'panel-presentation': 'passed',
    'canvas-discovery': 'passed',
    'canvas-step-down': 'passed',
    memory: 'passed',
    updates: 'passed',
    'capability.presentation.host-styles': 'passed',
    'capability.presentation.system-accent': 'passed',
    'capability.output.host-surface': 'not-offered',
  });
  expect(report.transcript[0]).toEqual({
    from: 'page',
    message: { protocol: 'sugarcubes.panel', version: 3, session, type: 'ready' },
  });
});

test('a host serving only the panel passes, with everything else not offered', async () => {
  const report = await runHostConformance(
    inMemoryHostChannel({
      session,
      description: { protocols: [HOST_PROTOCOLS.panel], capabilities: [] },
      presentation,
    }),
    { session },
  );
  expect(outcomes(report.results)).toMatchObject({
    'host-description': 'passed',
    'panel-presentation': 'passed',
    'canvas-discovery': 'not-offered',
    'canvas-step-down': 'not-offered',
    memory: 'not-offered',
    updates: 'not-offered',
  });
});

test('an invalid reply fails its check at once', async () => {
  const run = runHostConformance(
    inMemoryHostChannel({
      session,
      description: { protocols: [HOST_PROTOCOLS.panel, HOST_PROTOCOLS.memory], capabilities: [] },
      presentation,
      handlers: (send) => [memoryHost(send, { path: 'C:/Users/someone' })],
    }),
    { session },
  );
  await expect(run).rejects.toThrow(HostConformanceFailure);
  await expect(run).rejects.toMatchObject({ check: 'memory' });
});

test('a message for a protocol the host did not declare fails at once', async () => {
  const host = inMemoryHostChannel({
    session,
    description: { protocols: [HOST_PROTOCOLS.panel], capabilities: [] },
    presentation,
  });
  let deliver: (value: unknown) => void = () => undefined;
  const undeclared: ConformanceHostTransport = {
    send: (message) => {
      host.send(message);
      memoryHost(deliver).receive({
        protocol: 'sugarcubes.memory',
        version: 1,
        session,
        type: 'usage',
        usedBytes: 0,
        demandBytes: 0,
        consumers: [],
      });
    },
    subscribe: (receive) => {
      deliver = receive;
      return host.subscribe(receive);
    },
  };
  await expect(runHostConformance(undeclared, { session })).rejects.toThrow(
    'for a protocol it did not declare',
  );
});

test('a host that cannot step down from an unknown schema fails the step-down check', async () => {
  const run = runHostConformance(
    inMemoryHostChannel({
      session,
      description: { protocols: [HOST_PROTOCOLS.panel, HOST_PROTOCOLS.canvas], capabilities: [] },
      presentation,
      handlers: (send) => [
        {
          receive: (value) => {
            const request = value as { type?: string; requestId?: number; schema?: number };
            if (request.type !== 'surface.capabilities.request') return false;
            send({
              protocol: 'sugarcubes.canvas',
              version: 3,
              session,
              requestId: request.requestId,
              type: 'surface.capabilities.reply',
              status: request.schema === 3 ? 'unsupported' : 'error',
              capabilities: { ...EMPTY_CANVAS_SURFACE_CAPABILITIES, schema: 3 },
              message: '',
            });
            return true;
          },
        },
      ],
    }),
    { session },
  );
  await expect(run).rejects.toMatchObject({ check: 'canvas-step-down' });
});

test('declaring host-presented output without surface discovery fails that capability', async () => {
  await expect(
    runHostConformance(
      inMemoryHostChannel({
        session,
        description: {
          protocols: [HOST_PROTOCOLS.panel],
          capabilities: [{ name: HOST_CAPABILITIES.hostOutputSurface }],
        },
        presentation,
      }),
      { session },
    ),
  ).rejects.toMatchObject({ check: 'capability.output.host-surface' });
});

test('a run still waiting for a silent host ends when cancelled', async () => {
  const cancellation = new CanvasCancellation();
  const silent: ConformanceHostTransport = {
    send: () => undefined,
    subscribe: () => () => undefined,
  };
  const run = runHostConformance(silent, { session, cancellation });
  cancellation.cancel();
  await expect(run).rejects.toThrow('The conformance run was cancelled.');
});
