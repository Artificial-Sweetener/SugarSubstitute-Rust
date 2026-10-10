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
/** Prove named boolean presentation delegates edits and preserves native state and identity. */
import { afterEach, expect, jest, test } from '@jest/globals';
import {
  NativeBooleanSegments,
  type FluentBooleanSegment,
  type FluentBooleanSegmentsControls,
} from '../../../frontend/comfyui/ui/surface/appearance/NativeBooleanSegments.js';
import { NativeToggleRows } from '../../../frontend/comfyui/ui/surface/appearance/NativeToggleRows.js';
import { fluentInteractionCss } from '../../../frontend/comfyui/ui/surface/appearance/FluentInteractionStyles.js';

const adapters = new Set<NativeBooleanSegments>();

afterEach(() => {
  for (const adapter of adapters) adapter.clear();
  adapters.clear();
  document.body.replaceChildren();
});

test('shared indicator eases across the selection and remains immediate for reduced motion', () => {
  const stylesheet = document.createElement('style');
  stylesheet.textContent = fluentInteractionCss();
  document.body.append(stylesheet);
  const rules = Array.from(stylesheet.sheet?.cssRules ?? []);
  const indicator = rules.find(
    (rule): rule is CSSStyleRule =>
      rule instanceof CSSStyleRule && rule.selectorText === '[data-sc-boolean-segments]::after',
  );
  expect(indicator?.style.getPropertyValue('transition-duration')).toBe('var(--durationNormal)');
  expect(indicator?.style.getPropertyValue('transition-timing-function')).toBe(
    'var(--curveEasyEase)',
  );
  const reducedMotion = rules.find(
    (rule): rule is CSSMediaRule =>
      rule instanceof CSSMediaRule && rule.conditionText === '(prefers-reduced-motion:reduce)',
  );
  const reducedIndicator = Array.from(reducedMotion?.cssRules ?? []).find(
    (rule): rule is CSSStyleRule =>
      rule instanceof CSSStyleRule &&
      rule.selectorText.includes('[data-sc-boolean-segments]::after'),
  );
  expect(reducedIndicator?.style.getPropertyValue('transition')).toBe('none');
});

/** Model only the external Fluent property boundary, leaving native buttons and events real. */
function createControls(documentRef: Document): FluentBooleanSegmentsControls {
  const group = Object.assign(documentRef.createElement('div'), { activeid: '' });
  const off = documentRef.createElement('button');
  const on = documentRef.createElement('button');
  off.id = `test-off-${documentRef.querySelectorAll('[data-sc-boolean-segments]').length}`;
  on.id = `test-on-${documentRef.querySelectorAll('[data-sc-boolean-segments]').length}`;
  let active = '';
  Object.defineProperty(group, 'activeid', {
    get: () => active,
    set: (id: string) => {
      if (active === id) return;
      active = id;
      for (const button of [off, on])
        button.setAttribute('aria-selected', String(button.id === id));
      group.dispatchEvent(new Event('change'));
    },
  });
  for (const button of [off, on])
    button.addEventListener('click', () => {
      group.activeid = button.id;
    });
  return { group, segments: [off, on], release: jest.fn() };
}

/** Construct the host's named off/on choices with one native boolean edit owner. */
function fixture() {
  const root = document.createElement('div');
  const row = document.createElement('div');
  row.dataset.scFluentRow = '';
  row.dataset.scFluentWidget = 'WidgetToggleSwitch';
  const label = document.createElement('div');
  label.dataset.testid = 'widget-layout-field-label';
  label.textContent = '<literal> Combine Segs';
  const native = document.createElement('div');
  native.setAttribute('role', 'group');
  const off = document.createElement('button');
  const on = document.createElement('button');
  off.value = 'off';
  off.textContent = '<img src=x> separate';
  on.value = 'on';
  on.textContent = 'combined';
  const edit = jest.fn((value: boolean) => {
    off.setAttribute('aria-pressed', String(!value));
    on.setAttribute('aria-pressed', String(value));
  });
  off.addEventListener('click', () => edit(false));
  on.addEventListener('click', () => edit(true));
  edit(true);
  edit.mockClear();
  native.append(off, on);
  row.append(label, native);
  root.append(row);
  document.body.append(root);
  const adapter = new NativeBooleanSegments(createControls);
  adapters.add(adapter);
  adapter.reconcile(root);
  const segments = Array.from(
    row.querySelectorAll<FluentBooleanSegment>('[data-sc-boolean-segment]'),
  );
  const [separate, combined] = segments;
  if (!separate || !combined) throw new Error('Missing named segments');
  return { root, row, native, off, on, edit, adapter, segments, separate, combined };
}

test('delegates one edit per changed choice and never makes the inert row clickable or focusable', async () => {
  const { row, native, edit, separate, combined } = fixture();
  const bubbled = jest.fn();
  const leakedSelection = jest.fn();
  row.addEventListener('click', bubbled);
  row.addEventListener('change', leakedSelection);
  row.click();
  expect(edit).not.toHaveBeenCalled();
  expect(row.hasAttribute('tabindex')).toBe(false);
  expect(row.hasAttribute('data-sc-toggle-row')).toBe(false);
  expect(native.hasAttribute('data-sc-boolean-native')).toBe(true);
  expect(combined.getAttribute('aria-selected')).toBe('true');
  separate.click();
  expect(edit).toHaveBeenCalledTimes(1);
  expect(edit).toHaveBeenLastCalledWith(false);
  expect(separate.getAttribute('aria-selected')).toBe('true');
  expect(combined.getAttribute('aria-selected')).toBe('false');
  separate.click();
  expect(edit).toHaveBeenCalledTimes(1);
  combined.click();
  await Promise.resolve();
  expect(edit).toHaveBeenCalledTimes(2);
  expect(edit).toHaveBeenLastCalledWith(true);
  expect(bubbled).toHaveBeenCalledTimes(3);
  expect(leakedSelection).not.toHaveBeenCalled();
  expect(row.querySelector('img')).toBeNull();
  expect(separate.textContent).toBe('<img src=x> separate');
  expect(separate.parentElement?.getAttribute('aria-label')).toBe('<literal> Combine Segs');
});

test('delegates official selection changes and respects disabled choices without creating a second value owner', async () => {
  const { edit, off, separate, combined } = fixture();
  const group = separate.parentElement;
  if (!group || !('activeid' in group)) throw new Error('Missing selection owner');
  separate.tabIndex = 7;
  combined.tabIndex = 8;
  group.activeid = separate.id;
  group.dispatchEvent(new Event('change'));
  await Promise.resolve();
  expect(edit).toHaveBeenLastCalledWith(false);
  expect(separate.tabIndex).toBe(7);
  expect(combined.tabIndex).toBe(8);
  group.activeid = combined.id;
  group.dispatchEvent(new Event('change'));
  expect(edit).toHaveBeenLastCalledWith(true);
  off.disabled = true;
  await Promise.resolve();
  expect(separate.disabled).toBe(true);
  group.activeid = separate.id;
  group.dispatchEvent(new Event('change'));
  separate.click();
  await Promise.resolve();
  expect(edit).toHaveBeenCalledTimes(2);
  expect(group.activeid).toBe(combined.id);
});

test('observes host updates and keeps control identity through repeated reconciliation and presentation moves', async () => {
  const { root, row, off, on, edit, adapter, separate, combined } = fixture();
  combined.focus();
  const unrelated = document.createElement('div');
  unrelated.setAttribute('role', 'listbox');
  root.append(unrelated);
  unrelated.textContent = 'Unrelated popup';
  adapter.reconcile(root);
  adapter.reconcile(root);
  expect(row.querySelectorAll('[data-sc-boolean-segments]')).toHaveLength(1);
  expect(row.querySelector('[data-sc-boolean-segment]')).toBe(separate);
  expect(document.activeElement).toBe(combined);
  expect(edit).not.toHaveBeenCalled();
  off.click();
  await Promise.resolve();
  expect(separate.getAttribute('aria-selected')).toBe('true');
  expect(combined.getAttribute('aria-selected')).toBe('false');
  on.textContent = '<b> combined';
  await Promise.resolve();
  expect(combined.textContent).toBe('<b> combined');
  expect(combined.querySelector('b')).toBeNull();
  const destination = document.createElement('div');
  root.append(destination);
  destination.append(row);
  adapter.reconcile(root);
  expect(row.querySelector('[data-sc-boolean-segment]')).toBe(separate);
  adapter.clear();
  expect(row.querySelector('[data-sc-boolean-segments]')).toBeNull();
  expect(off.getAttribute('aria-pressed')).toBe('true');
  on.click();
  expect(on.getAttribute('aria-pressed')).toBe('true');
  expect(edit).toHaveBeenCalledTimes(2);
  adapter.reconcile(root);
  expect(row.querySelectorAll('[data-sc-boolean-segments]')).toHaveLength(1);
});

test('restores native groups when their widget changes or disappears and ignores custom button groups', () => {
  const { root, row, native, adapter, separate } = fixture();
  row.dataset.scFluentWidget = 'WidgetDOM';
  adapter.reconcile(root);
  expect(native.hasAttribute('data-sc-boolean-native')).toBe(false);
  expect(separate.isConnected).toBe(false);
  row.dataset.scFluentWidget = 'WidgetToggleSwitch';
  adapter.reconcile(root);
  expect(native.hasAttribute('data-sc-boolean-native')).toBe(true);
  native.remove();
  adapter.reconcile(root);
  expect(row.querySelector('[data-sc-boolean-segments]')).toBeNull();
  native.querySelector('button')?.removeAttribute('value');
  row.append(native);
  adapter.reconcile(root);
  expect(native.hasAttribute('data-sc-boolean-native')).toBe(false);
});

test('rebinds replaced native buttons instead of editing detached host controls', () => {
  const { root, row, off, on, adapter, separate, edit } = fixture();
  const replacement = document.createElement('button');
  replacement.value = 'off';
  replacement.textContent = 'separate';
  replacement.setAttribute('aria-pressed', 'false');
  const replacementEdit = jest.fn(() => {
    replacement.setAttribute('aria-pressed', 'true');
    on.setAttribute('aria-pressed', 'false');
  });
  replacement.addEventListener('click', replacementEdit);
  off.replaceWith(replacement);
  adapter.reconcile(root);
  expect(separate.isConnected).toBe(false);
  row.querySelector<HTMLElement>('[data-sc-boolean-segment]')?.click();
  expect(replacementEdit).toHaveBeenCalledTimes(1);
  expect(edit).not.toHaveBeenCalled();
  expect(row.querySelectorAll('[data-sc-boolean-segments]')).toHaveLength(1);
});

test('reflects a deferred host edit and ignores pending refreshes after release', async () => {
  const { root, row, native, off, on, adapter, separate } = fixture();
  const replacement = document.createElement('button');
  replacement.value = 'off';
  replacement.textContent = 'separate';
  replacement.setAttribute('aria-pressed', 'false');
  replacement.addEventListener('click', () =>
    queueMicrotask(() => {
      replacement.setAttribute('aria-pressed', 'true');
      on.setAttribute('aria-pressed', 'false');
    }),
  );
  off.replaceWith(replacement);
  adapter.reconcile(root);
  const current = row.querySelector<FluentBooleanSegment>('[data-sc-boolean-segment]');
  current?.click();
  await Promise.resolve();
  expect(current?.getAttribute('aria-selected')).toBe('true');
  adapter.clear();
  await Promise.resolve();
  expect(native.hasAttribute('data-sc-boolean-native')).toBe(false);
  expect(row.querySelector('[data-sc-boolean-segments]')).toBeNull();
  expect(separate.isConnected).toBe(false);
});

test('checkbox to named-choice replacement removes checkbox row affordances and safely restores them on return', () => {
  const { root, row, native, adapter } = fixture();
  adapter.clear();
  native.remove();
  const input = document.createElement('input');
  input.type = 'checkbox';
  row.append(input);
  const checkboxes = new NativeToggleRows((documentRef) => {
    const checkbox = documentRef.createElement('input');
    checkbox.type = 'checkbox';
    return checkbox;
  });
  checkboxes.reconcile(root);
  expect(row.hasAttribute('data-sc-toggle-row')).toBe(true);
  input.remove();
  row.append(native);
  checkboxes.reconcile(root);
  adapter.reconcile(root);
  expect(row.hasAttribute('data-sc-toggle-row')).toBe(false);
  expect(row.hasAttribute('tabindex')).toBe(false);
  expect(row.querySelector('[data-sc-boolean-control]')).toBeNull();
  native.replaceWith(input);
  checkboxes.reconcile(root);
  adapter.reconcile(root);
  expect(row.querySelector('[data-sc-boolean-segments]')).toBeNull();
  expect(row.hasAttribute('data-sc-toggle-row')).toBe(true);
  checkboxes.clear();
  expect(row.hasAttribute('tabindex')).toBe(false);
});
