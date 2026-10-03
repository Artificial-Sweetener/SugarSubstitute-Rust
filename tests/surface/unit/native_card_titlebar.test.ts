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
/** Prove titlebar action priority, native control delegation, and reversible ownership. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { NativeCardTitlebar } from '../../../frontend/comfyui/ui/surface/appearance/NativeCardTitlebar.js';
import { fluentInteractionCss } from '../../../frontend/comfyui/ui/surface/appearance/FluentInteractionStyles.js';

afterEach(() => document.body.replaceChildren());

test('titlebar reserves trailing space for a visible chevron and retains Substitute switch padding', () => {
  const { header, collapse } = fixture(true);
  header.dataset.scTitlebarAction = 'toggle';
  header.dataset.scTitlebarControls = '';
  const chevronHeader =
    '[data-sc-titlebar-controls]:has([data-testid="node-collapse-button"]:not([hidden]))';
  expect(header.matches(chevronHeader)).toBe(true);
  collapse.hidden = true;
  expect(header.matches(chevronHeader)).toBe(false);
  collapse.remove();
  expect(header.matches(chevronHeader)).toBe(false);
  const css = fluentInteractionCss();
  expect(css).toContain('[data-sc-titlebar-controls] > :first-child { padding-right:0; }');
  expect(css).toContain(`${chevronHeader} > :first-child {\n      padding-right:20px;`);
  expect(css).toContain(
    '[data-sc-titlebar-controls] [data-cube-card-activation] { padding-left:0; padding-right:8px; }',
  );
});

/** Create a native header with independent activation and collapse controls. */
function fixture(
  activation = false,
  masonry = true,
): {
  root: HTMLElement;
  header: HTMLElement;
  collapse: HTMLButtonElement;
  enabled: HTMLInputElement | null;
} {
  const root = document.createElement('div');
  const header = document.createElement('div');
  const collapse = document.createElement('button');
  collapse.dataset.testid = 'node-collapse-button';
  const title = document.createElement('span');
  title.dataset.testid = 'node-title';
  title.textContent = '<img src=x> Prompt';
  header.append(title, collapse);
  let enabled: HTMLInputElement | null = null;
  if (activation) {
    const label = document.createElement('label');
    label.dataset.cubeCardActivation = 'prompt';
    enabled = document.createElement('input');
    enabled.type = 'checkbox';
    enabled.checked = true;
    label.append(enabled);
    header.append(label);
  }
  root.append(header);
  const surface = document.createElement('div');
  if (masonry) surface.className = 'sugarcubes-workflow-masonry';
  surface.append(root);
  document.body.append(surface);
  return { root, header, collapse, enabled };
}

test('titlebar toggles the existing activation control before collapse; chevron remains independent', () => {
  const { root, header, collapse, enabled } = fixture(true);
  const collapseAction = jest.fn();
  const change = jest.fn();
  collapse.addEventListener('click', collapseAction);
  enabled?.addEventListener('change', change);
  const adapter = new NativeCardTitlebar();
  adapter.reconcile(root, header, false);
  header.click();
  expect(enabled?.checked).toBe(false);
  expect(change).toHaveBeenCalledTimes(1);
  expect(collapseAction).not.toHaveBeenCalled();
  collapse.click();
  expect(collapseAction).toHaveBeenCalledTimes(1);
  expect(header.querySelector('img')).toBeNull();
  adapter.clear();
  expect(header.hasAttribute('data-sc-titlebar-action')).toBe(false);
  expect(header.hasAttribute('tabindex')).toBe(false);
  header.click();
  expect(change).toHaveBeenCalledTimes(1);
});

test('titlebar supports keyboard collapse and preserves child controls and dragging', () => {
  const { root, header, collapse } = fixture();
  const action = jest.fn();
  collapse.addEventListener('click', action);
  const adapter = new NativeCardTitlebar();
  adapter.reconcile(root, header, false);
  expect(header.getAttribute('aria-expanded')).toBe('true');
  header.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  expect(action).toHaveBeenCalledTimes(1);
  const input = document.createElement('input');
  header.append(input);
  input.click();
  input.dispatchEvent(new KeyboardEvent('keydown', { key: ' ', bubbles: true }));
  expect(action).toHaveBeenCalledTimes(1);
  header.dispatchEvent(new MouseEvent('pointerdown', { clientX: 0, clientY: 0, bubbles: true }));
  header.dispatchEvent(new MouseEvent('pointerup', { clientX: 20, clientY: 0, bubbles: true }));
  header.click();
  expect(action).toHaveBeenCalledTimes(1);
  adapter.reconcile(root, header, true);
  expect(collapse.getAttribute('aria-expanded')).toBe('false');
  adapter.clear();
});

test('inert headers receive no action and replaced headers release their listeners', () => {
  const { root, header, collapse } = fixture();
  collapse.disabled = true;
  const adapter = new NativeCardTitlebar();
  adapter.reconcile(root, header, false);
  expect(header.hasAttribute('data-sc-titlebar-action')).toBe(false);
  collapse.disabled = false;
  const action = jest.fn();
  collapse.addEventListener('click', action);
  adapter.reconcile(root, header, false);
  const replacement = document.createElement('div');
  root.replaceChildren(replacement);
  adapter.reconcile(root, replacement, false);
  header.click();
  expect(action).not.toHaveBeenCalled();
  adapter.clear();
});

test('widgetless cards keep activation but hide collapse without manufacturing state', () => {
  const { root, header, collapse, enabled } = fixture(true);
  const action = jest.fn();
  collapse.addEventListener('click', action);
  const adapter = new NativeCardTitlebar();
  adapter.reconcile(root, header, true, false);
  expect(collapse.hidden).toBe(true);
  expect(header.dataset.scTitlebarAction).toBe('activation');
  header.click();
  expect(enabled?.checked).toBe(false);
  expect(action).not.toHaveBeenCalled();
  adapter.clear();
  expect(collapse.hidden).toBe(false);
});

test('graph headers leave collapse to the chevron and preserve native selection and drag events', () => {
  const { root, header, collapse } = fixture(false, false);
  const action = jest.fn();
  const selected = jest.fn();
  const pointer = jest.fn();
  collapse.addEventListener('click', action);
  root.addEventListener('click', selected);
  root.addEventListener('pointerdown', pointer);
  const adapter = new NativeCardTitlebar();
  adapter.reconcile(root, header, false);
  expect(header.hasAttribute('data-sc-titlebar-action')).toBe(false);
  expect(header.hasAttribute('role')).toBe(false);
  expect(header.hasAttribute('tabindex')).toBe(false);
  expect(header.hasAttribute('data-sc-titlebar-controls')).toBe(true);
  header.click();
  header.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  header.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true }));
  expect(action).not.toHaveBeenCalled();
  expect(selected).toHaveBeenCalledTimes(1);
  expect(pointer).toHaveBeenCalledTimes(1);
  collapse.click();
  expect(action).toHaveBeenCalledTimes(1);
  root.parentElement?.classList.add('sugarcubes-workflow-masonry');
  adapter.reconcile(root, header, false);
  header.click();
  expect(action).toHaveBeenCalledTimes(2);
  root.parentElement?.classList.remove('sugarcubes-workflow-masonry');
  adapter.reconcile(root, header, false);
  header.click();
  expect(action).toHaveBeenCalledTimes(2);
  adapter.clear();
  expect(header.hasAttribute('data-sc-titlebar-controls')).toBe(false);
});

test('graph title activation remains independent of chevron-only collapse', () => {
  const { root, header, collapse, enabled } = fixture(true, false);
  const action = jest.fn();
  collapse.addEventListener('click', action);
  const adapter = new NativeCardTitlebar();
  adapter.reconcile(root, header, false);
  expect(header.dataset.scTitlebarAction).toBe('activation');
  header.click();
  expect(enabled?.checked).toBe(false);
  expect(action).not.toHaveBeenCalled();
  collapse.click();
  expect(action).toHaveBeenCalledTimes(1);
  adapter.clear();
});
