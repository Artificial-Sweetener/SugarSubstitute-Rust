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
/** Replay host-facing registration and stylesheet matches on a deterministic native scene. */
import { expect, test } from '../support/LiveComfy.js';
import { loadWorkflow, setting } from '../support/NativeWorkflow.js';
import type { Page } from '@playwright/test';

/** Record the exact owned stylesheet and selector whenever it binds to the Comfy scene. */
async function matchedRules(page: Page): Promise<Record<string, string[]>> {
  return page.evaluate(() => {
    const sheets: Record<string, string[]> = {};
    for (const sheet of document.styleSheets) {
      const owner = sheet.ownerNode;
      if (!(owner instanceof HTMLElement) || !/^(?:sugarcubes-|sc-)/.test(owner.id)) continue;
      const matched = new Set<string>();
      const visit = (rules: CSSRuleList): void => {
        for (const rule of rules) {
          if (rule instanceof CSSMediaRule && !matchMedia(rule.conditionText).matches) continue;
          if (rule instanceof CSSGroupingRule) visit(rule.cssRules);
          if (!(rule instanceof CSSStyleRule)) continue;
          const selector = rule.selectorText;
          if (/:hover|:active|:focus|:has\(:focus/.test(selector)) continue;
          if (document.querySelector(selector)) matched.add(selector);
        }
      };
      visit(sheet.cssRules);
      if (matched.size) sheets[owner.id] = [...matched].sort();
    }
    return Object.fromEntries(Object.entries(sheets).sort(([a], [b]) => a.localeCompare(b, 'en')));
  });
}

test('Comfy registers the full SugarCubes command, menu, lifecycle and settings surface', async ({
  loaded: page,
}) => {
  const facts = await page.evaluate(async () => {
    const { app } = await import('/scripts/app.js');
    const host = app as typeof app & {
      extensions: { name: string; menuCommands?: { path: string[]; commands: string[] }[] }[];
      extensionManager: {
        command: { commands: { id: string; label: string }[] };
        setting: { settings: Record<string, { type?: unknown; defaultValue?: unknown }> };
      };
    };
    return {
      extensions: host.extensions
        .filter((ext) => ext.name.startsWith('SugarCubes.'))
        .map((ext) => ({
          name: ext.name,
          hooks: Object.keys(ext)
            .filter((key) => typeof Reflect.get(ext, key) === 'function')
            .sort(),
          menus: ext.menuCommands ?? [],
        }))
        .sort((a, b) => a.name.localeCompare(b.name, 'en')),
      commands: host.extensionManager.command.commands
        .filter((item) => item.id.startsWith('SugarCubes.'))
        .map(({ id, label }) => ({ id, label }))
        .sort((a, b) => a.id.localeCompare(b.id, 'en')),
      settings: Object.fromEntries(
        Object.entries(host.extensionManager.setting.settings)
          .filter(([id]) => id.startsWith('SugarCubes.'))
          .sort(([a], [b]) => a.localeCompare(b, 'en'))
          .map(([id, setting]) => [
            id,
            {
              type: typeof setting.type === 'function' ? 'custom' : setting.type,
              default: setting.defaultValue,
            },
          ]),
      ),
    };
  });
  expect(facts.commands.map((c) => c.id)).toContain('SugarCubes.SaveSugarScript');
  expect(facts.settings).toHaveProperty(['SugarCubes.Appearance.VisualStyle']);
  expect(JSON.stringify(facts, null, 2)).toMatchSnapshot('extension-registration.json');
});

test('owned styles still match the native graph, Substitute frame and application menu', async ({
  loaded: page,
}) => {
  await setting(page, 'Comfy.VueNodes.Enabled', true);
  await setting(page, 'SugarCubes.Appearance.VisualStyle', 'fluent');
  await loadWorkflow(page);
  await expect(page.locator('.lg-node[data-sc-style-node]')).toBeVisible();
  await page.evaluate(() => window.SugarCubes.setSubstituteFrame(true));
  await page
    .locator('header.sc-substitute-frame .sc-app-orb[aria-label="Application menu"]')
    .click();
  await expect(page.getByRole('menuitem', { name: 'Open workflow', exact: true })).toBeVisible();
  const menu = page.getByRole('menu', { name: 'Application menu', exact: true });
  await expect.soft(menu).toHaveScreenshot('application-menu.png', {
    animations: 'disabled',
    maxDiffPixelRatio: 0.003,
    threshold: 0.15,
  });
  const bindings = await matchedRules(page);
  expect(bindings).toHaveProperty(['sugarcubes-accent-theme']);
  expect(bindings).toHaveProperty(['sugarcubes-substitute-frame']);
  expect(JSON.stringify(bindings, null, 2)).toMatchSnapshot('native-style-selectors.json');
});
