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
/** Prove the editor's LoRA candidates reach the model library's gallery by installed identity. */
import { expect, jest, test } from '@jest/globals';
import { promptModelCompletions } from '../../../frontend/comfyui/ui/prompt_editor/PromptModelCompletions.js';
import type { ModelCatalogItem } from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';
import type { LoraCatalogItem } from '../../../frontend/sugarprompt/services/src/index.js';
import { promptCatalogFixture } from '../support/PromptCatalogFixture.js';

/** One ranked editor candidate for an installed file. */
function candidate(backendValue: string): LoraCatalogItem {
  return {
    model: { promptName: backendValue, backendValue, basename: backendValue },
    title: backendValue,
    subtitle: null,
    triggerWords: [],
    thumbnail: { status: 'unavailable' },
    modelPageUrl: null,
  };
}

/** The installed record the gallery draws for one file. */
function installed(value: string): ModelCatalogItem {
  const template = promptCatalogFixture().items[0];
  if (!template) throw new Error('Missing catalog fixture');
  return { ...template, id: value, value };
}

/** Mount the adapter over a recording gallery. */
function fixture(known: readonly string[]) {
  const updates: { values: string[]; active: number }[] = [];
  const activated: number[] = [];
  let choose: (index: number) => void = () => undefined;
  const watchers = new Set<() => void>();
  const surface = {
    element: document.createElement('div'),
    update: (items: readonly ModelCatalogItem[], active: number) => {
      updates.push({ values: items.map((item) => item.value), active });
    },
    neighbor: jest.fn((active: number) => active + 1),
    dispose: jest.fn(),
  };
  const popup = promptModelCompletions(
    {
      mountGallery: (events) => {
        choose = events.activate;
        return surface;
      },
    },
    (value) => (known.includes(value) ? installed(value) : null),
    (changed) => {
      watchers.add(changed);
      return () => watchers.delete(changed);
    },
  ).mount({ activate: (index) => activated.push(index) });
  return {
    popup,
    surface,
    updates,
    activated,
    choose: (index: number) => choose(index),
    /** Tell the popup the catalog changed, as the prompt editor host does. */
    catalogChanged: () => {
      for (const changed of [...watchers]) changed();
    },
    watching: () => watchers.size,
  };
}

test('ranked candidates are drawn as their installed models with the same current one', () => {
  const f = fixture(['a.safetensors', 'b.safetensors', 'c.safetensors']);
  expect(f.popup.element).toBe(f.surface.element);
  f.popup.update(['a.safetensors', 'b.safetensors', 'c.safetensors'].map(candidate), 2);
  expect(f.updates).toEqual([
    { values: ['a.safetensors', 'b.safetensors', 'c.safetensors'], active: 2 },
  ]);
  f.choose(1);
  expect(f.activated).toEqual([1]);
  expect(f.popup.neighbor(0, 'ArrowRight')).toBe(1);
  expect(f.surface.neighbor).toHaveBeenCalledWith(0, 'ArrowRight');
  f.popup.dispose();
  expect(f.surface.dispose).toHaveBeenCalledTimes(1);
});

test('a candidate the library no longer holds is left out and positions still agree', () => {
  const f = fixture(['a.safetensors', 'c.safetensors']);
  f.popup.update(['a.safetensors', 'gone.safetensors', 'c.safetensors'].map(candidate), 2);
  expect(f.updates).toEqual([{ values: ['a.safetensors', 'c.safetensors'], active: 1 }]);
  // The gallery's second tile is the editor's third candidate.
  f.choose(1);
  expect(f.activated).toEqual([2]);
  expect(f.popup.neighbor(0, 'ArrowRight')).toBe(2);
  // A current candidate the gallery does not show stays where it is.
  expect(f.popup.neighbor(1, 'ArrowRight')).toBe(1);
  // A neighbour past the shown models keeps the current candidate.
  expect(f.popup.neighbor(2, 'ArrowRight')).toBe(2);
});

test('an open popup draws its candidates again when the catalog changes, until it is disposed', () => {
  const f = fixture(['a.safetensors', 'b.safetensors']);
  f.popup.update(['a.safetensors', 'b.safetensors'].map(candidate), 1);
  const drawn = f.updates.length;
  f.catalogChanged();
  expect(f.updates).toHaveLength(drawn + 1);
  expect(f.updates.at(-1)).toEqual(f.updates[drawn - 1]);
  f.popup.dispose();
  expect(f.watching()).toBe(0);
});
