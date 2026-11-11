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
/** Prove asynchronous native relay admission preserves public session identity and bounded lifetime. */
import { jest } from '@jest/globals';
import { TauriPanelTransport } from '../../../integrations/gimp/src/infrastructure/TauriPanelTransport.js';

const session = 'fixture-session-0001';
const message = { protocol: 'sugarcubes.panel', version: 3, session, type: 'ready' } as const;

test('attaches before sending, forwards unknown inbound values and detaches consumers on disposal', async () => {
  let current: Channel | undefined;
  class Channel {
    onmessage: (value: unknown) => void = () => {};
    constructor() {
      current = this;
    }
  }
  let attach: (() => void) | undefined;
  const invoke = jest.fn((command: string) =>
    command === 'panel_open'
      ? new Promise<void>((resolve) => {
          attach = resolve;
        })
      : Promise.resolve(),
  );
  const report = jest.fn();
  const owner = new TauriPanelTransport(
    { __TAURI__: { core: { Channel, invoke } } },
    session,
    'host',
    report,
  );
  const receive = jest.fn();
  const stop = owner.subscribe(receive);
  owner.send(message);
  expect(invoke).toHaveBeenCalledTimes(1);
  attach?.();
  await owner.ready();
  await Promise.resolve();
  expect(invoke).toHaveBeenLastCalledWith('panel_send', { session, message });
  current?.onmessage({ untrusted: '<b>literal</b>' });
  expect(receive).toHaveBeenCalledWith({ untrusted: '<b>literal</b>' });
  stop();
  current?.onmessage('detached');
  expect(receive).toHaveBeenCalledTimes(1);
  owner.dispose();
  owner.send(message);
  expect(report).not.toHaveBeenCalled();
});

test('bounds pending sends during attachment and fences a closed renderer before native delivery', async () => {
  class Channel {
    onmessage: (value: unknown) => void = () => {};
  }
  let attach: (() => void) | undefined;
  const invoke = jest.fn((command: string) =>
    command === 'panel_attach'
      ? new Promise<void>((resolve) => {
          attach = resolve;
        })
      : Promise.resolve(),
  );
  const owner = new TauriPanelTransport(
    { __TAURI__: { core: { Channel, invoke } } },
    session,
    'guest',
    jest.fn(),
  );
  for (let index = 0; index < 16; index++) owner.send(message);
  expect(() => owner.send(message)).toThrow('busy');
  expect(() => owner.send({ ...message, session: 'stale-session-0001' })).toThrow('stale');
  owner.dispose();
  attach?.();
  await expect(owner.ready()).rejects.toThrow('closed');
  expect(invoke).toHaveBeenCalledTimes(1);
});
