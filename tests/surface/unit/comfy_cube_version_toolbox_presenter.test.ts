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
import { expect, jest, test } from '@jest/globals';
import { ComfyCubeVersionToolboxPresenter } from '../../../frontend/comfyui/ui/affordance/ComfyCubeVersionToolboxPresenter.js';
import { ComfyToolboxCheckMenuPresenter } from '../../../frontend/comfyui/ui/affordance/ComfyToolboxCheckMenuPresenter.js';
import { ComfyToolboxTooltipPresenter } from '../../../frontend/comfyui/ui/affordance/ComfyToolboxTooltipPresenter.js';

test('matches the color control and uses one exclusive shared check menu', () => {
  document.body.replaceChildren();
  const anchor = document.createElement('button');
  anchor.className = 'host-button';
  const colorButton = document.createElement('button');
  colorButton.setAttribute('data-testid', 'color-picker-button');
  const colorIcons = document.createElement('div');
  colorIcons.append(document.createElement('i'));
  const colorChevron = document.createElement('i');
  colorChevron.className = 'host-color-chevron';
  colorIcons.append(colorChevron);
  colorButton.append(colorIcons);
  document.body.append(colorButton, anchor);
  const select = jest.fn();
  const presenter = new ComfyCubeVersionToolboxPresenter({
    document,
    tooltip: new ComfyToolboxTooltipPresenter(document),
    menu: new ComfyToolboxCheckMenuPresenter({
      document,
      featureMenuAttribute: 'data-sugarcubes-version-menu',
      featureItemAttribute: 'data-sugarcubes-version-option',
    }),
  });
  const model = {
    currentVersion: '2.0.0',
    options: [
      {
        label: 'Latest (v2.0.0)',
        value: '2.0.0',
        revisionRef: 'WORKTREE',
        current: true,
        raw: null,
      },
      { label: 'v1.0.0', value: '1.0.0', revisionRef: 'abc123', current: false, raw: null },
    ],
    loading: false,
    busy: false,
    error: null,
    select,
  };

  const control = presenter.present(anchor, model);
  const button = control.querySelector<HTMLButtonElement>('[data-sugarcubes-version-button]');

  expect(control.previousElementSibling).toBe(anchor);
  expect(control.className).toBe('relative');
  expect(button?.className).toBe('host-button');
  expect(button?.getAttribute('aria-haspopup')).toBe('menu');
  expect(button?.getAttribute('aria-label')).toBe('Switch Cube version, current v2.0.0');
  expect(button?.firstElementChild?.className).toBe('flex items-center gap-1 px-0');
  expect(button?.querySelector('i.pi.pi-tags')).not.toBeNull();
  expect(button?.querySelector('i.host-color-chevron')).not.toBeNull();
  button?.dispatchEvent(new Event('pointerenter'));
  expect(document.querySelector('.p-tooltip-text')?.textContent).toBe('Switch Cube version');

  button?.click();
  const menu = document.querySelector<HTMLElement>('[data-sugarcubes-version-menu]');
  const rows = menu?.querySelectorAll<HTMLElement>('[data-sugarcubes-version-option]') ?? [];
  expect(menu?.getAttribute('role')).toBe('menu');
  expect([...rows].filter((row) => row.getAttribute('aria-checked') === 'true')).toHaveLength(1);
  expect([...rows].every((row) => row.getAttribute('role') === 'menuitemradio')).toBe(true);
  expect(rows[0]?.textContent).toContain('Latest (v2.0.0)');
  expect(document.getElementById('sugarcubes-toolbox-check-menu-styles')?.textContent).toContain(
    ':hover',
  );
  menu?.querySelector<HTMLButtonElement>('[data-sugarcubes-version-option="2.0.0"]')?.click();
  expect(select).not.toHaveBeenCalled();
  expect(document.querySelector('[data-sugarcubes-version-menu]')).toBeNull();

  button?.click();
  document.querySelector<HTMLButtonElement>('[data-sugarcubes-version-option="1.0.0"]')?.click();
  expect(select).toHaveBeenCalledWith('1.0.0');
});
