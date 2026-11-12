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
/** Independent source-query and acceptance contracts keep rendering separate from editable source. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  mapVisibleSource,
  parsePrompt,
  prepareTagQueries,
  tagReplacement,
  tagGhostText,
  tagIsNoop,
  completionSuffix,
  suffixWithoutRightText,
  type TagQuery,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';

/** Resolve one declared synthetic source caret through the real preparation owner. */
function query(source: string, caret = source.length): TagQuery {
  const resolved = prepareTagQueries(parsePrompt(source)).queryAt(caret, false);
  assert.ok(resolved);
  return resolved;
}

test('Unicode separators remain outside tag and emphasis replacement ranges and fallback words', () => {
  for (const separator of [' ', '\t', '\u00a0', '\u2009', '\u202f', '\u3000']) {
    for (const prefix of ['cat,', '(cat:1.20),']) {
      const source = prefix + separator + 'soft' + separator + 'li' + separator;
      const cursor = source.length - separator.length;
      const active = query(source, cursor);
      assert.equal(active.replacementRange.from, prefix.length + separator.length);
      assert.equal(active.activeTagEnd, cursor);
      assert.equal(active.fallback?.prefix, 'li');
      assert.equal(active.fallback?.replacementRange.from, cursor - 2);
      const replacement = tagReplacement(source, active, 'soft_light', true);
      assert.equal(
        source.slice(0, replacement.range.from) +
          replacement.text +
          source.slice(replacement.range.to),
        prefix + separator + 'soft light, ' + separator,
      );
    }
    const emphasis = '(cat:1.20), (' + separator + 'soft li' + separator + ':1.10)';
    const cursor = emphasis.indexOf('li') + 2;
    assert.equal(query(emphasis, cursor).prefix, 'soft li');
    assert.equal(query(emphasis, cursor).activeTagEnd, cursor);
  }
});

test('prepared query selects bounded lines, innermost emphasis and mapped last-word fallback', () => {
  const source = '🙂, \r\n (outer (soft li:1.20) tail:0.80), last';
  const caret = source.indexOf('li') + 2;
  const resolved = query(source, caret);
  assert.equal(resolved.prefix, 'soft li');
  assert.deepEqual(resolved.replacementRange, {
    from: source.indexOf('soft'),
    to: caret,
  });
  assert.equal(resolved.activeTagEnd, caret);
  assert.equal(resolved.fallback?.prefix, 'li');
  assert.equal(resolved.fallback?.replacementRange.from, caret - 2);
  const prepared = prepareTagQueries(parsePrompt(source));
  assert.equal(prepared.queryAt(source.indexOf('1.20') + 2, false), null);
  assert.equal(prepared.queryAt(caret, true), null);
  assert.equal(prepared.queryAt(caret, false, 8), null);
  assert.equal(query('earlier\r\n  soft li\t', 18).prefix, 'soft li');
});

test('literal escape mapping uses scalar boundaries and rejects caret inside a stored escape', () => {
  const source = '🙂a \\(real\\) tail';
  const visible = mapVisibleSource(source, 5);
  assert.equal(visible.text, '🙂a (real) tail');
  assert.deepEqual(visible.sourcePositions.slice(0, 5), [5, 7, 8, 9, 11]);
  assert.deepEqual(visible.displayPositions.slice(0, 5), [0, 2, 3, 4, 5]);
  const prepared = prepareTagQueries(parsePrompt(source));
  assert.equal(prepared.queryAt(source.indexOf('\\(') + 1, false), null);
  assert.equal(query(source, source.indexOf('real') + 4).prefix, '🙂a (real');
});

test('completion displays only the new middle suffix and acceptance consumes a compatible existing right tail once', () => {
  const source = 'soft ght\t ';
  const active = query(source, 5);
  assert.equal(tagGhostText(source, active, 'soft_light'), 'li');
  assert.deepEqual(tagReplacement(source, active, 'soft_light', true), {
    range: { from: 0, to: 8 },
    text: 'soft light, ',
  });
  assert.equal(source.slice(8), '\t ');
  const unmatched = 'soft tail';
  assert.deepEqual(tagReplacement(unmatched, query(unmatched, 5), 'soft_light', false), {
    range: { from: 0, to: 5 },
    text: 'soft light',
  });
  assert.equal(tagGhostText(unmatched, query(unmatched, 5), 'soft_light'), 'light');
  assert.deepEqual(tagReplacement('re', query('re'), 'real_(version)', true), {
    range: { from: 0, to: 2 },
    text: 'real \\(version\\), ',
  });
});

test('catalog identity can ignore case while inline suffix compatibility preserves scalar spelling', () => {
  assert.equal(completionSuffix('Straße', 'STRA'), '');
  assert.equal(completionSuffix('🙂 light', '🙂_'), 'light');
  assert.equal(suffixWithoutRightText('🙂tail', 'tail\t '), '🙂');
  assert.equal(suffixWithoutRightText('tail', 'tail\u00a0'), 'tail');
  const source = 'STRASSE \\(real\\)';
  assert.equal(tagIsNoop(source, query(source), 'straße_(real)'), true);
  assert.equal(tagGhostText(source, query(source, 4), 'straße_(real)'), '');
});

test('editable completion boundaries reject CRLF/surrogate interiors and malformed source command ranges', () => {
  const source = '🙂\r\nsoft',
    prepared = prepareTagQueries(parsePrompt(source));
  for (const position of [-1, 0.5, 1, 3, 9])
    assert.throws(() => prepared.queryAt(position, false), RangeError);
  for (const minimum of [0, -1, 1.5])
    assert.throws(() => prepared.queryAt(8, false, minimum), RangeError);
  const invalid: TagQuery = {
    prefix: 'x',
    replacementRange: { from: 4, to: 2 },
    activeTagEnd: 8,
    fallback: null,
  };
  assert.throws(() => tagReplacement(source, invalid, 'tag', true), RangeError);
});
