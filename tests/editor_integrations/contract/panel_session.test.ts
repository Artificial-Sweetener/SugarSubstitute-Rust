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
/** Prove one panel protocol across capable and deliberately limited in-memory applications. */
import { readFileSync } from 'node:fs';
import { panelBindings } from '../../../scripts/integrations/generate-panel-contract.js';
import {
  PanelClientSession,
  PanelHostSession,
  type PanelState,
} from '../../../integrations/shared/PanelSession.js';
import { isPanelMessage } from '../../../integrations/shared/PanelValidation.generated.js';
import type {
  PanelMessage,
  PanelPresentation,
} from '../../../integrations/contracts/PanelProtocol.generated.js';

const session = '00000000-0000-4000-8000-000000000001';
const presentation: PanelPresentation = {
  pluginMode: true,
  visualStyle: 'photopro',
  photoProPalette: 'medium-gray',
  accentColor: null,
  resizeCornerSize: 0,
};

/** Build a synchronous transport to exercise reentrant delivery as well as application capability limits. */
function application(supported: boolean): {
  host: PanelHostSession;
  client: PanelClientSession;
  states: PanelState[];
  applied: PanelPresentation[];
  setConnected(value: boolean): void;
} {
  const states: PanelState[] = [];
  const applied: PanelPresentation[] = [];
  let connected = true;
  const toClient: PanelMessage[] = [];
  const host = new PanelHostSession(
    session,
    presentation,
    { send: (message) => toClient.push(message), present: (state) => states.push(state) },
    0,
  );
  const client = new PanelClientSession(session, {
    send: (message) => {
      host.receive(message, 0);
      while (toClient.length) client.receive(toClient.shift());
    },
    apply: (value) => {
      if (supported) applied.push(value);
      return supported;
    },
    connected: () => connected,
    report: (error) => {
      throw error;
    },
  });
  return {
    host,
    client,
    states,
    applied,
    setConnected: (value) => {
      connected = value;
    },
  };
}

test.each([true, false])('negotiates explicit presentation capability: %s', (supported) => {
  const app = application(supported);
  app.client.ready();
  expect(app.states).toEqual(['loading', supported ? 'connected' : 'failed']);
  expect(app.applied).toEqual(supported ? [presentation] : []);
});

test('monitors disconnect and recovery without applying presentation or recreating workflows', () => {
  const app = application(true);
  app.client.ready();
  app.setConnected(false);
  app.host.tick(5000);
  // The native send port is deliberately queued; ready drains it through the same transport.
  app.client.receive({ protocol: 'sugarcubes.panel', version: 3, session, type: 'ping' });
  expect(app.states.at(-1)).toBe('disconnected');
  app.setConnected(true);
  app.client.receive({ protocol: 'sugarcubes.panel', version: 3, session, type: 'ping' });
  expect(app.states.at(-1)).toBe('connected');
  expect(app.applied).toHaveLength(1);
  app.host.tick(20000);
  expect(app.states.at(-1)).toBe('disconnected');
});

test('bounds missing frontend startup and rejects malformed, stale, and reversed messages', () => {
  const sent: PanelMessage[] = [];
  const states: PanelState[] = [];
  const host = new PanelHostSession(
    session,
    presentation,
    { send: (message) => sent.push(message), present: (state) => states.push(state) },
    0,
  );
  for (const value of [
    null,
    [],
    {},
    { protocol: 'sugarcubes.panel', version: 4, session, type: 'ready' },
    { protocol: 'sugarcubes.panel', version: 3, session: 'different-session-12345', type: 'ready' },
    { protocol: 'sugarcubes.panel', version: 3, session, type: 'ready', extra: true },
    { protocol: 'sugarcubes.panel', version: 3, session, type: 'configure', presentation },
    { protocol: 'sugarcubes.panel', version: 3, session, type: 'applied' },
    { protocol: 'sugarcubes.panel', version: 3, session, type: 'pong', connected: true },
  ])
    expect(host.receive(value, 0)).toBe(false);
  expect(sent).toEqual([]);
  host.tick(29999);
  expect(states).toEqual(['loading']);
  host.tick(30000);
  expect(states).toEqual(['loading', 'failed']);
});

test('schema validation rejects unsupported presentation and oversized session values', () => {
  expect(
    isPanelMessage({
      protocol: 'sugarcubes.panel',
      version: 3,
      session,
      type: 'configure',
      presentation,
    }),
  ).toBe(true);
  for (const invalid of [
    { ...presentation, visualStyle: '<script>' },
    { ...presentation, accentColor: 'red;display:none' },
    { ...presentation, accentColor: '#12345' },
    { ...presentation, accentColor: 123 },
    ...[-1, 65, 1.5, Number.NaN, Number.POSITIVE_INFINITY, '24', undefined].map(
      (resizeCornerSize) => ({ ...presentation, resizeCornerSize }),
    ),
    { ...presentation, pluginMode: 'true' },
    { ...presentation, photoProPalette: 'unknown', accentColor: null },
    { ...presentation, path: 'file:/secret' },
  ])
    expect(
      isPanelMessage({
        protocol: 'sugarcubes.panel',
        version: 3,
        session,
        type: 'configure',
        presentation: invalid,
      }),
    ).toBe(false);
  expect(
    isPanelMessage({
      protocol: 'sugarcubes.panel',
      version: 3,
      session: 'a'.repeat(100000),
      type: 'ready',
    }),
  ).toBe(false);
});

test('reports presentation failure and returns an explicit rejection to every host', () => {
  const sent: PanelMessage[] = [];
  const errors: unknown[] = [];
  const error = new Error('Presentation unavailable');
  const client = new PanelClientSession(session, {
    send: (message) => sent.push(message),
    apply: () => {
      throw error;
    },
    connected: () => true,
    report: (failure) => errors.push(failure),
  });
  client.receive({
    protocol: 'sugarcubes.panel',
    version: 3,
    session,
    type: 'configure',
    presentation,
  });
  expect(errors).toEqual([error]);
  expect(sent.at(-1)).toMatchObject({ type: 'rejected', reason: 'presentation-failed' });
});

test('updates an active presentation without replacing its session or resetting connection state', () => {
  const sent: PanelMessage[] = [];
  const states: PanelState[] = [];
  const host = new PanelHostSession(
    session,
    presentation,
    {
      send: (message) => sent.push(message),
      present: (state) => states.push(state),
    },
    0,
  );
  const updated: PanelPresentation = {
    ...presentation,
    photoProPalette: 'light-gray',
    accentColor: null,
    resizeCornerSize: 0,
  };
  host.updatePresentation(updated);
  expect(sent).toEqual([]);
  host.receive({ protocol: 'sugarcubes.panel', version: 3, session, type: 'ready' }, 1);
  expect(sent.at(-1)).toMatchObject({ type: 'configure', presentation: updated });
  host.receive({ protocol: 'sugarcubes.panel', version: 3, session, type: 'applied' }, 2);
  host.receive(
    { protocol: 'sugarcubes.panel', version: 3, session, type: 'pong', connected: true },
    3,
  );
  host.updatePresentation(presentation);
  expect(sent.at(-1)).toMatchObject({ type: 'configure', session, presentation });
  expect(states).toEqual(['loading', 'connected']);
  const count = sent.length;
  host.updatePresentation({ ...presentation });
  expect(sent).toHaveLength(count);
  const resized = { ...presentation, resizeCornerSize: 24 };
  host.updatePresentation(resized);
  expect(sent.at(-1)).toMatchObject({ type: 'configure', presentation: resized });
  const resizedCount = sent.length;
  host.updatePresentation({ ...resized });
  expect(sent).toHaveLength(resizedCount);
  host.fail();
  host.updatePresentation(updated);
  expect(sent).toHaveLength(resizedCount);
});

test('generated language bindings and validators match the authoritative wire schema', async () => {
  const bindings = await panelBindings(
    readFileSync('integrations/contracts/panel.schema.json', 'utf8'),
  );
  for (const [file, source] of Object.entries(bindings))
    expect(readFileSync(file, 'utf8').replaceAll('\r\n', '\n')).toBe(source);
});

test('accepts GIMP as an additive panel presentation style', () => {
  expect(
    isPanelMessage({
      protocol: 'sugarcubes.panel',
      version: 3,
      session,
      type: 'configure',
      presentation: { ...presentation, visualStyle: 'gimp' },
    }),
  ).toBe(true);
});
