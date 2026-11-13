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
/** Qualify an already-running, isolated native companion through its own Chromium debugging endpoint. */
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { build } from 'esbuild';
import { chromium } from '@playwright/test';
import { OPERATION_HANG_GUARD_MS } from '../../frontend_core/support/HangGuard.js';

/** Keep all screenshots and diagnostic outputs in the explicitly selected D-drive artifact directory. */
async function main(): Promise<void> {
  const artifact = process.argv[2];
  if (!artifact || !path.isAbsolute(artifact) || path.parse(artifact).root.toLowerCase() !== 'd:\\')
    throw new Error('Choose an absolute D-drive native qualification directory.');
  const fixture: unknown = JSON.parse(
    readFileSync(path.join(artifact, 'shared-fixture.json'), 'utf8'),
  );
  const output = await build({
    entryPoints: ['tests/gimp/support/native_round_trip.ts'],
    bundle: true,
    write: false,
    format: 'iife',
    globalName: 'SugarCubesNativeProof',
    platform: 'browser',
    target: 'es2022',
    tsconfig: 'tsconfig.gimp.json',
  });
  const script = output.outputFiles[0]?.text;
  if (!script) throw new Error('The native round-trip proof did not compile.');
  const browser = await chromium.connectOverCDP('http://127.0.0.1:9247', {
    timeout: OPERATION_HANG_GUARD_MS,
  });
  try {
    const page = browser
      .contexts()
      .flatMap((context) => context.pages())
      .find((page) => page.url().startsWith('http://tauri.localhost/'));
    if (!page) throw new Error('The isolated native companion page is unavailable.');
    page.setDefaultTimeout(OPERATION_HANG_GUARD_MS);
    await page.locator('#native-document').waitFor({ state: 'visible' });
    const documentLabel = await page.locator('#native-document').textContent();
    const result: unknown = await page.evaluate(
      `${script}\nSugarCubesNativeProof.proveNativeRoundTrip(globalThis, ${JSON.stringify(fixture)});`,
    );
    await page.screenshot({ path: path.join(artifact, 'native-shared-canvas-round-trip.png') });
    writeFileSync(
      path.join(artifact, 'shared-result.json'),
      JSON.stringify(
        {
          passed: true,
          result,
          documentLabel,
          guiLifecycleQualified: false,
          comfyExecutionQualified: false,
        },
        null,
        2,
      ),
    );
    console.log(JSON.stringify({ passed: true, result, documentLabel }));
  } finally {
    await browser.close();
  }
}

main().catch((error: unknown) => {
  console.error(error);
  process.exitCode = 1;
});
