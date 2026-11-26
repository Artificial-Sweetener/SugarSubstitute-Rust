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
/** Prove the complete committed dataset and its native ranking and normalization contract. */
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import {
  bundledPromptTags,
  preparePromptTagCatalog,
} from '../../../frontend/comfyui/ui/prompt_editor/runtime/PromptTagCatalog.js';

const source = readFileSync('frontend/comfyui/ui/prompt_editor/assets/danbooru-tags.txt', 'utf8');

test('ships the complete pinned Substitute tag snapshot with ordinary and rare tag metadata', () => {
  expect(createHash('sha256').update(source).digest('hex')).toBe(
    '38a5b8d1ff959d927d47423191575ea1281ee0c5e1a77e5773967ca1dfab5f82',
  );
  expect(source.trimEnd().split('\n')).toHaveLength(157516);
  const catalog = preparePromptTagCatalog(bundledPromptTags(source), 'bundled-proof', [], []);
  expect(catalog.search('blue_ey')[0]?.tag).toBe('blue eyes');
  expect(catalog.search('long hai')[0]?.tag).toBe('long hair');
  expect(catalog.lexicon.contains('BLUE_EYES')).toBe(true);
  expect(catalog.lexicon.contains('not a catalog tag at all')).toBe(false);
  expect(catalog.search('blue_ey')).toBe(catalog.search('BLUE EY'));
  const last = source.trimEnd().split('\n').at(-1)?.split(',')[0];
  expect(last).toBeDefined();
  expect(catalog.lexicon.contains(last ?? '')).toBe(true);
});

test('preserves popularity, underscore identity, zero counts and stable duplicate ranking', () => {
  const catalog = preparePromptTagCatalog(
    bundledPromptTags(
      'soft_light,7 (danbooru:General)\r\nsoft_lighting,3\nsoft_light,2\nsoft_lights,0\n',
    ),
    'small',
    [],
    [],
  );
  expect(catalog.search('SOFT L')).toEqual([
    { tag: 'soft light', popularity: 7, sourceLabel: null },
    { tag: 'soft lighting', popularity: 3, sourceLabel: null },
    { tag: 'soft lights', popularity: 0, sourceLabel: null },
  ]);
});

test.each([
  '',
  '\n ',
  'tag,not-a-number',
  'tag,-1',
  ',12',
  'tag,9007199254740992',
  'tag,12junk',
  'tag,1\nbroken',
])('rejects damaged bundled data: %j', (text) => {
  expect(() => bundledPromptTags(text)).toThrow(TypeError);
});

test('custom list tags come first and are known words; censored tags are neither suggested nor known', () => {
  const catalog = preparePromptTagCatalog(
    bundledPromptTags('soft_light,7\nsoft_lighting,3\nsoft_focus,2\n'),
    'lists',
    ['Soft_Glow', 'soft light'],
    ['soft_lighting'],
  );
  expect(catalog.search('soft')).toEqual([
    { tag: 'Soft Glow', popularity: null, sourceLabel: 'custom' },
    { tag: 'soft light', popularity: null, sourceLabel: 'custom' },
    { tag: 'soft focus', popularity: 2, sourceLabel: null },
  ]);
  expect(catalog.lexicon.contains('soft glow')).toBe(true);
  expect(catalog.lexicon.containsWord('glow')).toBe(true);
  expect(catalog.lexicon.contains('soft lighting')).toBe(false);
  expect(catalog.lexicon.containsWord('lighting')).toBe(false);
});
