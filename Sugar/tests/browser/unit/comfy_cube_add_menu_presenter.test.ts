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
/** Verify Add Cube menu search, model navigation, DOM safety, and selection. */

import { beforeEach, expect, jest, test } from '@jest/globals';
import type { CubeAddCandidate } from '../../../frontend/comfyui/ui/picker/CubeAddCandidateCatalog.js';
import { ComfyCubeAddMenuPresenter } from '../../../frontend/comfyui/ui/picker/ComfyCubeAddMenuPresenter.js';

beforeEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
});

test('shows same-model choices first and exposes other models through the bottom row', () => {
  const presenter = new ComfyCubeAddMenuPresenter(document);
  const same = candidate('sdxl', 'SDXL/<Upscale>', 'SDXL', true);
  const other = candidate('anima', 'Anima/Upscale', 'Anima', false);
  const select = jest.fn();
  presenter.toggle(
    { left: 100, top: 20, right: 132, bottom: 52 },
    {
      groups: { sameModel: [same], otherModels: [other] },
      search: (query) => [same, other].filter((item) => item.searchText.includes(query)),
      select,
    },
  );

  const menu = document.querySelector<HTMLElement>('[data-sugarcubes-add-cube-menu]');
  expect(menu?.querySelectorAll('[data-sugarcubes-add-cube-result]')).toHaveLength(1);
  expect(menu?.querySelector('script')).toBeNull();
  expect(menu?.querySelector('[data-sugarcubes-model-pill]')?.textContent).toBe('SDXL');
  expect(menu?.querySelector('.sugarcubes-model-title__name')?.textContent).toBe('<Upscale>');
  expect(menu?.querySelector('small')).toBeNull();
  expect(menu?.querySelector('[data-sugarcubes-add-cube-other-models]')).not.toBeNull();

  menu?.querySelector<HTMLButtonElement>('[data-sugarcubes-add-cube-other-models]')?.click();
  expect(menu?.querySelectorAll('[data-sugarcubes-add-cube-result]')).toHaveLength(1);
  expect(menu?.querySelector('[data-sugarcubes-model-pill]')?.textContent).toBe('Anima');
  expect(menu?.querySelector('.sugarcubes-model-title__name')?.textContent).toBe('Upscale');
  menu?.querySelector<HTMLButtonElement>('[data-sugarcubes-add-cube-result="anima"]')?.click();

  expect(select).toHaveBeenCalledWith('anima');
  expect(presenter.isOpen()).toBe(false);
});

test('searches all compatible models while retaining the scrollable result surface', () => {
  const presenter = new ComfyCubeAddMenuPresenter(document);
  const same = candidate('sdxl', 'SDXL/Upscale', 'SDXL', true);
  const other = candidate('anima', 'Anima/Upscale', 'Anima', false);
  const search = jest.fn((query: string) =>
    [same, other].filter((item) => item.searchText.includes(query.toLocaleLowerCase())),
  );
  presenter.toggle(
    { left: 100, top: 20, right: 132, bottom: 52 },
    {
      groups: { sameModel: [same], otherModels: [other] },
      search,
      select: jest.fn(),
    },
  );
  const input = document.querySelector<HTMLInputElement>(
    '[data-sugarcubes-add-cube-menu] input[type="search"]',
  );
  if (!input) throw new Error('Missing Add Cube search input.');
  input.value = 'anima';
  input.dispatchEvent(new Event('input', { bubbles: true }));

  expect(search).toHaveBeenCalledWith('anima');
  expect(document.querySelectorAll('[data-sugarcubes-add-cube-result]')).toHaveLength(1);
  expect(document.querySelector('[data-sugarcubes-model-pill]')?.textContent).toBe('Anima');
  expect(document.querySelector('.sugarcubes-model-title__name')?.textContent).toBe('Upscale');
  const styles = document.getElementById('sugarcubes-add-cube-menu-styles')?.textContent ?? '';
  expect(styles).toMatch(/overflow-y:\s*auto/);
  expect(styles).toMatch(/content-visibility:\s*auto/);
  expect(styles).toMatch(/contain-intrinsic-size:\s*auto 2\.75rem/);
});

test('keeps the menu open while its scroll surface moves', () => {
  const presenter = new ComfyCubeAddMenuPresenter(document);
  const same = candidate('sdxl', 'SDXL/Upscale', 'SDXL', true);
  presenter.toggle(
    { left: 100, top: 20, right: 132, bottom: 52 },
    {
      groups: { sameModel: [same], otherModels: [] },
      search: () => [same],
      select: jest.fn(),
    },
  );
  const results = document.querySelector<HTMLElement>('.sugarcubes-add-cube-menu__results');
  if (!results) throw new Error('Missing Add Cube result surface.');

  results.dispatchEvent(new Event('scroll'));

  expect(presenter.isOpen()).toBe(true);
});

/** Build one menu candidate with normalized search text. */
function candidate(
  type: string,
  displayName: string,
  targetModel: string,
  sameModel: boolean,
): CubeAddCandidate {
  return {
    type,
    cubeId: `local/${type}.cube`,
    displayName,
    description: `${displayName} description`,
    targetModel,
    sameModel,
    searchText: `${displayName} ${targetModel}`.toLocaleLowerCase(),
  };
}
