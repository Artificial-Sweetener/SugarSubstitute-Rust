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
/** Prove native state overlays share the theme's card shape without clipping custom content. */
import { afterEach, expect, test } from '@jest/globals';
import { fluentAppearanceCss } from '../../../frontend/comfyui/ui/surface/appearance/FluentAppearanceStyles.js';
import { ensureOwnedControlAppearanceStyles } from '../../../frontend/comfyui/ui/surface/appearance/OwnedControlAppearanceStyles.js';

test('owns Fluent control foregrounds while image-backed picker inputs keep their separate contrast owner', () => {
  ensureOwnedControlAppearanceStyles(document);
  const style = document.getElementById('sugarcubes-owned-control-appearance');
  if (!(style instanceof HTMLStyleElement)) throw new Error('Missing control appearance');
  const root = document.createElement('section');
  root.setAttribute('data-sc-style-node', '');
  const input = document.createElement('input');
  const combo = document.createElement('button');
  combo.setAttribute('role', 'combobox');
  const imageField = document.createElement('div');
  imageField.className = 'sc-model-field';
  const imageInput = document.createElement('input');
  imageField.append(imageInput);
  root.append(input, combo, imageField);
  document.body.append(root);
  const rule = Array.from(style.sheet?.cssRules ?? []).find(
    (entry) =>
      entry instanceof CSSStyleRule && entry.selectorText.includes(':not(.sc-model-field input)'),
  );
  if (!(rule instanceof CSSStyleRule)) throw new Error('Missing owned foreground rule');
  expect(rule.style.color).toBe('var(--colorNeutralForeground1,var(--fg-color,#eee))');
  expect(input.matches(rule.selectorText)).toBe(true);
  expect(combo.matches(rule.selectorText)).toBe(true);
  expect(imageInput.matches(rule.selectorText)).toBe(false);
});

afterEach(() => {
  document.head.replaceChildren();
  document.body.replaceChildren();
});

test.each([false, true])(
  'owns native card corners in masonry=%s across bypass, mute and collapse',
  (masonry) => {
    const style = document.createElement('style');
    // Resolve the radius token because JSDOM cannot compute inherited CSS variables.
    style.textContent = fluentAppearanceCss(true).replaceAll('var(--sc-node-card-radius)', '4px');
    document.head.append(style);
    const surface = document.createElement('div');
    if (masonry) surface.className = 'sugarcubes-workflow-masonry';
    const card = document.createElement('div');
    card.dataset.scStyleNode = '';
    const wrapper = document.createElement('div');
    wrapper.dataset.testid = 'node-inner-wrapper';
    const header = document.createElement('div');
    header.dataset.testid = 'node-header-test';
    wrapper.append(header);
    card.append(wrapper);
    surface.append(card);
    document.body.append(surface);
    for (const state of ['active', 'bypassed', 'muted']) {
      wrapper.className = `native-state-${state}`;
      expect(getComputedStyle(wrapper).borderRadius).toBe('4px');
      card.dataset.cubeFaceBody = 'header-only';
      expect(getComputedStyle(header).borderRadius).toBe('4px');
      delete card.dataset.cubeFaceBody;
      header.dataset.scTitlebarCollapsed = 'true';
      expect(getComputedStyle(header).borderRadius).toBe('4px');
      delete header.dataset.scTitlebarCollapsed;
    }
    // JSDOM cannot paint pseudo-elements; inspect their parsed rule and verify painting in the browser.
    const overlayRule = Array.from(style.sheet?.cssRules ?? []).find(
      (rule) =>
        rule instanceof CSSStyleRule &&
        rule.selectorText === '[data-sc-style-node] > [data-testid="node-inner-wrapper"]::before',
    );
    if (!(overlayRule instanceof CSSStyleRule)) throw new Error('Missing state-overlay rule');
    expect(overlayRule.style.getPropertyValue('border-radius')).toBe('inherit');
    expect(getComputedStyle(wrapper).overflow).not.toBe('hidden');
  },
);
