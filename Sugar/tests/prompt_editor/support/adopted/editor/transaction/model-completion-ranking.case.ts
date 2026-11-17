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
/** The real host catalog ranks model candidates by match tier, then by friendly name and path, with no cap. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { modelCatalog, modelResult, type ModelRow } from './model-completion-fixture.ts';

/** Ranked prompt names and scores for one query. */
function ranked(rows: readonly ModelRow[], query: string) {
  return modelCatalog(rows)
    .search(query)
    .map((row) => [row.item.model.promptName, row.score] as const);
}

test('an exact prompt name, extensionless file, friendly title or file name ranks first', () => {
  const rows: ModelRow[] = [
    {
      promptName: 'folder/Alpha',
      backendValue: 'folder/alpha-file.safetensors',
      basename: 'alpha-file',
      title: 'Friendly',
    },
  ];
  for (const query of [
    'folder/alpha',
    'FOLDER\\ALPHA',
    'folder/alpha-file',
    'friendly',
    'Alpha-File',
  ])
    assert.deepEqual(ranked(rows, query), [['folder/Alpha', 0]], query);
});

test('prefix, substring and metadata tiers order candidates; unmatched models are excluded', () => {
  const rows: ModelRow[] = [
    { promptName: 'p/none', title: 'Unrelated' },
    { promptName: 'p/meta', title: 'Meta', triggerWords: ['abstract'] },
    { promptName: 'ab-folder/n2', basename: 'n2', title: 'N2' },
    { promptName: 'p/n1', title: 'The Abacus' },
    { promptName: 'p/xaby', title: 'Q1' },
    { promptName: 'p/xyz', title: 'Abby' },
    { promptName: 'p/abc', title: 'Zed' },
  ];
  assert.deepEqual(ranked(rows, 'ab'), [
    ['p/abc', 100],
    ['p/xyz', 110],
    ['p/xaby', 300],
    ['p/n1', 310],
    // A prompt-name prefix counts as a path prefix only when the query names a folder.
    ['ab-folder/n2', 330],
    ['p/meta', 700],
  ]);
  const paths: ModelRow[] = [{ promptName: 'dir/abq', basename: 'abq', title: 'Q' }];
  assert.deepEqual(ranked(paths, 'dir/ab'), [['dir/abq', 120]]);
  assert.deepEqual(ranked(paths, 'dir\\ab'), [['dir/abq', 120]]);
  assert.deepEqual(ranked(paths, 'dir'), [['dir/abq', 330]]);
});

test('equal scores order by case-folded friendly name, then by case-folded path', () => {
  const rows: ModelRow[] = [
    { promptName: 'z/beta', title: 'beta' },
    { promptName: 'z/alpha', title: 'Alpha' },
    { promptName: 'a/alpha', title: 'alpha' },
  ];
  assert.deepEqual(ranked(rows, ''), [
    ['a/alpha', 900],
    ['z/alpha', 900],
    ['z/beta', 900],
  ]);
});

test('every match is offered; the candidate list is never truncated', () => {
  const rows = Array.from({ length: 30 }, (_, index) => ({ promptName: `m/mood${index}` }));
  assert.equal(ranked(rows, 'mood').length, 30);
  const result = modelResult('<lora:mood', undefined, { rows });
  assert.equal(result?.options.length, 30);
});

test('the ghost completes the friendly name, else a typed path, and shows the whole name for an empty query', () => {
  const rows: ModelRow[] = [{ promptName: 'folder/mid_night', title: 'Mid Night' }];
  for (const [source, ghost] of [
    ['<lora:', 'Mid Night'],
    ['<lora:mid', ' Night'],
    ['<lora:MID_n', 'ight'],
    ['<lora:folder\\mid', '_night'],
    ['<lora:night', ''],
  ] as const) {
    const result = modelResult(source, undefined, { rows });
    assert.equal(result?.surface, 'models', source);
    assert.equal(result.options[0]!.ghost, ghost, source);
  }
});

test('no ghost is shown while more of the name follows the caret', () => {
  const rows: ModelRow[] = [{ promptName: 'folder/mid_night', title: 'Mid Night' }];
  for (const source of ['<lora:mix>', '<lora:mid:1.0>', '<lora:mix']) {
    const result = modelResult(source, '<lora:mi'.length, { rows });
    assert.equal(result?.surface, 'models', source);
    assert.equal(result.options[0]!.ghost, '', source);
  }
  // At the end of the name the ghost returns, whatever follows the name field.
  const closed = modelResult('<lora:mid>', '<lora:mid'.length, { rows });
  assert.equal(closed?.options[0]?.ghost, ' Night');
});
