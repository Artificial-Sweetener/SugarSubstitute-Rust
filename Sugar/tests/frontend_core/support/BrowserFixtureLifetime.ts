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

/**
 * Bound every browser suite's fixture setup hook: browser launch, page bundle builds and page
 * creation. Native browser start-up stretches with machine-wide CPU load in a way the tests'
 * observations do not, so setup shares this budget while test bodies keep their own limits.
 */
export const BROWSER_FIXTURE_SETUP_TIMEOUT_MS = 60_000;

interface HeadlessBrowserEngine<Result> {
  name(): string;
  launch(options: LaunchOptions): Promise<Result>;
}

/** Keep display mode, desktop-key handling and hidden-window rendering under one fixture owner. */
export async function launchHeadlessBrowser<Result>(
  engine: HeadlessBrowserEngine<Result>,
  options: Omit<LaunchOptions, 'headless' | 'env' | 'firefoxUserPrefs'> = {},
): Promise<Result> {
  return engine.launch({
    ...options,
    headless: true,
    env: { ...process.env, MOZ_DISABLE_SAFE_MODE_KEY: '1' },
    ...(engine.name() === 'firefox'
      ? { firefoxUserPrefs: { 'widget.windows.window_occlusion_tracking.enabled': false } }
      : {}),
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
