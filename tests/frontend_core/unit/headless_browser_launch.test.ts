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
/** Keep browser proofs independent of desktop modifier keys and Windows occlusion throttling. */
import { expect, jest, test } from '@jest/globals';
import type { LaunchOptions } from '@playwright/test';
import { launchHeadlessBrowser } from '../support/BrowserFixtureLifetime.js';

test.each(['chromium', 'firefox'])(
  'launches %s headlessly without inheriting desktop input state',
  async (name) => {
    const browser = Symbol('owned browser');
    const launch = jest
      .fn<(options: LaunchOptions) => Promise<typeof browser>>()
      .mockResolvedValue(browser);
    const options = { timeout: 20000 };
    expect(await launchHeadlessBrowser({ name: () => name, launch }, options)).toBe(browser);
    expect(launch).toHaveBeenCalledTimes(1);
    const supplied = launch.mock.calls[0]?.[0];
    expect(supplied).toMatchObject({
      headless: true,
      timeout: 20000,
      env: { MOZ_DISABLE_SAFE_MODE_KEY: '1' },
    });
    if (name === 'firefox')
      expect(supplied?.firefoxUserPrefs).toEqual({
        'widget.windows.window_occlusion_tracking.enabled': false,
      });
    else expect(supplied?.firefoxUserPrefs).toBeUndefined();
    expect(options).toEqual({ timeout: 20000 });
  },
);

test('preserves the native launch failure instead of masking it with fixture setup', async () => {
  const failure = new Error('Private browser startup failed');
  const launch = jest.fn<(options: LaunchOptions) => Promise<symbol>>().mockRejectedValue(failure);
  await expect(launchHeadlessBrowser({ name: () => 'firefox', launch })).rejects.toBe(failure);
});
