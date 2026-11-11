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
/** Safeguard browser context custody when Jest interrupts GIMP fixture acquisition or interaction. */
import { expect, jest, test } from '@jest/globals';
import type { Logger } from '@playwright/test';
import { GimpBrowserPageLifetime } from '../support/GimpBrowserPageLifetime.js';

/** Control an external browser operation without elapsed-time assumptions. */
function deferred<Value>(): {
  promise: Promise<Value>;
  resolve(value: Value): void;
  reject(error: Error): void;
} {
  let resolve!: (value: Value) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<Value>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}

/** Fake only browser acquisition and context cancellation, leaving fixture custody real. */
function browserFixture() {
  const page = Symbol('private page');
  const context = {
    newPage: jest.fn<() => Promise<symbol>>().mockResolvedValue(page),
    close: jest.fn<() => Promise<void>>().mockResolvedValue(undefined),
  };
  const browser = {
    newContext: jest
      .fn<(options: { logger: Logger }) => Promise<typeof context>>()
      .mockResolvedValue(context),
  };
  const dispose = jest.fn<(ownedPage: symbol) => Promise<void>>().mockResolvedValue(undefined);
  const lifetime = new GimpBrowserPageLifetime<symbol>();
  return { page, context, browser, dispose, lifetime };
}

test('disposes product state and closes the private context once after normal interaction', async () => {
  const fixture = browserFixture();
  expect(await fixture.lifetime.openPage(fixture.browser)).toBe(fixture.page);
  await fixture.lifetime.retire(fixture.dispose);
  await fixture.lifetime.retire(fixture.dispose);
  expect(fixture.dispose).toHaveBeenCalledWith(fixture.page);
  expect(fixture.dispose).toHaveBeenCalledTimes(1);
  expect(fixture.context.close).toHaveBeenCalledTimes(1);
  expect(fixture.lifetime.diagnostics().phase).toBe('retired');
  expect(fixture.dispose.mock.invocationCallOrder[0]).toBeLessThan(
    fixture.context.close.mock.invocationCallOrder[0]!,
  );
});

test('retains a context whose page setup failed and preserves the setup failure', async () => {
  const fixture = browserFixture();
  const failure = new Error('Native page setup failed');
  fixture.context.newPage.mockRejectedValue(failure);
  await expect(fixture.lifetime.openPage(fixture.browser)).rejects.toBe(failure);
  await fixture.lifetime.retire(fixture.dispose);
  expect(fixture.context.close).toHaveBeenCalledTimes(1);
  expect(fixture.dispose).not.toHaveBeenCalled();
});

test('closes the retained context to cancel page setup that outlives its test', async () => {
  const fixture = browserFixture();
  const acquisition = deferred<symbol>();
  const started = deferred<void>();
  const cancelled = new Error('Context closed during page setup');
  fixture.context.newPage.mockImplementation(() => {
    started.resolve(undefined);
    return acquisition.promise;
  });
  fixture.context.close.mockImplementation(() => {
    acquisition.reject(cancelled);
    return Promise.resolve();
  });
  const opening = fixture.lifetime.openPage(fixture.browser);
  const outcome = expect(opening).rejects.toBe(cancelled);
  await started.promise;
  await fixture.lifetime.retire(fixture.dispose);
  await outcome;
  expect(fixture.context.close).toHaveBeenCalledTimes(1);
  expect(fixture.dispose).not.toHaveBeenCalled();
});

test('retires a late context acquisition without allowing it to open an escaped page', async () => {
  const fixture = browserFixture();
  const acquisition = deferred<typeof fixture.context>();
  fixture.browser.newContext.mockReturnValue(acquisition.promise);
  const opening = fixture.lifetime.openPage(fixture.browser);
  const outcome = expect(opening).rejects.toThrow('retired during context acquisition');
  const retirement = fixture.lifetime.retire(fixture.dispose);
  expect(fixture.context.close).not.toHaveBeenCalled();
  acquisition.resolve(fixture.context);
  await retirement;
  await outcome;
  expect(fixture.context.close).toHaveBeenCalledTimes(1);
  expect(fixture.context.newPage).not.toHaveBeenCalled();
  expect(fixture.dispose).not.toHaveBeenCalled();
  await expect(fixture.lifetime.openPage(fixture.browser)).rejects.toThrow('already used');
});

test('observes late context acquisition failures during both setup and retirement', async () => {
  const fixture = browserFixture();
  const acquisition = deferred<typeof fixture.context>();
  const failure = new Error('Context acquisition failed after setup timeout');
  fixture.browser.newContext.mockReturnValue(acquisition.promise);
  const opening = expect(fixture.lifetime.openPage(fixture.browser)).rejects.toBe(failure);
  const retirement = expect(fixture.lifetime.retire(fixture.dispose)).rejects.toBe(failure);
  acquisition.reject(failure);
  await Promise.all([opening, retirement]);
  expect(fixture.context.close).not.toHaveBeenCalled();
  expect(fixture.lifetime.diagnostics().phase).toBe('retirement failed');
});

test('preserves disposal and context-close errors while attempting both releases', async () => {
  const fixture = browserFixture();
  const disposal = new Error('Product disposal failed');
  const closure = new Error('Context closure failed');
  await fixture.lifetime.openPage(fixture.browser);
  fixture.dispose.mockRejectedValue(disposal);
  fixture.context.close.mockRejectedValue(closure);
  await expect(fixture.lifetime.retire(fixture.dispose)).rejects.toMatchObject({
    message: 'GIMP page cleanup failed',
    errors: [disposal, closure],
  });
  expect(fixture.context.close).toHaveBeenCalledTimes(1);
  expect(fixture.lifetime.diagnostics().phase).toBe('retirement failed');
});

test('retains bounded pending and recent API names without logging browser payloads', async () => {
  const fixture = browserFixture();
  await fixture.lifetime.openPage(fixture.browser);
  const logger = fixture.browser.newContext.mock.calls[0]![0].logger;
  expect(logger.isEnabled('api', 'info')).toBe(true);
  expect(logger.isEnabled('protocol', 'info')).toBe(false);
  for (let index = 0; index < 24; index++) {
    logger.log(
      'api',
      'info',
      `=> page.operation${String(index)} started`,
      ['private argument'],
      {},
    );
  }
  logger.log('api', 'info', '<= page.operation23 failed', [], {});
  const diagnostics = fixture.lifetime.diagnostics();
  expect(diagnostics.phase).toBe('ready');
  expect(diagnostics.pending).toHaveLength(15);
  expect(diagnostics.pending).toContain('page.operation22');
  expect(diagnostics.pending).not.toContain('page.operation23');
  expect(diagnostics.recent).toHaveLength(16);
  expect(diagnostics.recent.at(-1)).toBe('page.operation23 failed');
  expect(JSON.stringify(diagnostics)).not.toContain('private argument');
  await fixture.lifetime.retire(fixture.dispose);
});
