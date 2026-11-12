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
/** Keep browser proofs independent of desktop modifier keys, occlusion throttling and wall-clock waits. */
import { expect, jest, test } from '@jest/globals';
import type { BrowserContext, LaunchOptions, Page } from '@playwright/test';
import { launchHeadlessBrowser, type PageFactories } from '../support/BrowserFixtureLifetime.js';
import { OPERATION_HANG_GUARD_MS } from '../support/HangGuard.js';

/** A browser whose contexts and pages record the default timeout each is given. */
function observedBrowser(): { browser: PageFactories; defaults: number[] } {
  const defaults: number[] = [];
  const guarded = { setDefaultTimeout: (timeout: number) => void defaults.push(timeout) };
  return {
    defaults,
    browser: {
      newContext: () => Promise.resolve(guarded as unknown as BrowserContext),
      newPage: () => Promise.resolve(guarded as unknown as Page),
    },
  };
}

test.each(['chromium', 'firefox'])(
  'launches %s headlessly without inheriting desktop input state',
  async (name) => {
    const { browser } = observedBrowser();
    const launch = jest
      .fn<(options: LaunchOptions) => Promise<PageFactories>>()
      .mockResolvedValue(browser);
    const options = { slowMo: 1 };
    expect(await launchHeadlessBrowser({ name: () => name, launch }, options)).toBe(browser);
    expect(launch).toHaveBeenCalledTimes(1);
    const supplied = launch.mock.calls[0]?.[0];
    expect(supplied).toMatchObject({
      headless: true,
      slowMo: 1,
      env: { MOZ_DISABLE_SAFE_MODE_KEY: '1' },
    });
    if (name === 'firefox')
      expect(supplied?.firefoxUserPrefs).toEqual({
        'widget.windows.window_occlusion_tracking.enabled': false,
      });
    else expect(supplied?.firefoxUserPrefs).toBeUndefined();
    expect(options).toEqual({ slowMo: 1 });
  },
);

test('launches and opens every context and page under the operation hang guard only', async () => {
  const { browser, defaults } = observedBrowser();
  const launch = jest
    .fn<(options: LaunchOptions) => Promise<PageFactories>>()
    .mockResolvedValue(browser);
  const launched = await launchHeadlessBrowser({ name: () => 'chromium', launch });
  expect(launch.mock.calls[0]?.[0].timeout).toBe(OPERATION_HANG_GUARD_MS);
  await launched.newContext();
  await launched.newPage();
  expect(defaults).toEqual([OPERATION_HANG_GUARD_MS, OPERATION_HANG_GUARD_MS]);
});

test('preserves the native launch failure instead of masking it with fixture setup', async () => {
  const failure = new Error('Private browser startup failed');
  const launch = jest
    .fn<(options: LaunchOptions) => Promise<PageFactories>>()
    .mockRejectedValue(failure);
  await expect(launchHeadlessBrowser({ name: () => 'firefox', launch })).rejects.toBe(failure);
});
