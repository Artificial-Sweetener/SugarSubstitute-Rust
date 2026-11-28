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
/** Exercise menus that span the native graph canvas, contributed actions and projected fields. */
import { test, expect } from '../support/LiveComfy.js';
import { command, setting, importIntoBlank, loadWorkflow } from '../support/NativeWorkflow.js';

for (const vue of [false, true]) {
  test(`Nodes ${vue ? '2.0' : '1.0'} native graph context menu exposes Cube authoring and dismisses on a graph click`, async ({
    loaded: page,
  }) => {
    await setting(page, 'Comfy.VueNodes.Enabled', vue);
    await loadWorkflow(page);
    const canvas = page.locator('#graph-canvas');
    await canvas.click({ button: 'right', position: { x: 115, y: 340 } });
    const action = page.getByText('Create Empty SugarCube', { exact: true });
    await expect(action).toBeVisible();
    await canvas.click({ position: { x: 45, y: 320 } });
    await expect(action).toBeHidden();
  });
}

test('projected dimension menu swaps both native field values as one user action', async ({
  loaded: page,
}) => {
  await setting(page, 'Comfy.VueNodes.Enabled', true);
  await importIntoBlank(page, 'use "Artificial-Sweetener/Base-Cubes/Compatibility.cube" as Swatch');
  await command(page, 'SugarCubes.ToggleMasonry');
  const rows = page.locator('.sugarcubes-workflow-masonry').getByTestId('node-widget');
  const height = rows.filter({ has: page.getByText(/^height$/i) });
  const width = rows.filter({ has: page.getByText(/^width$/i) });
  await expect(height.locator('input')).toHaveValue('16');
  const originalWidth = await width.locator('input').inputValue();
  await height.click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Swap width & height', exact: true }).click();
  await expect(width.locator('input')).toHaveValue('16');
  await expect(height.locator('input')).toHaveValue(originalWidth);
});
