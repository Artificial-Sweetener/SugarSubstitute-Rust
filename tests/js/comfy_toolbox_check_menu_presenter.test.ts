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
import { ComfyToolboxCheckMenuPresenter } from '../../frontend/comfyui/ui/affordance/ComfyToolboxCheckMenuPresenter.js';

test('rejects a single-selection menu with more than one checked row', () => {
  document.body.replaceChildren();
  const anchor = document.createElement('button');
  document.body.append(anchor);
  const presenter = new ComfyToolboxCheckMenuPresenter({
    document,
    featureMenuAttribute: 'data-test-menu',
    featureItemAttribute: 'data-test-row',
  });

  expect(() =>
    presenter.toggle(anchor, {
      ariaLabel: 'Versions',
      selectionMode: 'single',
      closeOnSelect: true,
      items: () => [
        { id: '1', label: 'v1', checked: true },
        { id: '2', label: 'v2', checked: true },
      ],
      select: jest.fn(),
    }),
  ).toThrow('exactly one checked item');
  expect(document.querySelector('[data-test-menu]')).toBeNull();
});
