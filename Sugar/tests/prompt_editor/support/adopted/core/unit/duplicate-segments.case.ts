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
/** Independent canonical specimens qualify duplicate scope inheritance and protected grammar. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { prepareDocumentValues } from '../../../../../../frontend/sugarprompt/core/src/document/values.ts';
import { prepareDocumentPromptScopes } from '../../../../../../frontend/sugarprompt/core/src/document/prompt-scopes.ts';
import { normalizeTagIdentity } from '../../../../../../frontend/sugarprompt/core/src/normalization/lexicon.ts';
import { parsePrompt } from '../../../../../../frontend/sugarprompt/core/src/prompt/parser.ts';
import { parseScenes } from '../../../../../../frontend/sugarprompt/core/src/prompt/scenes.ts';
import { ConditioningTopology } from '../../../../../../frontend/sugarprompt/core/src/prompt/conditioning.ts';
import { prepareSceneContexts } from '../../../../../../frontend/sugarprompt/core/src/scenes/context.ts';
import { prepareDuplicateSegments } from '../../../../../../frontend/sugarprompt/core/src/diagnostics/duplicate-segments.ts';

/** Use the actual source parser and actual topology owner; no fixture grammar or payload construction. */
function prepare(source: string, mode: 'unresolved' | 'independent' | 'regional' = 'independent') {
  const document = parsePrompt(source);
  return prepareDuplicateSegments(document, {
    kind: 'conditioning',
    topology: new ConditioningTopology(mode, document.regions),
  });
}

test('duplicate keys share pinned Unicode folding, underscores and Python whitespace', () => {
  assert.equal(normalizeTagIdentity('  STRAẞE_\u0085 Hat\u001c'), 'strasse hat');
  const prepared = prepare('🦊 red_hat, 🦊 RED hat, (🦊 red hat:1.20)');
  assert.equal(prepared.findings.length, 2);
  assert.deepEqual(
    prepared.findings.map((f) => f.duplicate.range),
    [
      { from: 12, to: 22 },
      { from: 24, to: 41 },
    ],
  );
  assert.equal(prepared.findings[0]!.first, prepared.findings[1]!.first);
  assert.equal(prepared.findings[1]!.duplicate.weight, '1.20');
  assert.equal(Object.isFrozen(prepared.findings[0]!.first.range), true);
});

test('actual regional topology inherits global occurrences while independent siblings remain isolated', () => {
  const source = 'cat\r\n[SEP]\r\ncat, dog\n[SEP]\rcat, dog';
  assert.equal(prepare(source).findings.length, 0);
  assert.deepEqual(
    prepare(source, 'regional').findings.map((f) => f.duplicate.key),
    ['cat', 'cat'],
  );
  assert.equal(
    prepare(source, 'regional').findings.every((f) => f.first.range.from === 0),
    true,
  );
});

test('actual scene contexts inherit universal source and fork each scene without heading diagnostics', () => {
  const source = 'cat\n** A\ncat, dog\n** B\ncat, dog';
  const document = parsePrompt(source);
  const prepared = prepareDuplicateSegments(document, {
    kind: 'scenes',
    contexts: prepareSceneContexts(parseScenes(source)),
  });
  assert.deepEqual(
    prepared.findings.map((f) => f.duplicate.key),
    ['cat', 'cat'],
  );
  assert.equal(
    prepared.findings.every((f) => f.first.range.from === 0),
    true,
  );
});

test('protected comma/newline grammar and machine syntax do not create partial duplicate warnings', () => {
  assert.equal(prepare('"cat, dog", cat').findings.length, 0);
  assert.equal(prepare('"cat\ncat", cat').findings.length, 0);
  assert.equal(
    prepare('{cat}, {cat}, <lora:x:1>, <lora:x:1>, a/b, a/b, abcdef12, abcdef12').findings.length,
    0,
  );
  assert.equal(prepare('(cat), cat').findings.length, 0);
  assert.equal(prepare('cat\r\ncat\rcat\ncat').findings.length, 3);
});

test('actual decoded CSV candidates isolate whole-value duplicate groups across internal regions and protected headers', () => {
  const values = prepareDocumentValues(
    'first,second\r\n"cat\n[SEP]\ncat","cat"\r\n',
    'wildcard-csv',
  );
  const scopes = prepareDocumentPromptScopes(values, { style: 'curly' });
  const outputs = scopes.scopes.map((scope) =>
    prepareDuplicateSegments(scope.document, { kind: 'value' }),
  );
  assert.equal(outputs.length, 2);
  assert.deepEqual(
    outputs.map((output) => output.findings.length),
    [1, 0],
  );
  assert.equal(outputs[0]!.findings[0]!.first.range.from, 0);
  assert.equal(outputs[0]!.findings[0]!.duplicate.range.from, 10);
  assert.equal(outputs[0]!.document, scopes.scopes[0]!.document);
  const whole = parsePrompt('"cat, dog", "cat, dog"');
  const finding = prepareDuplicateSegments(whole, { kind: 'value' }).findings[0]!;
  assert.equal(finding.duplicate.key, '"cat, dog"');
  assert.deepEqual(finding.duplicate.range, { from: 12, to: 22 });
});

test('one actual parser stream handles thousands of syntax/scope inputs and reuses populated current findings', () => {
  const document = parsePrompt(
    Array.from(
      { length: 4000 },
      (_, index) => `(unique${index}:1.20), <lora:model${index}:1>`,
    ).join(', '),
  );
  const ownership = Object.freeze({ kind: 'value' as const });
  const syntax = prepareDuplicateSegments(document, ownership);
  assert.equal(syntax.document, document);
  assert.equal(syntax.findings.length, 0);
  assert.equal(document.emphasisSpans.length, 4000);
  assert.equal(document.loraSpans.length, 4000);
  const source =
    'cat\n' +
    Array.from(
      { length: 4000 },
      (_, index) => `** Scene${index}\ncat, unique${index}, unique${index}`,
    ).join('\n');
  const sceneDocument = parsePrompt(source),
    contexts = prepareSceneContexts(parseScenes(source));
  const sceneOwnership = Object.freeze({ kind: 'scenes' as const, contexts });
  const prepared = prepareDuplicateSegments(sceneDocument, sceneOwnership);
  assert.equal(prepared.findings.length, 8000);
  assert.equal(prepared.findings[0]!.first, prepared.findings.at(-2)!.first);
  assert.notEqual(prepared.findings[1]!.first, prepared.findings.at(-1)!.first);
  assert.equal(prepareDuplicateSegments(sceneDocument, sceneOwnership, prepared), prepared);
  assert.equal(
    prepareDuplicateSegments(sceneDocument, sceneOwnership, prepared).findings,
    prepared.findings,
  );
  assert.equal(prepared.document, sceneDocument);
});
