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
/** Create and retire headless browser fixtures independently of desktop input and visibility. */
import type { Server } from 'node:http';
import type { Browser, LaunchOptions } from '@playwright/test';
import { OPERATION_HANG_GUARD_MS } from './HangGuard.js';

/** The page factories through which suites open every context and page they observe. */
export type PageFactories = Pick<Browser, 'newContext' | 'newPage'>;

interface HeadlessBrowserEngine<Result extends PageFactories> {
  name(): string;
  launch(options: LaunchOptions): Promise<Result>;
}

/**
 * Keep display mode, desktop-key handling and hidden-window rendering under one fixture owner.
 *
 * The launch and every context and page opened from the browser wait under the browser hang
 * guard instead of Playwright's 30 s defaults, so they await the observed outcome.
 */
export async function launchHeadlessBrowser<Result extends PageFactories>(
  engine: HeadlessBrowserEngine<Result>,
  options: Omit<LaunchOptions, 'headless' | 'env' | 'firefoxUserPrefs' | 'timeout'> = {},
): Promise<Result> {
  const browser = await engine.launch({
    ...options,
    headless: true,
    timeout: OPERATION_HANG_GUARD_MS,
    env: { ...process.env, MOZ_DISABLE_SAFE_MODE_KEY: '1' },
    ...(engine.name() === 'firefox'
      ? { firefoxUserPrefs: { 'widget.windows.window_occlusion_tracking.enabled': false } }
      : {}),
  });
  return guardPageWaits(browser);
}

/** Give every context and page opened from `browser` the browser hang guard as its default. */
export function guardPageWaits<Result extends PageFactories>(browser: Result): Result {
  const newContext = browser.newContext.bind(browser);
  const newPage = browser.newPage.bind(browser);
  return Object.assign(browser, {
    newContext: async (...args: Parameters<PageFactories['newContext']>) => {
      const context = await newContext(...args);
      context.setDefaultTimeout(OPERATION_HANG_GUARD_MS);
      return context;
    },
    newPage: async (...args: Parameters<PageFactories['newPage']>) => {
      const page = await newPage(...args);
      page.setDefaultTimeout(OPERATION_HANG_GUARD_MS);
      return page;
    },
  });
}

/** Stop accepting fixture requests and release active or idle sockets after observations finish. */
function closeFixtureServer(server: Server | undefined): Promise<void> {
  if (!server?.listening) return Promise.resolve();
  return new Promise<void>((resolve, reject) => {
    server.close((error) => (error ? reject(error) : resolve()));
    server.closeAllConnections();
  });
}

/** Attempt every owned release and preserve all teardown failures without leaking the other resource. */
export async function closeBrowserFixture(
  browser: Pick<Browser, 'close'> | undefined,
  server: Server | undefined,
): Promise<void> {
  const outcomes = await Promise.allSettled([closeFixtureServer(server), browser?.close()]);
  const causes = outcomes.flatMap((outcome): unknown[] =>
    outcome.status === 'rejected' ? [outcome.reason] : [],
  );
  if (causes.length) throw new AggregateError(causes, 'Browser fixture cleanup failed');
}
