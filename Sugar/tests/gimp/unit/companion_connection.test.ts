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
/** Prove the companion's local retry lifecycle independently of Tauri and native canvas behavior. */
import { jest } from '@jest/globals';
import {
  CompanionConnection,
  type ConnectionState,
} from '../../../integrations/gimp/src/application/CompanionConnection.js';

test('coalesces retry and preserves a usable disconnected shell', async () => {
  let finish: ((state: ConnectionState) => void) | undefined;
  const connect = jest.fn(
    (publish: (state: ConnectionState) => void) =>
      new Promise<void>((resolve) => {
        finish = (state) => {
          publish(state);
          resolve();
        };
      }),
  );
  const present = jest.fn();
  const report = jest.fn();
  const owner = new CompanionConnection({ connect, report }, present);
  const first = owner.retry();
  await owner.retry();
  expect(connect).toHaveBeenCalledTimes(1);
  expect(present).toHaveBeenCalledWith(expect.objectContaining({ phase: 'connecting' }));
  finish?.({
    phase: 'disconnected',
    address: 'http://127.0.0.1:8226/',
    message: 'Start ComfyUI, then retry.',
  });
  await first;
  expect(present).toHaveBeenLastCalledWith(expect.objectContaining({ phase: 'disconnected' }));
  expect(report).not.toHaveBeenCalled();
  owner.dispose();
});

test('does not revive presentation after the companion closes', async () => {
  let finish: ((state: ConnectionState) => void) | undefined;
  const present = jest.fn();
  const owner = new CompanionConnection(
    {
      connect: (publish) =>
        new Promise((resolve) => {
          finish = (state) => {
            publish(state);
            resolve();
          };
        }),
      report: jest.fn(),
    },
    present,
  );
  const pending = owner.retry();
  owner.dispose();
  finish?.({ phase: 'connected', address: 'http://127.0.0.1:8226/', message: 'Connected.' });
  await pending;
  await owner.retry();
  expect(present).toHaveBeenCalledTimes(1);
});

test('reports bridge failures and permits another attempt', async () => {
  const error = new Error('Native transport failed.');
  const report = jest.fn();
  const present = jest.fn();
  const connect = jest
    .fn<(publish: (state: ConnectionState) => void) => Promise<void>>()
    .mockRejectedValueOnce(error)
    .mockImplementation(async (publish) => {
      publish({ phase: 'disconnected', address: '', message: 'Start ComfyUI.' });
    });
  const owner = new CompanionConnection({ connect, report }, present);
  await owner.retry();
  expect(report).toHaveBeenCalledWith(error);
  expect(present).toHaveBeenLastCalledWith(expect.objectContaining({ phase: 'disconnected' }));
  await owner.retry();
  expect(connect).toHaveBeenCalledTimes(2);
  owner.dispose();
});

test('fences heartbeat callbacks from a replaced attempt while accepting the current lifetime', async () => {
  const callbacks: ((state: ConnectionState) => void)[] = [];
  const present = jest.fn();
  const owner = new CompanionConnection(
    {
      connect: async (publish) => {
        callbacks.push(publish);
      },
      report: jest.fn(),
    },
    present,
  );
  await owner.retry();
  await owner.retry();
  present.mockClear();
  const state = { phase: 'connected', address: '', message: 'Connected.' } as const;
  callbacks[0]?.(state);
  expect(present).not.toHaveBeenCalled();
  callbacks[1]?.(state);
  expect(present).toHaveBeenCalledWith(state);
  owner.dispose();
  present.mockClear();
  callbacks[1]?.(state);
  expect(present).not.toHaveBeenCalled();
});
