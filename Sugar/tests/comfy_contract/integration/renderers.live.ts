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
/** Verify renderer transitions retain workflow data and restyle an actual upstream node. */
import { test, expect } from '../support/LiveComfy.js';
import { setting, loadWorkflow } from '../support/NativeWorkflow.js';

for (const style of ['fluent', 'spectrum', 'photopro', 'gimp'] as const) {
  test(`${style} native node survives Nodes 2.0 to 1.0 and back with authored values`, async ({
    loaded: page,
  }) => {
    await setting(page, 'Comfy.VueNodes.Enabled', true);
    await setting(page, 'SugarCubes.Appearance.VisualStyle', style);
    await loadWorkflow(page);
    const card = page.locator('.lg-node[data-node-id="1"]');
    await expect(page.locator('html')).toHaveAttribute('data-sc-visual-style', style);
    await expect(card).toBeVisible();
    await expect(card).toHaveAttribute('data-sc-style-node', '');
    await expect(card).toContainText('Compatibility swatch');
    await card.hover();
    await page.mouse.move(5, 895);
    await page.evaluate(async () => {
      await document.fonts.ready;
    });
    await expect.soft(card).toHaveScreenshot(style + '-native-card.png', {
      animations: 'disabled',
      maxDiffPixelRatio: 0.003,
      threshold: 0.15,
    });
    await setting(page, 'Comfy.VueNodes.Enabled', false);
    await expect(card).toHaveCount(0);
    const canvas = page.locator('#graph-canvas');
    await expect(canvas).toBeVisible();
    const clip = await page.evaluate(async () => {
      const { app } = await import('/scripts/app.js');
      const host = app as typeof app & {
        canvas: { canvas: HTMLCanvasElement; ds: { scale: number; offset: ArrayLike<number> } };
      };
      const bounds = host.canvas.canvas.getBoundingClientRect();
      const { scale, offset } = host.canvas.ds;
      return {
        x: Math.round(bounds.x + (180 + offset[0]!) * scale),
        y: Math.round(bounds.y + (140 + offset[1]! - 30) * scale),
        width: Math.round(270 * scale),
        height: Math.round(152 * scale),
      };
    });
    await expect.soft(page).toHaveScreenshot(style + '-legacy-card.png', {
      clip,
      animations: 'disabled',
      maxDiffPixelRatio: 0.003,
      threshold: 0.15,
    });
    await setting(page, 'Comfy.VueNodes.Enabled', true);
    await expect(card).toBeVisible();
    await expect(card).toHaveAttribute('data-sc-style-node', '');
    const values = await page.evaluate(async () => {
      const { app } = await import('/scripts/app.js');
      const host = app as typeof app & {
        graphToPrompt(): Promise<{ output: Record<string, { inputs: unknown }> }>;
      };
      return (await host.graphToPrompt()).output['1']?.inputs;
    });
    expect(values).toMatchObject({ width: 32, height: 16, batch_size: 1, color: 16711680 });
  });
}
