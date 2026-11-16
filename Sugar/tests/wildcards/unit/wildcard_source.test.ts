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
/** Prove wildcard files are read the way SugarSubstitute's Python application read them. */
import {
  csvColumns,
  csvRecords,
  csvWildcard,
  textCandidates,
  wildcardFile,
  wildcardIdentifier,
} from '../../../frontend/comfyui/ui/wildcards/domain/WildcardSource.js';

test('a wildcard is a TXT or CSV file inside the folder, named by its lowercased path', () => {
  expect(wildcardFile('Colors.txt', 's')).toEqual({
    path: 'Colors.txt',
    identifier: 'colors',
    kind: 'text',
    stamp: 's',
  });
  expect(wildcardFile('Characters\\Hair Styles.CSV', 's')).toMatchObject({
    path: 'Characters/Hair Styles.CSV',
    identifier: 'characters/hair styles',
    kind: 'csv',
  });
  for (const path of ['notes.md', '../colors.txt', 'a//b.txt', './a.txt', '.txt', 'C:/a.txt', ''])
    expect(wildcardFile(path, 's')).toBeNull();
  // A prompt may write a name with repeated separators or dot segments, as Python's paths allow.
  expect(wildcardIdentifier('Folder//./Name')).toBe('folder/name');
});

test('a TXT wildcard offers each line that is not blank, without surrounding whitespace', () => {
  expect(textCandidates('red\r\n  green \n\n\t\nblue\rdeep  blue')).toEqual([
    'red',
    'green',
    'blue',
    'deep  blue',
  ]);
  expect(textCandidates('')).toEqual([]);
});

// Each expectation below is what Python's `list(csv.reader(io.StringIO(text, newline='')))` gives.
test.each<[string, string[][]]>([
  [
    'a,b\r\n1,2\r\n',
    [
      ['a', 'b'],
      ['1', '2'],
    ],
  ],
  [
    'a,b\n1,2',
    [
      ['a', 'b'],
      ['1', '2'],
    ],
  ],
  ['a,b\n\n1,2\n', [['a', 'b'], [], ['1', '2']]],
  ['"x, y",z\n', [['x, y', 'z']]],
  ['"say ""hi""",b\n', [['say "hi"', 'b']]],
  ['"line\nbreak",b\n', [['line\nbreak', 'b']]],
  ['a"b,c\n', [['a"b', 'c']]],
  ['"ab"c,d\n', [['abc', 'd']]],
  ['a,\n', [['a', '']]],
  [',\n', [['', '']]],
  ['a\rb\n', [['a'], ['b']]],
  ['"open', [['open']]],
  ['', []],
])('CSV %j splits into the records Python reads', (text, records) => {
  expect(csvRecords(text)).toEqual(records);
});

test('a CSV wildcard names its columns from the first record and offers every later record', () => {
  const source = ' Name , HAIR ,name\nAlice, red \n\nBob\nCara,blue,extra,more\n';
  expect(csvColumns(source)).toEqual(['Name', 'HAIR']);
  const wildcard = csvWildcard(source);
  expect(wildcard.columns).toEqual(['Name', 'HAIR']);
  // A repeated header keeps the last value paired with it, a blank line is an empty candidate
  // and values beyond the headers are dropped, exactly as a Python dict built by zip.
  expect(wildcard.rows.map((row) => ({ ...row }))).toEqual([
    { name: 'Alice', hair: 'red' },
    {},
    { name: 'Bob' },
    { name: 'extra', hair: 'blue' },
  ]);
  expect(csvWildcard('')).toEqual({ columns: [], rows: [] });
  // Only the header is decoded when the columns alone are wanted.
  expect(csvRecords('a,b\n1,2\n3,4\n', 1)).toEqual([['a', 'b']]);
});
