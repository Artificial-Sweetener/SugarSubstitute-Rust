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
/** Canonical structured scopes and full serialized witnesses prove one-pass batch admission without another grammar or cache. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { prepareDocumentValues } from '../../../../../../../frontend/sugarprompt/core/src/document/values.ts';
import {
  prepareDocumentPromptScopes,
  prepareDocumentValueReorders,
  prepareValueReorder,
  serializeValueReorder,
  type PreparedDocumentPromptScopes,
  type PromptDocumentKind,
} from '../../../../../../../frontend/sugarprompt/core/src/index.ts';

/** The actual document-value and prompt preparation owners produce every admitted immutable scope. */
function scopes(source: string, kind: PromptDocumentKind): PreparedDocumentPromptScopes {
  return prepareDocumentPromptScopes(prepareDocumentValues(source, kind), {
    style: 'curly',
  });
}

for (const specimen of [
  {
    kind: 'prompt',
    source: ' alpha, 🧭, (a,b:1), <lora:x:1>\r\n[SEP]\n tail, ',
  },
  {
    kind: 'wildcard-txt',
    source: '🧭, (a,b:1)\r\n\r\n <lora:x:1>, tail \nneighbor, value',
  },
  {
    kind: 'wildcard-csv',
    source: 'name,prompt\r\nrow,"🧭, (a,b:1), <lora:x:1>"\r\nother,"neighbor, value"\r\n',
  },
  {
    kind: 'wildcard-csv',
    source: 'name,prompt\nrow,"quoted ""literal"", value"\nempty,\n',
  },
] satisfies readonly {
  readonly kind: PromptDocumentKind;
  readonly source: string;
}[])
  test(`complete ${specimen.kind} batch preserves canonical ordering and every actual single serialized field`, () => {
    const prepared = scopes(specimen.source, specimen.kind),
      before = JSON.stringify(prepared);
    const result = prepareDocumentValueReorders(prepared);
    assert.ok(result);
    assert.equal(result.length, prepared.scopes.length);
    assert.equal(Object.isFrozen(result), true);
    for (const [index, batch] of result.entries()) {
      const scope = prepared.scopes[index]!;
      const single = prepareValueReorder(prepared, scope);
      assert.ok(single);
      assert.equal(batch.scope, scope);
      assert.deepEqual(batch, single);
      const actual = serializeValueReorder(batch),
        independent = serializeValueReorder(single);
      assert.ok(actual && independent);
      assert.deepEqual(actual, independent);
      assert.equal(actual.text, scope.document.sourceText);
      assert.equal(prepareValueReorder(prepared, scope, batch), batch);
      assert.equal(Object.isFrozen(batch.chips), true);
      assert.equal(Object.isFrozen(batch.initial.slots), true);
    }
    assert.equal(JSON.stringify(prepared), before);
  });

test('actual populated CSV batch witnesses retain decoded chips, neighboring value identity and header exclusion', () => {
  const prepared = scopes(
    'name,prompt\r\nrow,"🧭, (a,b:1), <lora:x:1>"\r\nother,"neighbor, value"\r\n',
    'wildcard-csv',
  );
  const result = prepareDocumentValueReorders(prepared);
  assert.ok(result);
  assert.equal(result.length, 4);
  assert.deepEqual(
    result.map((value) => value.scope.id),
    ['csv-container:1:0', 'csv-container:1:1', 'csv-container:2:0', 'csv-container:2:1'],
  );
  assert.equal(prepared.byId['csv-container:0:1'], undefined);
  assert.deepEqual(
    result[1]!.chips.map((chip) => chip.text),
    ['🧭', 'a', 'b', '<lora:x:1>'],
  );
  assert.deepEqual(
    result[3]!.chips.map((chip) => chip.text),
    ['neighbor', 'value'],
  );
  assert.equal(serializeValueReorder(result[1]!)!.text, '🧭, (a,b:1), <lora:x:1>');
  assert.equal(serializeValueReorder(result[3]!)!.text, 'neighbor, value');
});

test('inconsistent missing, foreign, duplicate and extra indexes reject while original single-call membership and reuse guards remain exact', () => {
  const prepared = scopes('one, two\nthree, four', 'wildcard-txt'),
    foreign = scopes('other', 'wildcard-txt');
  const [first, second] = prepared.scopes;
  assert.ok(first && second);
  const plan = prepareValueReorder(prepared, first);
  assert.ok(plan);
  assert.equal(prepareValueReorder(prepared, first, plan), plan);
  assert.equal(prepareValueReorder(prepared, { ...first }, plan), null);
  assert.equal(prepareValueReorder({ ...prepared, scopes: [second] }, first, plan), null);
  assert.equal(
    prepareValueReorder(
      {
        ...prepared,
        byId: { ...prepared.byId, [first.id]: foreign.scopes[0]! },
      },
      first,
      plan,
    ),
    null,
  );
  const sparse = [...prepared.scopes];
  // eslint-disable-next-line @typescript-eslint/no-array-delete -- Deliberately supply a sparse array to prove fail-closed input validation.
  delete sparse[0];
  for (const changed of [
    { ...prepared, scopes: sparse },
    { ...prepared, byId: { [first.id]: first } },
    { ...prepared, byId: { ...prepared.byId, [first.id]: foreign.scopes[0]! } },
    { ...prepared, scopes: [first, first] },
    { ...prepared, byId: { ...prepared.byId, extra: foreign.scopes[0]! } },
    { ...prepared, scopes: [{ ...first }, second] },
  ])
    assert.equal(prepareDocumentValueReorders(changed), null);
});

test('batch walks the actual roster once and never repeats single-scope includes admission', () => {
  const prepared = scopes('one, two\nthree, four\nfive, six', 'wildcard-txt');
  let iterations = 0,
    visits = 0;
  const roster = new Proxy(prepared.scopes, {
    /** Literal observation of roster traversal detects per-scope membership scans without reproducing derivation. */
    get(target, property, receiver) {
      if (property === 'includes') assert.fail('BATCH_REPEATED_SINGLE_SCOPE_MEMBERSHIP');
      if (property === Symbol.iterator)
        return function* () {
          iterations++;
          for (const scope of target) {
            visits++;
            yield scope;
          }
        };
      return Reflect.get(target, property, receiver);
    },
  });
  const result = prepareDocumentValueReorders({ ...prepared, scopes: roster });
  assert.ok(result);
  assert.equal(iterations, 1);
  assert.equal(visits, prepared.scopes.length);
  assert.deepEqual(
    result.map((value) => serializeValueReorder(value)!.text),
    ['one, two', 'three, four', 'five, six'],
  );
});

test('valid header-only and malformed document rosters remain immutable empty results', () => {
  for (const source of ['name,prompt\r\n', 'name,prompt\nrow,"unfinished']) {
    const prepared = scopes(source, 'wildcard-csv'),
      result = prepareDocumentValueReorders(prepared);
    assert.ok(result);
    assert.equal(result.length, prepared.scopes.length);
    assert.deepEqual(result, []);
    assert.equal(Object.isFrozen(result), true);
  }
});
