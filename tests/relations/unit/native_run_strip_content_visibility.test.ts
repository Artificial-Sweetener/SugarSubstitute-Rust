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
/** Prove empty legacy chrome gives up space and restores live native controls. */
import { afterEach, expect, test } from '@jest/globals';
import { NativeRunStripContentVisibility } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeRunStripContentVisibility.js';
import { ensureRelationStyles } from '../../../frontend/comfyui/ui/relations/presentation/RelationStyles.js';

afterEach(() => {
  document.body.replaceChildren();
  document.getElementById('sc-relation-styles')?.remove();
});

/** Model the native wrappers around a button hidden by another extension. */
function fixture() {
  ensureRelationStyles(document);
  const bar = document.createElement('div');
  bar.className = 'actionbar-container';
  const legacy = document.createElement('div');
  legacy.dataset.testid = 'legacy-topbar-container';
  const group = document.createElement('div');
  group.className = 'comfyui-button-group';
  const button = document.createElement('button');
  button.textContent = 'Image Feed';
  button.style.display = 'none';
  group.append(button);
  legacy.append(group);
  bar.append(legacy);
  document.body.append(bar);
  const visibility = new NativeRunStripContentVisibility();
  visibility.mount(bar);
  return { bar, legacy, group, button, visibility };
}

test('removes empty wrapper space and restores a revealed button without replacing it', async () => {
  const f = fixture();
  try {
    expect(getComputedStyle(f.legacy).display).toBe('none');
    f.button.style.display = '';
    await Promise.resolve();
    expect(f.legacy.hasAttribute('data-sc-empty-run-content')).toBe(false);
    expect(getComputedStyle(f.legacy).display).not.toBe('none');
    expect(f.legacy.querySelector('button')).toBe(f.button);
    f.group.hidden = true;
    await Promise.resolve();
    expect(getComputedStyle(f.legacy).display).toBe('none');
    f.group.hidden = false;
    await Promise.resolve();
    expect(getComputedStyle(f.legacy).display).not.toBe('none');
  } finally {
    f.visibility.dispose();
  }
});

test('keeps newly mounted input and text surfaces, then collapses after their removal', async () => {
  const f = fixture();
  try {
    const input = document.createElement('input');
    input.value = 'preserved';
    f.group.append(input);
    await Promise.resolve();
    expect(getComputedStyle(f.legacy).display).not.toBe('none');
    expect(input.value).toBe('preserved');
    input.remove();
    await Promise.resolve();
    expect(getComputedStyle(f.legacy).display).toBe('none');
    f.legacy.append(document.createTextNode('Native status'));
    await Promise.resolve();
    expect(getComputedStyle(f.legacy).display).not.toBe('none');
  } finally {
    f.visibility.dispose();
  }
});

test('releases the previous surface on replacement and restores native policy on undocking', async () => {
  const f = fixture();
  try {
    const replacement = f.legacy.cloneNode(false);
    if (!(replacement instanceof HTMLElement)) throw Error('Missing native container');
    f.legacy.replaceWith(replacement);
    f.visibility.mount(f.bar);
    expect(f.legacy.hasAttribute('data-sc-empty-run-content')).toBe(false);
    expect(getComputedStyle(replacement).display).toBe('none');
    f.visibility.mount(null);
    expect(replacement.hasAttribute('data-sc-empty-run-content')).toBe(false);
    replacement.append(document.createElement('button'));
    await Promise.resolve();
    expect(replacement.hasAttribute('data-sc-empty-run-content')).toBe(false);
  } finally {
    f.visibility.dispose();
  }
});
