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
/** Prove reversible public host observation preserves extension call and failure contracts. */

import { jest } from '@jest/globals';
import { ComfyAsyncMethodObserver } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyAsyncMethodObserver.js';

/** Construct an observer whose metadata callbacks can be inspected independently of host behavior. */
function observe(host: object) {
  const enter = jest.fn<(args: readonly unknown[]) => string>(() => 'captured');
  const succeeded = jest.fn<(context: string, result: unknown) => void>();
  const failed = jest.fn<(context: string, error: unknown) => void>();
  const cancelled = jest.fn<(context: string) => void>();
  const logger = { error: jest.fn() };
  const observer = new ComfyAsyncMethodObserver({
    host,
    method: 'graphToPrompt',
    enter,
    succeeded,
    failed,
    cancelled,
    logger,
  });
  return { observer, enter, succeeded, failed, cancelled, logger };
}

test('preserves receiver, arguments, result identity, and stable installation', async () => {
  const result = { output: {} };
  const original = jest.fn<(this: unknown, ...args: unknown[]) => Promise<unknown>>(
    async function () {
      expect(this).toBe(host);
      return result;
    },
  );
  const host = { graphToPrompt: original };
  const hooks = observe(host);
  hooks.observer.install();
  const installed = host.graphToPrompt;
  hooks.observer.install();
  expect(host.graphToPrompt).toBe(installed);
  const argument = { workflow: 7 };
  expect(await host.graphToPrompt(argument, false)).toBe(result);
  expect(original).toHaveBeenCalledWith(argument, false);
  expect(hooks.enter).toHaveBeenCalledWith([argument, false]);
  expect(hooks.succeeded).toHaveBeenCalledWith('captured', result);
  hooks.observer.dispose();
  expect(host.graphToPrompt).toBe(original);
});

test('preserves rejection identity even if failure observation throws', async () => {
  const failure = new Error('Host serialization failed');
  const host = {
    graphToPrompt: async () => {
      throw failure;
    },
  };
  const hooks = observe(host);
  hooks.failed.mockImplementation(() => {
    throw new Error('Metadata failed');
  });
  hooks.observer.install();
  await expect(host.graphToPrompt()).rejects.toBe(failure);
  expect(hooks.failed).toHaveBeenCalledWith('captured', failure);
  expect(hooks.logger.error).toHaveBeenCalledWith(
    expect.any(String),
    expect.objectContaining({ stage: 'failed' }),
  );
  hooks.observer.dispose();
});

test('contains entry and completion metadata failures without changing host success', async () => {
  const result = { output: {} };
  const host = { graphToPrompt: async () => result };
  const hooks = observe(host);
  hooks.observer.install();
  hooks.enter.mockImplementationOnce(() => {
    throw new Error('Capture failed');
  });
  expect(await host.graphToPrompt()).toBe(result);
  expect(hooks.succeeded).not.toHaveBeenCalled();
  hooks.succeeded.mockImplementation(() => {
    throw new Error('Completion failed');
  });
  expect(await host.graphToPrompt()).toBe(result);
  expect(hooks.logger.error).toHaveBeenCalledTimes(2);
  hooks.observer.dispose();
});

test('allows a later extension wrapper to delegate after disposal and reinstall without recursion', async () => {
  const original = jest.fn<() => Promise<string>>(async () => 'host result');
  const host = { graphToPrompt: original as () => Promise<unknown> };
  const hooks = observe(host);
  hooks.observer.install();
  const retained = host.graphToPrompt;
  const later = async () => await retained.call(host);
  host.graphToPrompt = later;
  hooks.observer.dispose();
  expect(host.graphToPrompt).toBe(later);
  expect(await host.graphToPrompt()).toBe('host result');
  expect(hooks.enter).not.toHaveBeenCalled();
  hooks.observer.install();
  expect(await host.graphToPrompt()).toBe('host result');
  expect(hooks.enter).toHaveBeenCalledTimes(1);
  expect(original).toHaveBeenCalledTimes(2);
  hooks.observer.dispose();
  expect(host.graphToPrompt).toBe(later);
});

test('cancels pending metadata once and ignores its completion after a fresh installation', async () => {
  let complete: ((result: string) => void) | undefined;
  const host = {
    graphToPrompt: () =>
      new Promise<string>((resolve) => {
        complete = resolve;
      }),
  };
  const hooks = observe(host);
  hooks.observer.install();
  const pending = host.graphToPrompt();
  hooks.observer.dispose();
  hooks.observer.dispose();
  hooks.observer.install();
  if (!complete) throw new Error('Host completion is missing');
  complete('finished');
  expect(await pending).toBe('finished');
  expect(hooks.cancelled).toHaveBeenCalledTimes(1);
  expect(hooks.succeeded).not.toHaveBeenCalled();
  hooks.observer.dispose();
});

test('rejects an unavailable public method before mutating the host', () => {
  const host = { graphToPrompt: 'unavailable' };
  const hooks = observe(host);
  expect(() => hooks.observer.install()).toThrow('graphToPrompt is unavailable');
  expect(host.graphToPrompt).toBe('unavailable');
});
