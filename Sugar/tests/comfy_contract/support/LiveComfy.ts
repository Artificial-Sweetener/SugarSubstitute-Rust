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
/** Capture the real page's extension diagnostics and require its completed startup. */
import { expect, test as base, type Locator, type Page, type TestInfo } from '@playwright/test';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { OPERATION_HANG_GUARD_MS } from '../../frontend_core/support/HangGuard.js';

/** Compare the mounted UI against its reviewed browser-specific image while retaining subsequent interaction evidence. */
export async function expectAppearance(surface: Locator, name: string): Promise<void> {
  await expect.soft(surface).toHaveScreenshot(name + '.png', {
    animations: 'disabled',
    caret: 'hide',
    threshold: 0.15,
    maxDiffPixelRatio: 0.003,
  });
}

/** Wait for Comfy to finish the installed extension's actual setup under the shared browser guard. */
export async function expectSugarLoaded(page: Page): Promise<void> {
  await expect(page.locator('html'), 'SugarCubes.UI must finish setup on the host').toHaveAttribute(
    'data-sugarcubes-setup',
    'loaded',
    { timeout: OPERATION_HANG_GUARD_MS },
  );
}

/** Measure the literal selectors and CSS variables consumed by the installed pack in each scene. */
async function measurePresentation(page: Page, info: TestInfo): Promise<void> {
  const inventory = JSON.parse(
    await readFile(new URL('./host-dependencies.json', import.meta.url), 'utf8'),
  ) as Record<string, string[]>;
  const expressions = new Set<string>();
  for (const entries of Object.values(inventory)) {
    for (const entry of entries) {
      if (entry.startsWith('host selector or token ')) {
        expressions.add(entry.slice('host selector or token '.length));
      } else {
        const literal = /^DOM lookup (?:'([^']+)'|"([^"]+)")$/.exec(entry);
        if (literal) expressions.add((literal[1] ?? literal[2])!);
      }
    }
  }
  const observed = await page.evaluate(
    (selectors) => {
      const root = getComputedStyle(document.documentElement);
      const result: Record<string, string[]> = {};
      for (const selector of selectors) {
        if (selector.startsWith('--')) {
          const value = root.getPropertyValue(selector).trim();
          if (value) result[selector] = [value];
        } else {
          try {
            const elements = [...document.querySelectorAll(selector)];
            const byId = document.getElementById(selector);
            if (byId) elements.push(byId);
            if (elements.length)
              result[selector] = [
                ...new Set(
                  elements.map(
                    (el) =>
                      el.localName +
                      (el.hasAttribute('role') ? '[role=' + el.getAttribute('role') + ']' : ''),
                  ),
                ),
              ].sort();
          } catch (error) {
            if (!(error instanceof DOMException) || error.name !== 'SyntaxError') throw error;
          }
        }
      }
      return result;
    },
    [...expressions].sort(),
  );
  const directory = path.join(process.env.SUGAR_COMPAT_OUTPUT!, 'presentation');
  await mkdir(directory, { recursive: true });
  await writeFile(
    path.join(directory, info.testId + '.json'),
    JSON.stringify({
      scenario: info.title,
      browser: info.project.name,
      observed,
    }),
  );
}

/** List measured sources separately from assertions that establish their intended behavior. */
async function record(info: TestInfo, runtime: string, observed: unknown): Promise<void> {
  const directory = path.join(process.env.SUGAR_COMPAT_OUTPUT!, 'source-coverage');
  await mkdir(directory, { recursive: true });
  await writeFile(
    path.join(directory, info.testId + '-' + runtime + '.json'),
    JSON.stringify({
      scenario: info.title,
      browser: info.project.name,
      runtime,
      observed,
    }),
  );
}

export const test = base.extend<{ loaded: Page; backendMeasurement: void }>({
  backendMeasurement: [
    async ({ request }, use, info) => {
      if (!process.env.SUGAR_COMPAT_MEASURE) return use();
      await expect(await request.post('/sugar-test-host/coverage/start')).toBeOK();
      try {
        await use();
      } finally {
        const result = await request.get('/sugar-test-host/coverage');
        await expect(result).toBeOK();
        await record(info, 'python', await result.json());
      }
    },
    { auto: true },
  ],
  loaded: async ({ page, request }, use, info) => {
    const measure = Boolean(process.env.SUGAR_COMPAT_MEASURE) && info.project.name === 'chromium';
    if (measure) await page.coverage.startJSCoverage({ resetOnNavigation: false });
    const settings = await request.post('/settings', {
      data: {
        'SugarCubes.Appearance.VisualStyle': 'comfy',
        'SugarCubes.Appearance.SubstituteFrame': false,
        'SugarCubes.OutputCanvas.Visible': false,
        'Comfy.VueNodes.Enabled': true,
      },
    });
    await expect(settings).toBeOK();
    const messages: string[] = [];
    page.on('pageerror', (error) => messages.push(error.stack ?? error.message));
    page.on('console', (event) => {
      if (['warning', 'error'].includes(event.type())) messages.push(event.text());
    });
    try {
      await page.goto('/', { waitUntil: 'domcontentloaded' });
      await expectSugarLoaded(page);
      const templates = page
        .getByRole('dialog')
        .filter({ has: page.getByRole('heading', { name: 'Templates', exact: true }) });
      await page.addLocatorHandler(templates, async () => {
        await templates.getByRole('button', { name: 'Close dialog' }).click();
      });
      await use(page);
      if (process.env.SUGAR_COMPAT_MEASURE && !page.isClosed())
        await measurePresentation(page, info);
    } finally {
      if (measure && !page.isClosed()) {
        const coverage = await page.coverage.stopJSCoverage();
        const observed: Record<string, string[]> = {};
        for (const script of coverage) {
          const file = script.url.split('/extensions/SugarCubes/')[1];
          if (!file || !file.endsWith('.js') || file.startsWith('third_party/')) continue;
          const names = script.functions
            .filter((fn) => fn.functionName && fn.ranges[0]?.count)
            .map((fn) => fn.functionName);
          if (names.length)
            observed['frontend/' + file.slice(0, -3) + '.ts'] = [...new Set(names)].sort();
        }
        await record(info, 'javascript', observed);
      }
      await info.attach('browser-diagnostics', {
        body: messages.join('\n'),
        contentType: 'text/plain',
      });
    }
  },
});

export { expect };
