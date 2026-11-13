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
/** Verify the isolated PrimeVue lazy-tooltip compatibility boundary. */

import { jest } from '@jest/globals';
import { PrimeVueTooltipPresentationAdapter } from '../../../frontend/comfyui/ui/affordance/PrimeVueTooltipPresentationAdapter.js';

test('presents and restores lazy and already-mounted PrimeVue tooltip state', () => {
  document.body.replaceChildren();
  const button = document.createElement('button');
  button.setAttribute('aria-label', 'Publish Subgraph');
  Reflect.set(button, '$_ptooltipValue', 'Publish Subgraph');
  Reflect.set(button, '$_ptooltipId', 'publish-tooltip');
  const tooltip = document.createElement('div');
  tooltip.id = 'publish-tooltip';
  const text = document.createElement('span');
  text.className = 'p-tooltip-text';
  text.textContent = 'Publish Subgraph';
  tooltip.append(text);
  document.body.append(button, tooltip);
  const adapter = new PrimeVueTooltipPresentationAdapter({ document, logger: console });
  const original = adapter.capture(button);

  adapter.present(button, 'Save Cube');

  expect(button.getAttribute('aria-label')).toBe('Save Cube');
  expect(button.hasAttribute('title')).toBe(false);
  expect(Reflect.get(button, '$_ptooltipValue')).toBe('Save Cube');
  expect(text.textContent).toBe('Save Cube');

  adapter.restore(button, original);

  expect(button.getAttribute('aria-label')).toBe('Publish Subgraph');
  expect(button.hasAttribute('title')).toBe(false);
  expect(Reflect.get(button, '$_ptooltipValue')).toBe('Publish Subgraph');
  expect(text.textContent).toBe('Publish Subgraph');
});

test('removes a Cube-native title and reports a missing PrimeVue contract once', () => {
  const warn = jest.fn();
  const button = document.createElement('button');
  button.title = 'Host title';
  const adapter = new PrimeVueTooltipPresentationAdapter({ document, logger: { warn } });
  const original = adapter.capture(button);

  adapter.present(button, 'Save Cube');
  adapter.present(button, 'Save Cube');

  expect(button.hasAttribute('title')).toBe(false);
  expect(warn).toHaveBeenCalledTimes(1);

  adapter.restore(button, original);

  expect(button.title).toBe('Host title');
  expect(Reflect.has(button, '$_ptooltipValue')).toBe(false);
});

test('does not mutate an already-correct mounted tooltip during reconciliation', async () => {
  document.body.replaceChildren();
  const button = document.createElement('button');
  Reflect.set(button, '$_ptooltipValue', 'Save Cube');
  Reflect.set(button, '$_ptooltipId', 'save-tooltip');
  const tooltip = document.createElement('div');
  tooltip.id = 'save-tooltip';
  const text = document.createElement('span');
  text.className = 'p-tooltip-text';
  text.textContent = 'Save Cube';
  tooltip.append(text);
  document.body.append(button, tooltip);
  const mutations = jest.fn();
  const observer = new MutationObserver(mutations);
  observer.observe(text, { childList: true, characterData: true, subtree: true });
  const adapter = new PrimeVueTooltipPresentationAdapter({ document, logger: console });

  adapter.present(button, 'Save Cube');
  await Promise.resolve();

  expect(mutations).not.toHaveBeenCalled();
  observer.disconnect();
});
