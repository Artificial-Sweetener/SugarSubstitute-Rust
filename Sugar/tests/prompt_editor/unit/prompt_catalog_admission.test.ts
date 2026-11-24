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
/** Prove background model data never interrupts a prompt a person is using. */
import { PromptCatalogAdmission } from '../../../frontend/comfyui/ui/prompt_editor/runtime/PromptCatalogAdmission.js';
import type { LoraResourceAdapter } from '../../../frontend/sugarprompt/services/src/index.js';

function catalog(revision: string): LoraResourceAdapter {
  return { revision, resolve: async () => [] };
}

function mount(): {
  element: HTMLElement;
  field: HTMLTextAreaElement;
  elsewhere: HTMLButtonElement;
  applied: string[];
  admission: PromptCatalogAdmission;
  settle: () => Promise<void>;
} {
  const element = document.createElement('div');
  const field = document.createElement('textarea');
  const elsewhere = document.createElement('button');
  element.append(field);
  document.body.append(element, elsewhere);
  const applied: string[] = [];
  const admission = new PromptCatalogAdmission(element, {
    catalog: (adapter) => applied.push(adapter.revision),
    media: () => applied.push('images'),
  });
  return { element, field, elsewhere, applied, admission, settle: () => Promise.resolve() };
}

afterEach(() => document.body.replaceChildren());

test('the first catalog is adopted at once and later ones wait while the prompt has focus', async () => {
  const { field, elsewhere, applied, admission, settle } = mount();
  field.focus();
  admission.offerCatalog(catalog('first'));
  expect(applied).toEqual(['first']);
  admission.offerCatalog(catalog('second'));
  admission.offerCatalog(catalog('third'));
  expect(applied).toEqual(['first']);
  elsewhere.focus();
  await settle();
  // Only the newest catalog that waited is adopted.
  expect(applied).toEqual(['first', 'third']);
  admission.offerCatalog(catalog('fourth'));
  expect(applied).toEqual(['first', 'third', 'fourth']);
});

test('a catalog and images wait for the prompt menu to close', async () => {
  const { applied, admission, settle } = mount();
  admission.offerCatalog(catalog('first'));
  admission.menuPresented(true);
  admission.offerMedia();
  admission.offerCatalog(catalog('second'));
  expect(applied).toEqual(['first']);
  admission.menuPresented(false);
  await settle();
  // The catalog that waited carries its own images.
  expect(applied).toEqual(['first', 'second']);
});

test('images are shown while the prompt has focus, but not under a held pointer', async () => {
  const { element, field, applied, admission, settle } = mount();
  admission.offerCatalog(catalog('first'));
  field.focus();
  admission.offerMedia();
  expect(applied).toEqual(['first', 'images']);
  element.dispatchEvent(new Event('pointerdown', { bubbles: true }));
  admission.offerMedia();
  expect(applied).toEqual(['first', 'images']);
  window.dispatchEvent(new Event('pointerup'));
  await settle();
  expect(applied).toEqual(['first', 'images', 'images']);
});

test('nothing held is applied after the editor is disposed', async () => {
  const { field, elsewhere, applied, admission, settle } = mount();
  admission.offerCatalog(catalog('first'));
  field.focus();
  admission.offerCatalog(catalog('second'));
  admission.dispose();
  elsewhere.focus();
  await settle();
  expect(applied).toEqual(['first']);
});
