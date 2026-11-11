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
/** Characterize Substitute finder parsing, filtering, source ranges, and navigation intent. */
import { expect, test } from '@jest/globals';
import {
  buildSearchQuery,
  parseSearchInput,
} from '../../../frontend/comfyui/ui/search/domain/SearchQuery.js';
import {
  matchSearch,
  findSearchRanges,
  type SearchNode,
} from '../../../frontend/comfyui/ui/search/domain/SearchMatching.js';
import { SearchSession } from '../../../frontend/comfyui/ui/search/application/SearchSession.js';

const nodes: readonly SearchNode[] = [
  {
    id: 'A:ksampler',
    name: 'ksampler',
    type: 'KSampler',
    title: 'Sample',
    fields: [
      {
        id: 'a:prompt',
        key: 'prompt_template',
        label: 'Prompt Template',
        text: 'fox in the forest fox',
      },
      { id: 'a:sampler', key: 'sampler_name', label: 'Sampler', text: 'euler' },
      { id: 'a:seed', key: 'seed', label: 'Seed', text: '42' },
    ],
  },
  {
    id: 'B:positive',
    name: 'positive_prompt',
    type: 'PrimitiveStringMultiline',
    title: 'Positive prompt',
    fields: [{ id: 'b:prompt', key: 'value', label: 'Scale Factor', text: 'fox in the attic' }],
  },
];

test('Text finds every non-overlapping source occurrence in cube, node, field and source order', () => {
  const result = matchSearch(nodes, buildSearchQuery('text', ' FOX '));
  expect(result.matches).toEqual([
    { node: 'A:ksampler', field: 'a:prompt', start: 0, length: 3 },
    { node: 'A:ksampler', field: 'a:prompt', start: 18, length: 3 },
    { node: 'B:positive', field: 'b:prompt', start: 0, length: 3 },
  ]);
  expect(result.nodes.size).toBe(0);
  expect(matchSearch(nodes, buildSearchQuery('text', '42')).matches[0]?.field).toBe('a:seed');
  expect(findSearchRanges('aaaa', 'aa')).toEqual([
    { start: 0, length: 2 },
    { start: 2, length: 2 },
  ]);
});

test('Field matches all tokens against canonical keys and effective labels and derives matching nodes', () => {
  expect([...matchSearch(nodes, buildSearchQuery('field', 'sampler_name')).fields]).toEqual([
    'a:sampler',
  ]);
  expect([...matchSearch(nodes, buildSearchQuery('field', 'factor scale')).fields]).toEqual([
    'b:prompt',
  ]);
  expect([...matchSearch(nodes, buildSearchQuery('field', 'sampler missing')).nodes]).toEqual([]);
  expect(matchSearch(nodes, buildSearchQuery('field', '')).fields.size).toBe(4);
});

test('Node matches names, types, authored titles and field labels without substituting Comfy aliases', () => {
  for (const query of ['ksampler', 'sampler_name', 'prompt template', 'sample'])
    expect([...matchSearch(nodes, buildSearchQuery('node', query)).nodes]).toContain('A:ksampler');
  expect([...matchSearch(nodes, buildSearchQuery('node', 'positive prompt')).nodes]).toEqual([
    'B:positive',
  ]);
  expect(matchSearch(nodes, buildSearchQuery('node', 'input text')).nodes.size).toBe(0);
});

test('quoted Node queries scope value matches and accept unfinished quotes', () => {
  for (const query of ['ksampler "fox"', 'ksampler "fox'])
    expect(matchSearch(nodes, buildSearchQuery('node', query)).matches).toHaveLength(2);
  expect(matchSearch(nodes, buildSearchQuery('node', '"fox"')).matches).toHaveLength(3);
  expect(matchSearch(nodes, buildSearchQuery('node', 'missing "fox"')).matches).toHaveLength(0);
  expect(matchSearch(nodes, buildSearchQuery('node', 'ksampler')).matches).toHaveLength(0);
});

test('context commands switch modes and incomplete commands defer matching', () => {
  expect(parseSearchInput('text', '  @FIELD seed')).toEqual({
    mode: 'field',
    text: 'seed',
    pending: false,
  });
  expect(parseSearchInput('field', '@node ksampler')).toEqual({
    mode: 'node',
    text: 'ksampler',
    pending: false,
  });
  expect(parseSearchInput('node', '@text fox')).toEqual({
    mode: 'text',
    text: 'fox',
    pending: false,
  });
  expect(parseSearchInput('text', '@no').pending).toBe(true);
});

test('literal markup and regex punctuation remain source text and Unicode selections use original offsets', () => {
  expect(findSearchRanges('<img>.*<img>', '<img>')).toEqual([
    { start: 0, length: 5 },
    { start: 7, length: 5 },
  ]);
  expect(findSearchRanges('🙂İ FOX', 'fox')).toEqual([{ start: 4, length: 3 }]);
  expect(findSearchRanges('İ', 'i')).toEqual([{ start: 0, length: 1 }]);
});

test('cached navigation wraps in both directions and keeps the closest same-field match after edits', () => {
  const session = new SearchSession();
  session.setCorpus(nodes);
  session.setInput('text', 'fox');
  const result = session.result;
  expect(session.cycle(-1)?.field).toBe('b:prompt');
  expect(session.cycle(1)?.start).toBe(0);
  expect(session.cycle(1)?.start).toBe(18);
  expect(session.result).toBe(result);
  session.setCorpus([
    {
      ...nodes[0]!,
      fields: [{ id: 'a:prompt', key: 'prompt_template', label: 'Prompt', text: 'fox then fox' }],
    },
  ]);
  expect(session.state.active?.start).toBe(9);
  session.setOpen(true);
  session.setScrollTop(400);
  const restored = new SearchSession(session.state);
  restored.setCorpus(nodes);
  expect(restored.state).toMatchObject({ open: true, mode: 'text', text: 'fox', scrollTop: 400 });
  expect(restored.state.active?.field).toBe('a:prompt');
});

test('pending commands retain results and selectable filtering disables unsupported navigation', () => {
  const session = new SearchSession();
  session.setCorpus(nodes);
  session.setInput('text', 'fox');
  const previous = session.result;
  expect(session.setInput('text', '@n')).toBe(false);
  expect(session.result).toBe(previous);
  session.setCorpus(nodes);
  expect(session.result.query.text).toBe('fox');
  session.selectable(new Set(['b:prompt']));
  expect(session.result.matches).toHaveLength(1);
  session.selectable(new Set());
  expect(session.cycle(1)).toBeNull();
});
