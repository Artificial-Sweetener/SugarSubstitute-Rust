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
/**
 * A library publishes a new catalog for every change to any model. What an editor is given stays
 * the same object while what it shows is unchanged, so it prepares and draws nothing again.
 */
import { expect, test } from '@jest/globals';
import {
  PROMPT_LIBRARY_REVISION,
  PromptCompletionCatalog,
} from '../../../frontend/comfyui/ui/prompt_editor/runtime/PromptCompletionCatalog.js';
import { sameLoraResources } from '../../../frontend/sugarprompt/services/src/index.js';
import type {
  LoraCompletionCatalog,
  LoraResource,
  LoraSuggestion,
} from '../../../frontend/sugarprompt/services/src/index.js';

function library(authoritative: boolean, found: readonly LoraSuggestion[]): LoraCompletionCatalog {
  return { revision: String(Math.random()), authoritative, search: () => found };
}

test('an editor keeps one completion catalog that searches the library as it is now', () => {
  const first = [{ display: 'first' }] as unknown as readonly LoraSuggestion[];
  const second = [{ display: 'second' }] as unknown as readonly LoraSuggestion[];
  const completion = new PromptCompletionCatalog();
  const held = completion.follow(library(true, first));
  expect(held?.revision).toBe(PROMPT_LIBRARY_REVISION);
  expect(held?.search('x')).toBe(first);
  // The library published again: the same object, answering from the newer catalog.
  expect(completion.follow(library(true, second))).toBe(held);
  expect(held?.search('x')).toBe(second);
  // A scan began: names not found are no longer certain to be missing, which an editor shows.
  const scanning = completion.follow(library(false, second));
  expect(scanning).not.toBe(held);
  expect(scanning?.authoritative).toBe(false);
  expect(completion.follow(null)).toBeNull();
});

test('two resolutions are the same when they show the same models, words and pictures', () => {
  const picture = { status: 'ready', url: 'http://localhost/a.png', width: 4, height: 4 } as const;
  const found: LoraResource = {
    name: 'ink',
    resolution: 'found',
    model: { promptName: 'style/ink', backendValue: 'style/ink.safetensors', basename: 'ink' },
    title: 'Ink',
    subtitle: null,
    triggerWords: ['ink wash'],
    thumbnail: picture,
    modelPageUrl: null,
  };
  const snapshot = (...resources: LoraResource[]) => ({ status: 'ready' as const, resources });
  const missing: LoraResource = { name: 'gone', resolution: 'missing' };
  expect(sameLoraResources(snapshot(found, missing), snapshot({ ...found }, { ...missing }))).toBe(
    true,
  );
  for (const changed of [
    { ...found, title: 'Ink Wash' },
    { ...found, triggerWords: ['ink wash', 'sumi'] },
    { ...found, thumbnail: { ...picture } },
    { name: 'ink', resolution: 'ambiguous', candidateCount: 2 } satisfies LoraResource,
  ])
    expect(sameLoraResources(snapshot(found), snapshot(changed))).toBe(false);
  expect(sameLoraResources(snapshot(found), snapshot(found, missing))).toBe(false);
  expect(sameLoraResources(snapshot(found), { status: 'unavailable', resources: [found] })).toBe(
    false,
  );
});
