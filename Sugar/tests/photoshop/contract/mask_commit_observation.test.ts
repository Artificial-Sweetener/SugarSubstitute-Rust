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
/** Prove metadata-only commit observation excludes drawing frames, owned history and disposed lifetimes. */
import { jest } from '@jest/globals';
import { build } from 'esbuild';
import { runInNewContext } from 'node:vm';
import { BUILD_FIXTURE_SETUP_TIMEOUT_MS } from '../../frontend_core/support/BuildFixtureBudget.js';
interface Observer {
  start(): void;
  acceptCurrent(): void;
  stop(): void;
  dispose(): void;
}

let code = '';
beforeAll(async () => {
  const bundle = await build({
    entryPoints: ['integrations/photoshop/src/infrastructure/PhotoshopCommittedChanges.ts'],
    bundle: true,
    write: false,
    format: 'cjs',
    platform: 'neutral',
    target: 'es2022',
  });
  const file = bundle.outputFiles[0];
  if (!file) throw new Error('Missing observer bundle.');
  code = file.text;
}, BUILD_FIXTURE_SETUP_TIMEOUT_MS);

test('reads metadata at a bounded cadence and captures once only when committed identity changes', async () => {
  let tick: () => void = () => {},
    state: string | null = 'doc:history-1',
    busy = false;
  const changed = jest.fn(async () => {}),
    read = jest.fn(() => state),
    report = jest.fn(),
    clear = jest.fn();
  const module: { exports: Record<string, unknown> } = { exports: {} };
  const interval = jest.fn((callback: () => void, _ms: number) => {
    tick = callback;
    return 1;
  });
  runInNewContext(code, { module, setInterval: interval, clearInterval: clear });
  // The compiled host boundary has no shared constructor identity with this test process.
  const Constructor = module.exports.PhotoshopCommittedChanges as new (
    read: () => string | null,
    busy: () => boolean,
    changed: () => Promise<void>,
    report: (error: unknown) => void,
  ) => Observer;
  const observer = new Constructor(read, () => busy, changed, report);
  observer.start();
  expect(interval).toHaveBeenCalledWith(expect.any(Function), 500);
  for (let frame = 0; frame < 100; frame++) tick();
  expect(changed).not.toHaveBeenCalled();
  busy = true;
  state = 'doc:history-2';
  tick();
  expect(changed).not.toHaveBeenCalled();
  busy = false;
  tick();
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
  expect(changed).toHaveBeenCalledTimes(1);
  state = 'doc:owned-history';
  observer.acceptCurrent();
  tick();
  expect(changed).toHaveBeenCalledTimes(1);
  state = null;
  tick();
  state = 'doc:history-3';
  tick();
  await Promise.resolve();
  expect(changed).toHaveBeenCalledTimes(2);
  observer.stop();
  state = 'doc:paused-history';
  tick();
  expect(changed).toHaveBeenCalledTimes(2);
  observer.start();
  tick();
  expect(changed).toHaveBeenCalledTimes(2);
  observer.dispose();
  state = 'doc:history-4';
  tick();
  expect(clear).toHaveBeenCalledWith(1);
  expect(changed).toHaveBeenCalledTimes(2);
  expect(report).not.toHaveBeenCalled();
});
