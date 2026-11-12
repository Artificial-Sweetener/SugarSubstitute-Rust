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
/** Independent prose and prompt specimens prove actual canonical scope filtering and populated reuse. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { prepareDocumentValues } from '../../../../../../frontend/sugarprompt/core/src/document/values.ts';
import { prepareDocumentPromptScopes } from '../../../../../../frontend/sugarprompt/core/src/document/prompt-scopes.ts';
import { prepareTagLexicon } from '../../../../../../frontend/sugarprompt/core/src/normalization/lexicon.ts';
import { prepareSpellcheckCandidates } from '../../../../../../frontend/sugarprompt/core/src/diagnostics/spellcheck-candidates.ts';

/** Consume the sole canonical scope preparation with exact candidate storage owned elsewhere. */
function scopes(source: string, kind: 'prompt' | 'wildcard-txt' | 'wildcard-csv' = 'prompt') {
  return prepareDocumentPromptScopes(prepareDocumentValues(source, kind), {
    style: 'curly',
  }).scopes;
}

test('plain prose retains exact apostrophe words and decoded UTF16 after nonBMP prefixes', () => {
  const scope = scopes("🦊 soft light, don't rush")[0]!;
  const output = prepareSpellcheckCandidates(scope);
  assert.deepEqual(output.candidates, [
    { range: { from: 3, to: 7 }, text: 'soft' },
    { range: { from: 8, to: 13 }, text: 'light' },
    { range: { from: 15, to: 20 }, text: "don't" },
    { range: { from: 21, to: 25 }, text: 'rush' },
  ]);
  assert.equal(output.scope, scope);
  assert.equal(Object.isFrozen(output.candidates[0]!.range), true);
  assert.equal(prepareSpellcheckCandidates(scope, undefined, output), output);
});

test('complete syntax, known tags and machine segments never become backend candidates', () => {
  const scope = scopes(
    '(soft light:1.2), {library}, <lora:model:1>, known_phrase, blue sky, artist, soft prose, ModelName, model², model.ckpt, https://site.test, C:/folder/file',
  )[0]!;
  const lexicon = prepareTagLexicon(['blue_sky', 'artist']);
  const output = prepareSpellcheckCandidates(scope, lexicon);
  assert.deepEqual(
    output.candidates.map((value) => value.text),
    ['soft', 'prose'],
  );
  assert.equal(prepareSpellcheckCandidates(scope, lexicon, output), output);
  const next = prepareSpellcheckCandidates(scope, prepareTagLexicon([]), output);
  assert.notEqual(next, output);
  assert.deepEqual(
    next.candidates.map((value) => value.text),
    ['blue', 'sky', 'artist', 'soft', 'prose'],
  );
});

test('actual CSV header exclusion and quoted decoding preserve independent candidate ranges across cells', () => {
  const actual = scopes(`first,second\r\n"soft light","don't rush"\r\n`, 'wildcard-csv');
  const first = prepareSpellcheckCandidates(actual[0]!),
    second = prepareSpellcheckCandidates(actual[1]!);
  assert.equal(actual.length, 2);
  assert.deepEqual(
    first.candidates.map((value) => value.text),
    ['soft', 'light'],
  );
  assert.deepEqual(second.candidates, [
    { range: { from: 0, to: 5 }, text: "don't" },
    { range: { from: 6, to: 10 }, text: 'rush' },
  ]);
  assert.notEqual(prepareSpellcheckCandidates(actual[1]!, undefined, first), first);
  assert.deepEqual(
    prepareSpellcheckCandidates(actual[1]!, undefined, first).candidates,
    second.candidates,
  );
});

test('pinned digit metadata excludes superscripts and decimal numbers while Roman/fraction token text remains prose', () => {
  const output = prepareSpellcheckCandidates(scopes('model², model٣, softⅧ, soft½')[0]!);
  assert.deepEqual(
    output.candidates.map((value) => value.text),
    ['soft', 'soft'],
  );
  const error = new Error('lexicon-boundary');
  assert.throws(
    () =>
      prepareSpellcheckCandidates(scopes('soft prose')[0]!, {
        contains() {
          throw error;
        },
      }),
    (value) => value === error,
  );
});

test('prepared syntax never invokes tag knowledge and shared tokens preserve exact independent prose ranges', () => {
  const error = new Error('unused-lexicon');
  const blocked = prepareSpellcheckCandidates(scopes('(soft:1.2), {library}, <lora:model:1>')[0]!, {
    contains() {
      throw error;
    },
  });
  assert.deepEqual(blocked.candidates, []);
  const output = prepareSpellcheckCandidates(scopes('soft-light-gentle, plain words')[0]!);
  assert.deepEqual(
    output.candidates.map((value) => value.text),
    ['soft', 'light', 'gentle', 'plain', 'words'],
  );
});
