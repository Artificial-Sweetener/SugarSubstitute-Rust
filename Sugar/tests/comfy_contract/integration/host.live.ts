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
/** Compare deployed backend registration and frame interaction on a real ComfyUI release. */
import { test, expect } from '../support/LiveComfy.js';

test('backend registers SugarCubes with its running pack identity', async ({ request }) => {
  const response = await request.get('/sugarcubes/status');
  await expect(response).toBeOK();
  const status: unknown = await response.json();
  expect(status).toMatchObject({
    workflowNodePack: { cnrId: 'SugarCubes' },
    sugarCubesVersion: expect.any(String),
  });
});

test('installed frontend finishes setup and reads the Cube library', async ({ loaded: page }) => {
  const result = await page.evaluate(() => window.SugarCubes.listCubes());
  expect(result).toMatchObject({
    exists: true,
    cubes: expect.any(Array),
    count: expect.any(Number),
  });
});

test('Substitute frame retains a 44px toolbar row on the live graph', async ({
  loaded: page,
}, info) => {
  await page.evaluate(() => window.SugarCubes.setSubstituteFrame(true));
  const frame = page.locator('header.sc-substitute-frame');
  await expect(frame).toBeVisible();
  const landmarks = await page.evaluate(() =>
    Object.fromEntries(
      [
        '.actionbar-container',
        '[aria-label="Workflow actions"]',
        '[data-sc-substitute-toolbar-line]',
      ].map((selector) => [
        selector,
        [...document.querySelectorAll(selector)].map((element) => ({
          tag: element.tagName.toLowerCase(),
          role: element.getAttribute('role'),
          height: getComputedStyle(element).height,
          parent: element.parentElement?.className,
        })),
      ]),
    ),
  );
  await info.attach('measured-host-state', {
    body: JSON.stringify(landmarks, null, 2),
    contentType: 'application/json',
  });
  await expect(
    page.locator('[data-sc-substitute-toolbar-line]'),
    'native action bar and workflow selector share the Substitute row',
  ).toHaveCSS('height', '44px');
});

test('Substitute application menu opens and frame cleanup restores native controls', async ({
  loaded: page,
}) => {
  await page.evaluate(() => window.SugarCubes.setSubstituteFrame(true));
  const frame = page.locator('header.sc-substitute-frame');
  await frame.locator('.sc-app-orb[aria-label="Application menu"]').click();
  await expect(page.getByRole('menuitem', { name: 'Open workflow', exact: true })).toBeVisible();
  await page.keyboard.press('Escape');
  await page.evaluate(() => window.SugarCubes.setSubstituteFrame(false));
  await expect(frame).toHaveCount(0);
  await expect(page.locator('[data-sc-substitute-toolbar-line]')).toHaveCount(0);
});
