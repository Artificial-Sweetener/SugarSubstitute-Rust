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
/** Actual search owners preserve newer intent while capturing ordinary host accessor objects once. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { reactive, isReactive } from 'vue';
import type { SearchContext } from '../../../../../../frontend/sugarprompt/services/src/search/contracts.ts';
import { searchOwnerFixture, searchPublication, searchNavigation } from './search-owner-fixture.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

for (const key of ['fieldId', 'queryId'] as const)
  test(`a reactive ${key} capture preserves newer issued context before native retirement`, () => {
    const fixture = searchOwnerFixture(),
      owner = fixture.owner;
    let newer: ReturnType<typeof owner.setContext> = null,
      reads = 0;
    const context = reactive({
      fieldId: 'field',
      queryId: 'older',
      get [key](): string {
        reads++;
        newer = owner.setContext({ fieldId: 'field', queryId: 'newer' });
        return key === 'fieldId' ? 'field' : 'older';
      },
    });
    assert.ok(isReactive(context));
    assert.equal(owner.setContext(context), newer);
    assert.ok(newer);
    assert.equal(owner.snapshot().request, newer);
    assert.equal(reads, 1);
    assert.equal(readExactSource(fixture.state) === 'alpha', true);
    assert.equal(undoDepth(fixture.state), 0);
    owner.dispose();
  });

test('context values are captured once rather than revisited during service request construction', () => {
  const fixture = searchOwnerFixture(),
    owner = fixture.owner;
  let fields = 0,
    queries = 0;
  const context: SearchContext = {
    get fieldId() {
      fields++;
      return 'field';
    },
    get queryId() {
      queries++;
      return 'query';
    },
  };
  const request = owner.setContext(context)!;
  assert.equal(request.fieldId, 'field');
  assert.equal(request.queryId, 'query');
  assert.deepEqual([fields, queries], [1, 1]);
  owner.dispose();
});

test('a newer adapter from the first callable getter keeps its original receiver', async () => {
  const fixture = searchOwnerFixture(),
    owner = fixture.owner,
    request = owner.setContext({ fieldId: 'field', queryId: 'query' })!;
  owner.publish(searchPublication(request));
  const calls: string[] = [],
    newer = {
      reveal() {
        assert.equal(this, newer);
        calls.push('newer');
        return Promise.resolve('ready' as const);
      },
    };
  let reads = 0;
  owner.setAdapter({
    get reveal() {
      reads++;
      owner.setAdapter(newer);
      return () => {
        calls.push('older');
        return Promise.resolve('ready' as const);
      };
    },
  });
  assert.deepEqual(await owner.navigate(searchNavigation(request)), {
    status: 'applied',
  });
  assert.deepEqual(calls, ['newer']);
  assert.equal(reads, 1);
  owner.dispose();
});

test('result getters cannot publish newer metadata and then be overwritten by the capturing caller', () => {
  const fixture = searchOwnerFixture(),
    owner = fixture.owner,
    request = owner.setContext({ fieldId: 'field', queryId: 'query' })!;
  owner.publish(searchPublication(request));
  let reads = 0;
  const older = {
    ...searchPublication(request, 2),
    get resultRevision() {
      reads++;
      owner.publish(searchPublication(request, 3));
      return 2;
    },
  };
  assert.deepEqual(owner.publish(older), { status: 'stale' });
  assert.equal(owner.snapshot().resultRevision, 3);
  assert.equal(reads, 1);
  assert.equal(fixture.state.selection.main.head, 0);
  assert.equal(undoDepth(fixture.state), 0);
  owner.dispose();
});

test('navigation fields remain captured through actual asynchronous reveal and focus', async () => {
  const fixture = searchOwnerFixture(),
    owner = fixture.owner,
    request = owner.setContext({ fieldId: 'field', queryId: 'query' })!;
  owner.publish(searchPublication(request));
  let selects = 0,
    focuses = 0,
    methods = 0;
  const adapter = {
    get reveal() {
      methods++;
      return function (this: typeof adapter) {
        assert.equal(this, adapter);
        return Promise.resolve('ready' as const);
      };
    },
  };
  owner.setAdapter(adapter);
  const intent = {
    ...searchNavigation(request),
    get select() {
      selects++;
      return true;
    },
    get focus() {
      focuses++;
      return true;
    },
  };
  assert.deepEqual(await owner.navigate(intent), { status: 'applied' });
  assert.deepEqual([selects, focuses, methods], [1, 1, 1]);
  assert.equal(fixture.state.selection.main.head, 5);
  assert.equal(undoDepth(fixture.state), 0);
  owner.dispose();
});

for (const key of ['select', 'focus'] as const)
  test(`new context from initial ${key} capture retires the outer navigation before dispatch`, async () => {
    const fixture = searchOwnerFixture(),
      owner = fixture.owner,
      request = owner.setContext({ fieldId: 'field', queryId: 'query' })!;
    owner.publish(searchPublication(request));
    let reads = 0,
      newer: ReturnType<typeof owner.setContext> = null;
    const intent = {
      ...searchNavigation(request),
      get [key]() {
        reads++;
        newer = owner.setContext({ fieldId: 'field', queryId: 'newer' });
        owner.publish(searchPublication(newer!, 3));
        return true;
      },
    };
    assert.deepEqual(await owner.navigate(intent), { status: 'stale' });
    assert.equal(owner.snapshot().request, newer);
    assert.equal(owner.snapshot().resultRevision, 3);
    assert.equal(fixture.state.selection.main.head, 0);
    assert.equal(fixture.focuses.length, 0);
    assert.equal(reads, 1);
    owner.dispose();
  });

test('native reveal retains its genuine issued request through attempted host-observer mutation', async () => {
  const fixture = searchOwnerFixture(),
    owner = fixture.owner,
    request = owner.setContext({ fieldId: 'field', queryId: 'query' })!;
  owner.publish(searchPublication(request));
  let attempted = false;
  fixture.hooks.intent = (intent) => {
    if (intent.kind === 'reveal') {
      attempted = true;
      assert.equal(
        Reflect.set(intent, 'request', Object.freeze({ ...intent.request, fieldId: 'other' })),
        false,
      );
    }
  };
  owner.setAdapter({
    reveal(intent) {
      assert.equal(intent.request, request);
      return Promise.resolve('ready');
    },
  });
  try {
    assert.deepEqual(await owner.navigate(searchNavigation(request)), {
      status: 'applied',
    });
    assert.equal(attempted, true);
    assert.equal(owner.snapshot().request, request);
    assert.equal(readExactSource(fixture.state) === 'alpha', true);
    assert.equal(undoDepth(fixture.state), 0);
  } finally {
    owner.dispose();
  }
});
