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
/** Safeguard the projected advanced footer's native palette and theme boundary. */
import { afterEach, expect, test } from '@jest/globals';
import { ensureNodeCardAppearanceStyles } from '../../../frontend/comfyui/ui/surface/appearance/NodeCardAppearanceStyles.js';

afterEach(() => {
  document.body.replaceChildren();
  document.getElementById('sugarcubes-node-card-appearance')?.remove();
});

/** Mount a native footer inside the shared projected card boundary. */
function footer(projected: boolean, fluent: boolean): HTMLButtonElement {
  const card = document.createElement('div');
  if (projected) card.className = 'sugarcubes-cube-face__node-card';
  const root = document.createElement('div');
  root.className = 'lg-node';
  if (fluent) root.dataset.scFluentNode = '';
  const button = document.createElement('button');
  button.dataset.testid = 'advanced-inputs-button';
  button.className = 'bg-node-component-header-surface hover:bg-secondary-background-hover';
  root.append(button);
  card.append(root);
  document.body.append(card);
  ensureNodeCardAppearanceStyles(document);
  return button;
}

test('shares the card surface with the footer through Comfy tokens and retains native hover', () => {
  const button = footer(true, false);
  expect(getComputedStyle(button).getPropertyValue('--node-component-header-surface')).toBe(
    'var(--sugarcubes-cube-card-header, var(--sugarcubes-masonry-card-surface, var(--comfy-input-bg, #222)))',
  );
  expect(button.classList.contains('hover:bg-secondary-background-hover')).toBe(true);
  expect(button.style.backgroundColor).toBe('');
});

test.each<[boolean, boolean]>([
  [false, false],
  [true, true],
])('leaves other native palettes alone for projected=%s and fluent=%s', (projected, fluent) => {
  const button = footer(projected, fluent);
  expect(getComputedStyle(button).getPropertyValue('--node-component-header-surface')).toBe('');
});
