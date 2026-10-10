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
/** Prove the official Microsoft control activates named native choices and retains selection. */
import { afterAll, afterEach, beforeAll, expect, test } from '@jest/globals';
import { Updates } from '@microsoft/fast-element';
import { Tab } from '@fluentui/web-components/tab/class.js';
import { Tablist } from '@fluentui/web-components/tablist/class.js';
import { createFluentBooleanSegments } from '../../../frontend/comfyui/ui/surface/appearance/FluentThemeRuntime.js';
import { NativeBooleanSegments } from '../../../frontend/comfyui/ui/surface/appearance/NativeBooleanSegments.js';

const internalsDescriptor = Object.getOwnPropertyDescriptor(
  HTMLElement.prototype,
  'attachInternals',
);
const adapter = new NativeBooleanSegments(createFluentBooleanSegments);

beforeAll(() => {
  // JSDOM lacks custom element internals; keep Microsoft's actual component and event bindings.
  Object.defineProperty(HTMLElement.prototype, 'attachInternals', {
    configurable: true,
    value: function (this: HTMLElement) {
      const host = this;
      return {
        role: null,
        ariaDisabled: null,
        ariaPressed: null,
        states: new Set<string>(),
        get shadowRoot() {
          return host.shadowRoot;
        },
      };
    },
  });
});

afterEach(() => {
  adapter.clear();
  document.body.replaceChildren();
});

afterAll(() => {
  if (internalsDescriptor)
    Object.defineProperty(HTMLElement.prototype, 'attachInternals', internalsDescriptor);
  else Reflect.deleteProperty(HTMLElement.prototype, 'attachInternals');
});

test('uses the official sliding Tablist for mouse and keyboard without allowing an empty selection', async () => {
  const row = document.createElement('div');
  row.dataset.scFluentRow = '';
  row.dataset.scFluentWidget = 'WidgetToggleSwitch';
  const native = document.createElement('div');
  native.setAttribute('role', 'group');
  const off = document.createElement('button');
  off.value = 'off';
  off.textContent = 'separate';
  const on = document.createElement('button');
  on.value = 'on';
  on.textContent = 'combined';
  let edits = 0;
  const update = (value: boolean): void => {
    off.setAttribute('aria-pressed', String(!value));
    on.setAttribute('aria-pressed', String(value));
  };
  off.addEventListener('click', () => {
    edits++;
    queueMicrotask(() => update(false));
  });
  on.addEventListener('click', () => {
    edits++;
    queueMicrotask(() => update(true));
  });
  update(true);
  native.append(off, on);
  row.append(native);
  document.body.append(row);
  adapter.reconcile(document.body);
  const [separate, combined] = row.querySelectorAll('fluent-tab');
  if (!(separate instanceof Tab) || !(combined instanceof Tab))
    throw new Error('Official Fluent tabs missing');
  const group = row.querySelector('fluent-tablist');
  expect(group).toBeInstanceOf(Tablist);
  expect(separate.shadowRoot).not.toBeNull();
  expect(group?.shadowRoot).not.toBeNull();
  await Updates.next();
  separate.focus();
  separate.click();
  await Promise.resolve();
  expect(edits).toBe(1);
  expect(separate.getAttribute('aria-selected')).toBe('true');
  expect(combined.getAttribute('aria-selected')).toBe('false');
  separate.click();
  await Promise.resolve();
  expect(edits).toBe(1);
  expect(separate.getAttribute('aria-selected')).toBe('true');
  combined.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  await Promise.resolve();
  expect(edits).toBe(2);
  expect(combined.getAttribute('aria-selected')).toBe('true');
  expect(separate.getAttribute('aria-selected')).toBe('false');
  combined.dispatchEvent(new KeyboardEvent('keydown', { key: ' ', bubbles: true }));
  await Promise.resolve();
  expect(edits).toBe(2);
  expect(combined.getAttribute('aria-selected')).toBe('true');
  await Updates.next();
  combined.focus();
  combined.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
  await Updates.next();
  expect(document.activeElement).toBe(separate);
  expect(off.getAttribute('aria-pressed')).toBe('true');
  separate.dispatchEvent(new KeyboardEvent('keydown', { key: 'End', bubbles: true }));
  await Updates.next();
  expect(document.activeElement).toBe(combined);
  expect(on.getAttribute('aria-pressed')).toBe('true');
  const destination = document.createElement('div');
  document.body.append(destination);
  row.remove();
  await Updates.next();
  destination.append(row);
  await Updates.next();
  combined.focus();
  combined.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
  await Updates.next();
  expect(document.activeElement).toBe(separate);
  expect(off.getAttribute('aria-pressed')).toBe('true');
  separate.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  await Updates.next();
  expect(document.activeElement).toBe(combined);
  expect(on.getAttribute('aria-pressed')).toBe('true');
  off.disabled = true;
  await Promise.resolve();
  combined.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
  await Updates.next();
  expect(document.activeElement).toBe(combined);
  expect(on.getAttribute('aria-pressed')).toBe('true');
  const identity = row.querySelector('fluent-tablist');
  adapter.reconcile(document.body);
  expect(row.querySelector('fluent-tablist')).toBe(identity);
  adapter.clear();
  expect(native.hasAttribute('data-sc-boolean-native')).toBe(false);
  expect(on.getAttribute('aria-pressed')).toBe('true');
});
