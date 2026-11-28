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
/** Exercise the installed extension in isolated browser contexts on one real host. */
import { defineConfig } from '@playwright/test';
import path from 'node:path';

const output = process.env.SUGAR_COMPAT_OUTPUT;
const baseURL = process.env.SUGAR_COMFY_URL;
if (!output || !baseURL) throw new Error('Launch through python -m tools.compatibility.host');

export default defineConfig({
  testDir: '../../tests/comfy_contract/integration',
  testMatch: '**/*.live.ts',
  outputDir: path.join(output, 'artifacts'),
  reporter: [['list'], ['json', { outputFile: path.join(output, 'checks.json') }]],
  snapshotPathTemplate: '{testDir}/../support/reference-images/{projectName}/{arg}{ext}',
  updateSnapshots: 'none',
  workers: 1,
  retries: 0,
  timeout: 150000,
  expect: { timeout: 12000 },
  use: {
    baseURL,
    actionTimeout: 12000,
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 1,
    locale: 'en-US',
    colorScheme: 'dark',
    reducedMotion: 'reduce',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [
    { name: 'chromium', use: { browserName: 'chromium' } },
    {
      name: 'firefox',
      use: {
        browserName: 'firefox',
        launchOptions: {
          env: { ...process.env, MOZ_DISABLE_SAFE_MODE_KEY: '1' },
          firefoxUserPrefs: { 'widget.windows.window_occlusion_tracking.enabled': false },
        },
      },
    },
  ],
});
