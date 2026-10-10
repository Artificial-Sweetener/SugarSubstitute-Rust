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
/** Prove shared token boundaries release without retaining retired native surfaces. */
import { afterEach, expect, test } from '@jest/globals';
import { NativeFluentThemeScopes } from '../../../frontend/comfyui/ui/surface/appearance/NativeFluentThemeScopes.js';
import { fluentAppearanceCss } from '../../../frontend/comfyui/ui/surface/appearance/FluentAppearanceStyles.js';
import { ensureWorkflowMasonryStyles } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryStyles.js';

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
});

test('shares graph tokens and keeps independent toolbar and popup scopes isolated', () => {
  const scopes = new NativeFluentThemeScopes();
  const graph = document.createElement('div');
  graph.id = 'graph-canvas-container';
  const cards = [document.createElement('div'), document.createElement('div')];
  graph.append(...cards);
  const toolbar = document.createElement('div');
  toolbar.className = 'sc-overrides-toolbar';
  const field = document.createElement('div');
  toolbar.append(field);
  const popup = document.createElement('div');
  document.body.append(graph, toolbar, popup);
  cards.forEach((card) => scopes.retain(card));
  scopes.retain(field);
  scopes.retain(popup);
  expect([...document.querySelectorAll('[data-sc-fluent-theme-root]')]).toEqual([
    graph,
    toolbar,
    popup,
  ]);
  popup.remove();
  scopes.prune();
  expect(popup.hasAttribute('data-sc-fluent-theme-root')).toBe(false);
  toolbar.setAttribute('data-sc-fluent-theme-root', 'host-owned');
  scopes.clear();
  expect(graph.hasAttribute('data-sc-fluent-theme-root')).toBe(false);
  expect(toolbar.getAttribute('data-sc-fluent-theme-root')).toBe('host-owned');
});

test.each([
  [true, '#202020'],
  [false, '#f3f3f3'],
])('shares the masonry workspace color with sibling chrome for dark=%s', (dark, color) => {
  const theme = document.createElement('style');
  theme.textContent = fluentAppearanceCss(dark);
  document.head.append(theme);
  const themeRules = Array.from(theme.sheet!.cssRules).filter(
    (rule): rule is CSSStyleRule => rule instanceof CSSStyleRule,
  );
  const root = themeRules.find((rule) => rule.selectorText === '[data-sc-fluent-theme-root]');
  expect(root?.style.getPropertyValue('--sugarcubes-masonry-surface')).toBe(color);
  expect(
    themeRules.filter((rule) => rule.style.getPropertyValue('--sugarcubes-masonry-surface')),
  ).toHaveLength(1);
  ensureWorkflowMasonryStyles(document);
  const layout = document.getElementById('sugarcubes-workflow-masonry-styles') as HTMLStyleElement;
  const layoutRules = Array.from(layout.sheet!.cssRules).filter(
    (rule): rule is CSSStyleRule => rule instanceof CSSStyleRule,
  );
  const chrome = layoutRules.find(
    (rule) => rule.selectorText === '#graph-canvas-container[data-sugarcubes-masonry="true"]',
  );
  const backdrop = layoutRules.find(
    (rule) => rule.selectorText === '.sugarcubes-workflow-masonry__backdrop',
  );
  expect(chrome?.style.getPropertyValue('background')).toBe(
    backdrop?.style.getPropertyValue('background'),
  );
  expect(chrome?.style.getPropertyValue('background')).toContain('--sugarcubes-masonry-surface');
});
