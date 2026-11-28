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
/** Exercise the production command registry, host layout and reversible workspace views. */
import { test, expect, expectAppearance } from '../support/LiveComfy.js';
import { command, importIntoBlank, loadWorkflow, setting } from '../support/NativeWorkflow.js';

test('masonry finder reads native prompt values and restores its query after reopening', async ({
  loaded: page,
}) => {
  await importIntoBlank(page, 'use "Artificial-Sweetener/Base-Cubes/PromptStudio.cube" as Studio');
  await command(page, 'SugarCubes.ToggleMasonry');
  const masonry = page.locator('.sugarcubes-workflow-masonry');
  await expect(masonry.locator('.cm-content')).toContainText('sunlit garden');
  await page.keyboard.press('Control+f');
  const finder = page.getByLabel('Find in masonry');
  await expect(finder).toBeVisible();
  await page.keyboard.insertText('sunlit');
  await expect(finder.getByRole('status')).toHaveText('1 of 1 matches');
  await expectAppearance(finder, 'masonry-finder-match');
  await page.keyboard.press('Escape');
  await expect(finder).toBeHidden();
  await page.keyboard.press('Control+f');
  await expect(finder.getByRole('status')).toHaveText('1 of 1 matches');
});

test('Substitute settings mounts inside the native dialog and releases it after search', async ({
  loaded: page,
}) => {
  await page.evaluate(() => window.SugarCubes.setSubstituteFrame(true));
  await page
    .locator('header.sc-substitute-frame .sc-app-orb[aria-label="Application menu"]')
    .click();
  await page.getByRole('menuitem', { name: 'Settings', exact: true }).click();
  const dialog = page.getByTestId('settings-dialog');
  const workspace = dialog.locator('.sc-sw[data-mode="frame"]');
  const search = workspace.getByRole('searchbox', { name: 'Search settings' });
  await expect.soft(search).toBeFocused();
  await expect(workspace.getByRole('navigation', { name: 'Settings pages' })).toBeVisible();
  await search.fill('compatibility fixture with no matching preference');
  await expect(workspace.getByText('No settings found')).toBeVisible();
  await expectAppearance(workspace, 'settings-search');
  await workspace.getByRole('button', { name: 'Close settings' }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator('#graph-canvas')).toBeVisible();
});

test('output canvas stays flush with the graph panel through a resize and releases the viewport', async ({
  loaded: page,
}) => {
  const pane = page.locator('.sc-output-workspace-pane');
  const graph = page.locator('.graph-canvas-panel');
  await setting(page, 'SugarCubes.OutputCanvas.Visible', false);
  await loadWorkflow(page);
  await expect(page.locator('.lg-node[data-node-id="1"]')).toContainText('Compatibility swatch');
  await expect(pane).toBeAttached();
  await expect(pane).toHaveAttribute('hidden', '');
  await command(page, 'SugarCubes.ToggleOutputCanvas');
  await expect(pane).toBeVisible();
  for (const width of [1440, 1190]) {
    await page.setViewportSize({ width, height: 900 });
    await expect
      .poll(
        async () => {
          const panelBox = await graph.boundingBox();
          const paneBox = await pane.boundingBox();
          if (!panelBox || !paneBox) return null;
          return Math.round(paneBox.x - panelBox.x - panelBox.width);
        },
        { message: 'Output pane must start at the graph panel right edge in CSS pixels' },
      )
      .toBe(0);
    if (width === 1190) await expectAppearance(pane, 'output-pane');
  }
  await command(page, 'SugarCubes.ToggleOutputCanvas');
  await expect(pane).toBeHidden();
  await expect(graph).toBeVisible();
});

test('masonry opens under Nodes 2.0 and restores the graph view when closed', async ({
  loaded: page,
}) => {
  await page.evaluate(async () => {
    const { app } = await import('/scripts/app.js');
    const host = app as typeof app & {
      ui: { settings: { setSettingValue(key: string, value: boolean): Promise<void> } };
    };
    await host.ui.settings.setSettingValue('Comfy.VueNodes.Enabled', true);
  });
  const masonry = page.locator('.sugarcubes-workflow-masonry');
  await command(page, 'SugarCubes.ToggleMasonry');
  await expect(masonry).toBeVisible();
  await command(page, 'SugarCubes.ToggleMasonry');
  await expect(masonry).toBeHidden();
  await expect(page.locator('#graph-canvas')).toBeVisible();
});

test('Substitute console opens within the frame and releases its dock on close', async ({
  loaded: page,
}) => {
  await page.evaluate(() => window.SugarCubes.setSubstituteFrame(true));
  await command(page, 'SugarCubes.ToggleConsole');
  await expect(page.locator('.sc-console__output')).toBeVisible();
  await expectAppearance(page.locator('.sc-console__header'), 'console-header');
  await command(page, 'SugarCubes.ToggleConsole');
  await expect(page.locator('.sc-console')).toBeHidden();
});
