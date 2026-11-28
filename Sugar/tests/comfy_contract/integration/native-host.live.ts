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
/** Exercise native component acquisition and the desktop's web-facing presentation contract. */
import { test, expect, expectAppearance } from '../support/LiveComfy.js';
import type { ComfyNativeNodeAssets } from '../../../frontend/comfyui/ui/surface/ComfyNativeNodeAssets.js';

test('projected cards can acquire the real native component and its matching Vue runtime', async ({
  loaded: page,
}) => {
  const capabilities = await page.evaluate(async () => {
    const adapter = '/extensions/SugarCubes/comfyui/ui/surface/ComfyNativeNodeAssets.js';
    const {
      comfyNativeNodeAssets,
    }: { comfyNativeNodeAssets(document: Document): ComfyNativeNodeAssets } = await import(adapter);
    const assets = comfyNativeNodeAssets(document);
    const [component, runtime] = await Promise.all([assets.component(), assets.runtime()]);
    return {
      component: component.__name ?? component.name,
      render: typeof runtime.render,
      extract: typeof runtime.extractVueNodeData,
      layout: typeof runtime.requestSlotLayoutSync,
    };
  });
  expect(capabilities).toEqual({
    component: 'LGraphNode',
    render: 'function',
    extract: 'function',
    layout: 'function',
  });
});

test('desktop frame reserves caption geometry and follows light and dark material colors', async ({
  loaded: page,
}) => {
  for (const darkTheme of [true, false]) {
    await page.evaluate(
      (darkTheme) =>
        window.SugarCubes.configureDesktopFrame({
          schemaVersion: 1,
          captionWidth: 138,
          titlebarHeight: 34,
          darkTheme,
        }),
      darkTheme,
    );
    await expect
      .poll(() => page.evaluate(() => window.SugarCubes.readDesktopFrame()))
      .toMatchObject({
        darkTheme,
        mounted: true,
        captionWidth: 138,
        titlebarHeight: 34,
      });
    await expect(page.locator('[data-sc-desktop-frame-host]')).toHaveCSS(
      '--sc-native-caption-width',
      '138px',
    );
    await expect(page.locator('html')).toHaveCSS(
      '--sc-desktop-body-wash',
      darkTheme ? 'rgba(32, 32, 32, 0.5882)' : 'rgba(251, 251, 251, 0.7373)',
    );
  }
});

test('Cube picker opens the real catalog and cancels without editing the graph', async ({
  loaded: page,
}) => {
  const graph = () =>
    page.evaluate(async () => {
      const { app } = await import('/scripts/app.js');
      const host = app as typeof app & {
        graphToPrompt(): Promise<{
          output: unknown;
          workflow: {
            nodes: { id: unknown; type: unknown; widgets_values?: unknown; pos: unknown }[];
            links: unknown;
          };
        }>;
      };
      const { output, workflow } = await host.graphToPrompt();
      return {
        output,
        nodes: workflow.nodes.map(({ id, type, widgets_values, pos }) => ({
          id,
          type,
          widgets_values,
          pos,
        })),
        links: workflow.links,
      };
    });
  const previous = await graph();
  await page.evaluate(() => {
    void window.SugarCubes.openLibrary();
  });
  const dialog = page.locator('.sc-cube-stack-cart-dialog');
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('button', { name: 'Apply' })).toBeDisabled();
  await expect(dialog.getByRole('button', { name: 'Refresh' })).toBeVisible();
  const search = dialog.getByRole('searchbox', { name: 'Search cubes' });
  await search.click();
  await expect(search).toBeFocused();
  await expectAppearance(dialog, 'cube-stack-picker');
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog).toBeHidden();
  expect(await graph()).toEqual(previous);
});
