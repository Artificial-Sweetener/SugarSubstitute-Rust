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
/** Edit a genuine Cube prompt through the native widget and SugarPrompt's browser editor. */
import { expect, expectAppearance, test } from '../support/LiveComfy.js';
import { command, importIntoBlank, setting } from '../support/NativeWorkflow.js';

test('projected prompt edits survive canvas shortcuts and remount with the original Unicode text', async ({
  loaded: page,
}) => {
  await setting(page, 'Comfy.VueNodes.Enabled', true);
  await importIntoBlank(page, 'use "Artificial-Sweetener/Base-Cubes/PromptStudio.cube" as Scene');
  await command(page, 'SugarCubes.ToggleMasonry');
  const surface = page.locator('.sugarcubes-workflow-masonry .cm-content[contenteditable=true]');
  await expect(surface).toHaveText('sunlit garden');
  await expectAppearance(
    page.locator('.sugarcubes-workflow-masonry .cm-editor'),
    'projected-prompt',
  );
  await surface
    .locator('.cm-line')
    .first()
    .click({ position: { x: 8, y: 6 } });
  await expect(surface).toBeFocused();
  await surface.press('ControlOrMeta+a');
  await expect(surface).toBeFocused();
  await surface.press('k');
  await expect(surface).toHaveText('k');
  await surface.press('ControlOrMeta+a');
  await surface.evaluate((editor) => {
    const paste = new ClipboardEvent('paste', {
      bubbles: true,
      cancelable: true,
      clipboardData: new DataTransfer(),
    });
    // Firefox supplies its own DataTransfer when constructing a paste event.
    paste.clipboardData!.setData('text/plain', '夜の庭');
    editor.dispatchEvent(paste);
  });
  await expect(surface).toHaveText('夜の庭');
  await command(page, 'SugarCubes.ToggleMasonry');
  await expect(surface).toBeHidden();
  await command(page, 'SugarCubes.ToggleMasonry');
  await expect(surface).toHaveText('夜の庭');
});
