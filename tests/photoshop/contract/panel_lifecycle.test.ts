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
import type { PanelState } from '../../../integrations/shared/PanelSession.js';
import { panelFeedback } from '../../../integrations/photoshop/src/presentation/PanelFeedback.js';

test('preserves the document on hide/show and rotates sessions only on explicit retry', () => {
  const opened: string[] = [];
  const states: PanelState[] = [];
  const sent: PanelMessage[] = [];
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
  const controller = new PanelController(port, {
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
  expect(panelFeedback('connected').visible).toBe(false);
  expect(panelFeedback('loading')).toMatchObject({ visible: true, retry: false });
  expect(panelFeedback('disconnected')).toMatchObject({ visible: true, retry: true });
  expect(panelFeedback('failed')).toMatchObject({ visible: true, retry: true });
});
