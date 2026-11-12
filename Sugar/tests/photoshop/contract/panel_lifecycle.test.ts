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
/** Verify native lifecycle sequencing without loading Photoshop or reproducing transport policy. */
import {
  PanelController,
  type NativePanelPort,
} from '../../../integrations/photoshop/src/application/PanelController.js';
import type { PanelMessage } from '../../../integrations/contracts/PanelProtocol.generated.js';
import type { HostDescriptionMessage } from '../../../integrations/contracts/HostDescription.js';
import { PHOTOSHOP_HOST_DESCRIPTION } from '../../../integrations/photoshop/src/application/PhotoshopHostDescription.js';
import type { PanelState } from '../../../integrations/shared/PanelSession.js';
import { panelFeedback } from '../../../integrations/photoshop/src/presentation/PanelFeedback.js';

test('preserves hide/show and rotates an uninitialized navigation on retry', () => {
  const opened: string[] = [];
  const states: PanelState[] = [];
  const sent: (PanelMessage | HostDescriptionMessage)[] = [];
  const errors: unknown[] = [];
  const timers = new Set<() => void>();
  let identity = 0;
  const port: NativePanelPort = {
    newSession: () => `session-identity-${++identity}`,
    now: () => 0,
    open: (session) => opened.push(session),
    send: (value) => sent.push(value),
    present: (state) => states.push(state),
    report: (error) => errors.push(error),
    schedule: (tick) => {
      timers.add(tick);
      return () => {
        timers.delete(tick);
      };
    },
  };
  const controller = new PanelController(port, PHOTOSHOP_HOST_DESCRIPTION, {
    pluginMode: true,
    visualStyle: 'photopro',
    photoProPalette: 'medium-gray',
    accentColor: null,
    resizeCornerSize: 0,
  });
  controller.start();
  controller.start();
  expect(opened).toEqual(['session-identity-1']);
  expect(timers.size).toBe(1);
  controller.hide();
  expect(timers.size).toBe(0);
  controller.show();
  controller.show();
  expect(timers.size).toBe(1);
  expect(opened).toHaveLength(1);
  controller.retry();
  expect(opened).toEqual(['session-identity-1', 'session-identity-2']);
  expect(timers.size).toBe(1);
  controller.receive({
    protocol: 'sugarcubes.panel',
    version: 3,
    session: 'session-identity-1',
    type: 'ready',
  });
  expect(sent).toEqual([]);
  controller.receive({
    protocol: 'sugarcubes.panel',
    version: 3,
    session: 'session-identity-2',
    type: 'ready',
  });
  expect(sent.at(-1)).toMatchObject({
    type: 'configure',
    presentation: { pluginMode: true, visualStyle: 'photopro' },
  });
  controller.fail(new Error('Load failed'));
  expect(timers.size).toBe(0);
  expect(states.at(-1)).toBe('failed');
  expect(errors).toHaveLength(1);
  controller.hide();
  controller.show();
  expect(timers.size).toBe(0);
  controller.dispose();
  expect(timers.size).toBe(0);
});

test('gives connection failures a visible retry action and keeps loading feedback nonblocking', () => {
  expect(panelFeedback('connected', 'http://127.0.0.1:8188').visible).toBe(false);
  expect(panelFeedback('loading', 'http://127.0.0.1:8188')).toMatchObject({
    visible: true,
    retry: false,
  });
  expect(panelFeedback('disconnected', 'http://127.0.0.1:8188')).toMatchObject({
    visible: true,
    retry: true,
  });
  expect(panelFeedback('failed', 'http://127.0.0.1:8188')).toMatchObject({
    visible: true,
    retry: true,
  });
});

test('recovery feedback identifies the configured server address', () => {
  const feedback = panelFeedback('failed', 'http://127.0.0.1:32984');
  expect(feedback.message).toContain('http://127.0.0.1:32984');
  expect(feedback.message).not.toContain('8188');
  expect(feedback).toMatchObject({ visible: true, retry: true });
});

/** Exercise navigation as the externally visible native boundary for every recovery reason. */
test.each([
  { name: 'responsive disconnected page', now: 10000, failed: false, navigations: 1 },
  { name: 'stalled page', now: 15000, failed: false, navigations: 2 },
  { name: 'failed page', now: 10000, failed: true, navigations: 2 },
])('retries a $name with $navigations navigation(s)', ({ now: retryTime, failed, navigations }) => {
  const opened: string[] = [];
  const sent: (PanelMessage | HostDescriptionMessage)[] = [];
  const states: PanelState[] = [];
  const timers = new Set<() => void>();
  let identity = 0;
  let now = 0;
  const controller = new PanelController(
    {
      newSession: () => 'recovery-session-' + String(++identity),
      now: () => now,
      open: (session) => opened.push(session),
      send: (message) => sent.push(message),
      present: (state) => states.push(state),
      report: () => undefined,
      schedule: (tick) => {
        timers.add(tick);
        return () => {
          timers.delete(tick);
        };
      },
    },
    PHOTOSHOP_HOST_DESCRIPTION,
    {
      pluginMode: true,
      visualStyle: 'photopro',
      photoProPalette: 'medium-gray',
      accentColor: null,
      resizeCornerSize: 0,
    },
  );
  controller.start();
  const session = opened[0];
  if (!session) throw new Error('The initial navigation was not opened.');
  controller.receive({ protocol: 'sugarcubes.panel', version: 3, session, type: 'ready' });
  controller.receive({ protocol: 'sugarcubes.panel', version: 3, session, type: 'applied' });
  controller.receive({
    protocol: 'sugarcubes.panel',
    version: 3,
    session,
    type: 'pong',
    connected: false,
  });
  expect(states.at(-1)).toBe('disconnected');
  if (failed) controller.fail(new Error('Native navigation failed.'));
  now = retryTime;
  controller.retry();
  expect(opened).toHaveLength(navigations);
  expect(timers.size).toBe(1);
  if (navigations === 1) {
    expect(sent.at(-1)).toMatchObject({ type: 'ping', session });
    controller.receive({
      protocol: 'sugarcubes.panel',
      version: 3,
      session,
      type: 'pong',
      connected: true,
    });
    expect(states.at(-1)).toBe('connected');
    controller.retry();
    expect(opened).toHaveLength(1);
    expect(timers.size).toBe(1);
  }
  controller.dispose();
  expect(timers.size).toBe(0);
});
