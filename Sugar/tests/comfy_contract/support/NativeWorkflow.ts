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
/** Install a deterministic native workflow and change preferences through the real host. */
import type { Page } from '@playwright/test';

/** Place an authored Cube in a clean graph through SugarCubes' public import entry point. */
export async function importIntoBlank(page: Page, source: string): Promise<unknown> {
  return page.evaluate(async (source) => {
    const { app } = await import('/scripts/app.js');
    const host = app as typeof app & { loadGraphData(value: unknown): Promise<void> };
    await host.loadGraphData({
      version: 0.4,
      last_node_id: 0,
      last_link_id: 0,
      nodes: [],
      links: [],
      groups: [],
      config: {},
      extra: {},
    });
    return window.SugarCubes.importSugarScript(source);
  }, source);
}

/** Execute the same registered command the host menu and keyboard dispatch use. */
export async function command(page: Page, id: string): Promise<void> {
  await page.evaluate(async (id) => {
    const { app } = await import('/scripts/app.js');
    const host = app as typeof app & {
      extensionManager: { command: { execute(id: string): Promise<void> } };
    };
    await host.extensionManager.command.execute(id);
  }, id);
}

/** Apply a preference through ComfyUI's settings owner and wait for its write. */
export async function setting(page: Page, key: string, value: string | boolean): Promise<void> {
  await page.evaluate(
    async ({ key, value }) => {
      const { app } = await import('/scripts/app.js');
      const host = app as typeof app & {
        ui: { settings: { setSettingValue(key: string, value: string | boolean): Promise<void> } };
      };
      await host.ui.settings.setSettingValue(key, value);
    },
    { key, value },
  );
}

/** Load a saved workflow through ComfyUI's normal graph configuration lifecycle. */
export async function loadWorkflow(page: Page): Promise<void> {
  await page.evaluate(async () => {
    const { app } = await import('/scripts/app.js');
    const host = app as typeof app & { loadGraphData(data: unknown): Promise<void> };
    await host.loadGraphData({
      last_node_id: 1,
      last_link_id: 0,
      version: 0.4,
      nodes: [
        {
          id: 1,
          type: 'EmptyImage',
          title: 'Compatibility swatch',
          pos: [180, 140],
          size: [270, 140],
          flags: {},
          order: 0,
          mode: 0,
          inputs: [],
          outputs: [{ name: 'IMAGE', type: 'IMAGE', links: [] }],
          properties: {},
          widgets_values: [32, 16, 1, 16711680],
        },
      ],
      links: [],
      groups: [],
      config: {},
      extra: { ds: { scale: 1, offset: [0, 0] } },
    });
  });
}
