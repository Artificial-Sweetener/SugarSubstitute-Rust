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
/** Independent semantic specimens for source ownership, tolerant syntax and immutable snapshots. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  parsePrompt,
  parseScenes,
  normalizeSceneTitle,
  serializePrompt,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { parseWeight } from '../../../../../../frontend/sugarprompt/core/src/prompt/weights.ts';

test('segments preserve exact owned separators and never invent a trailing segment', () => {
  const source = 'alpha, \t\r\n"beta,gamma", (delta,epsilon:1.20), ';
  const document = parsePrompt(source);
  assert.equal(document.segments.length, 3);
  assert.deepEqual(document.segments[0]?.contentRange, { from: 0, to: 5 });
  assert.deepEqual(document.segments[0]?.separatorRange, { from: 5, to: 10 });
  assert.equal(document.hasTrailingComma, true);
  assert.equal(serializePrompt(document) === source, true);
  assert.equal(parsePrompt('').segments.length, 0);
  assert.equal(parsePrompt(',,').segments.length, 2);
});

test('weighted syntax retains source ranges, precision, same-kind nesting and second LoRA weights', () => {
  const source = '🙂, ((cat:1.20):1.10), <lora:folder/model: +0.80:0.60:block>';
  const document = parsePrompt(source);
  assert.deepEqual(
    document.emphasisSpans.map((span) => span.depth),
    [0, 1],
  );
  assert.deepEqual(document.emphasisSpans[1]?.contentRange, { from: 6, to: 9 });
  const lora = document.loraSpans[0]!;
  assert.equal(lora.firstWeight === '0.80' && lora.secondWeight === '0.60', true);
  assert.equal(source.slice(lora.nameRange.from, lora.nameRange.to) === 'folder/model', true);
  assert.equal(
    source.slice(lora.firstWeightRange.from, lora.firstWeightRange.to) === '+0.80',
    true,
  );
  assert.equal(
    source.slice(lora.blockWeightsRange!.from, lora.blockWeightsRange!.to) === 'block',
    true,
  );
  assert.equal(parsePrompt('(cat:+1), <lora:model:bad>, {csv:a}').syntaxSpans.length, 0);
});

test('decimal spelling preserves arbitrary precision, signed zero and Unicode numeric fields', () => {
  for (const [source, expected] of [
    ['-000.00', '-0.00'],
    ['+.5', '0.5'],
    ['1.', '1'],
    ['.000000100', '1.00E-7'],
    ['-0.0000000', '-0E-7'],
    ['１２.３０', '12.30'],
    ['١.٢', '1.2'],
    ['999999999999999999999', '999999999999999999999'],
  ])
    assert.equal(parseWeight(source!, true) === expected, true);
  for (const source of [' 1', '1 ', '1e2', 'NaN', '∞', '²', '＋1', ''])
    assert.equal(parseWeight(source, true), null);
});

test('wildcard wrappers validate configuration while malformed content remains ordinary source', () => {
  const document = parsePrompt('%%animal%%, {animal}', {
    style: 'custom',
    prefix: '%%',
    suffix: '%%',
    alsoRecognizeCurly: false,
  });
  assert.equal(document.wildcardSpans.length, 1);
  assert.deepEqual(document.wildcardSpans[0]?.outerRange, { from: 0, to: 10 });
  for (const [prefix, suffix] of [
    ['', '%%'],
    ['letters', '%%'],
    ['{', '}'],
    ['<', '>'],
    ['(', ')'],
    ['/', '/'],
    ['\x1c', '%%'],
    ['012345678!', '%%'],
  ])
    assert.throws(
      () =>
        parsePrompt('', {
          style: 'custom',
          prefix: prefix!,
          suffix: suffix!,
          alsoRecognizeCurly: true,
        }),
      RangeError,
    );
});

test('regions preserve exact names and empty partitions while recovering from malformed brackets', () => {
  const document = parsePrompt('[SEP]\r\n[SEP| Foreground ✨ ]\nopen[\r[SEP]');
  assert.equal(document.regions.separators.length, 3);
  assert.deepEqual(
    document.regions.partitions.map((part) => part.sourceRange),
    [
      { from: 0, to: 0 },
      { from: 7, to: 7 },
      { from: 28, to: 34 },
      { from: 39, to: 39 },
    ],
  );
  assert.equal(document.regions.separators[1]?.name === ' Foreground ✨ ', true);
});

test('semantic trees are deeply frozen and scene identity retains reference Unicode behavior', () => {
  const document = parsePrompt('(cat:1.20), {animal}');
  assert.equal(Object.isFrozen(document), true);
  assert.equal(Object.isFrozen(document.emphasisSpans[0]?.outerRange), true);
  assert.equal(Object.isFrozen(document.regions.partitions), true);
  const scenes = parseScenes('global\n** Straße\nfirst\n** STRASSE\nsecond');
  assert.equal(scenes.universalText === 'global\n', true);
  assert.equal(scenes.scenes[1]?.marker.duplicate, true);
  assert.equal(normalizeSceneTitle(' \tΣςσ\u0085Straße\ufeff ') === 'σσσ strasse\ufeff', true);
  assert.equal(Object.isFrozen(scenes.scenes[0]?.marker), true);
});
