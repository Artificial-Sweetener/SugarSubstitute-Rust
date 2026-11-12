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
/** Host abort, intent and native publication callbacks must preserve newer real owner intent. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { EditorState, StateEffect } from '@codemirror/state';
import {
  readExactSource,
  exactReplacement,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { searchMatches } from '../../../../../../frontend/sugarprompt/editor/src/search-matches.ts';
import { searchOwnerFixture, searchPublication, searchNavigation } from './search-owner-fixture.ts';
import type { SearchRequest } from '../../../../../../frontend/sugarprompt/services/src/search/contracts.ts';

for (const action of ['context', 'results', 'adapter', 'navigation', 'clear'] as const)
  test(`newer ${action} from synchronous abort supersedes older native continuation`, async () => {
    const fixture = searchOwnerFixture(),
      owner = fixture.owner,
      request = owner.setContext({ fieldId: 'field', queryId: 'initial' })!;
    owner.publish(searchPublication(request));
    const calls: string[] = [];
    const release: Array<(outcome: 'ready') => void> = [];
    let newerNavigation: Promise<unknown> | undefined;
    const newer = {
        reveal: () => {
          calls.push('newer');
          return Promise.resolve('ready' as const);
        },
      },
      older = {
        reveal: () => {
          calls.push('older');
          return Promise.resolve('ready' as const);
        },
      };
    owner.setAdapter({
      reveal: (_intent, signal) => {
        signal.addEventListener(
          'abort',
          () => {
            if (action === 'context' || action === 'clear') {
              const fresh = owner.setContext({
                fieldId: 'field',
                queryId: 'newer',
              })!;
              owner.publish(searchPublication(fresh, 3));
            }
            if (action === 'results') owner.publish(searchPublication(request, 3));
            if (action === 'adapter') owner.setAdapter(newer);
            if (action === 'navigation') {
              owner.setAdapter(newer);
              newerNavigation = owner.navigate({
                ...searchNavigation(request),
                focus: false,
              });
            }
          },
          { once: true },
        );
        return new Promise<'ready'>((resolve) => release.push(resolve));
      },
    });
    const held = owner.navigate({
      ...searchNavigation(request),
      select: false,
      scroll: false,
      focus: false,
    });
    if (action === 'context') owner.setContext({ fieldId: 'field', queryId: 'older' });
    if (action === 'results')
      assert.deepEqual(owner.publish(searchPublication(request, 2)), {
        status: 'stale',
      });
    if (action === 'adapter') owner.setAdapter(older);
    const continued =
      action === 'navigation'
        ? owner.navigate({ ...searchNavigation(request), focus: false })
        : undefined;
    for (const resolve of release) resolve('ready');
    if (continued) assert.notEqual((await continued).status, 'applied');
    if (action === 'clear') assert.deepEqual(await owner.clear(true), { status: 'stale' });
    assert.deepEqual(await held, { status: 'cancelled' });
    if (action === 'context' || action === 'clear' || action === 'results')
      assert.equal(owner.snapshot().resultRevision, 3);
    if (action === 'context' || action === 'clear')
      assert.equal(owner.snapshot().request!.queryId, 'newer');
    if (action === 'adapter') {
      await owner.navigate({ ...searchNavigation(request), focus: false });
      assert.deepEqual(calls, ['newer']);
    }
    if (action === 'navigation') {
      assert.deepEqual(await newerNavigation, { status: 'applied' });
      assert.deepEqual(calls, ['newer']);
    }
    assert.equal(readExactSource(fixture.state), 'alpha');
    assert.equal(undoDepth(fixture.state), 0);
    owner.dispose();
  });

test('newer intent at native retirement, native dispatch and host filter boundaries prevents older focus/publication', async () => {
  const fixture = searchOwnerFixture(),
    owner = fixture.owner;
  const request = owner.setContext({ fieldId: 'field', queryId: 'initial' })!;
  owner.publish(searchPublication(request));
  fixture.hooks.retire = () => {
    delete fixture.hooks.retire;
    const fresh = owner.setContext({
      fieldId: 'field',
      queryId: 'retire-newer',
    })!;
    owner.publish(searchPublication(fresh, 3));
  };
  assert.deepEqual(owner.publish(searchPublication(request, 2)), {
    status: 'stale',
  });
  assert.equal(owner.snapshot().resultRevision, 3);
  fixture.hooks.intent = (intent) => {
    if (intent.kind === 'filter' && intent.request.queryId === 'older') {
      const fresh = owner.setContext({
        fieldId: 'field',
        queryId: 'filter-newer',
      })!;
      owner.publish(searchPublication(fresh, 4));
    }
  };
  owner.setContext({ fieldId: 'field', queryId: 'older' });
  assert.equal(owner.snapshot().request!.queryId, 'filter-newer');
  const fresh = owner.snapshot().request!;
  fixture.hooks.dispatch = () => {
    delete fixture.hooks.dispatch;
    const replacement = owner.setContext({
      fieldId: 'field',
      queryId: 'dispatch-newer',
    })!;
    owner.publish(searchPublication(replacement, 5));
  };
  const outcome = await owner.navigate({
    ...searchNavigation(fresh),
    resultRevision: 4,
  });
  assert.deepEqual(outcome, { status: 'stale' });
  assert.equal(owner.snapshot().resultRevision, 5);
  assert.equal(fixture.focuses.length, 0);
  owner.dispose();
});

test('native disposal rejects revival before external abort and null publication cannot populate cleared paint', async () => {
  const fixture = searchOwnerFixture(),
    owner = fixture.owner,
    request = owner.setContext({ fieldId: 'field', queryId: 'query' })!;
  owner.publish(searchPublication(request));
  let revival: Promise<unknown> | undefined,
    adapterInvoked = false;
  owner.setAdapter({
    reveal: (_intent, signal) => {
      signal.addEventListener(
        'abort',
        () => {
          owner.setAdapter({
            reveal: () => {
              adapterInvoked = true;
              return Promise.resolve('ready');
            },
          });
          revival = owner.navigate(searchNavigation(request));
        },
        { once: true },
      );
      return new Promise(() => {});
    },
  });
  const held = owner.navigate(searchNavigation(request));
  owner.dispose();
  assert.deepEqual(await held, { status: 'cancelled' });
  assert.deepEqual(await revival, { status: 'cancelled' });
  assert.equal(adapterInvoked, false);
  const cleared = searchOwnerFixture();
  assert.deepEqual(cleared.owner.publish(searchPublication(null as unknown as SearchRequest)), {
    status: 'stale',
  });
  assert.equal(cleared.state.field(searchMatches), null);
  cleared.owner.dispose();
});

for (const change of ['source', 'permission'] as const)
  test(`newer ${change} identity from abort callback rejects held source preparation`, async () => {
    const fixture = searchOwnerFixture(),
      owner = fixture.owner,
      request = owner.setContext({ fieldId: 'field', queryId: 'query' })!;
    owner.publish(searchPublication(request));
    owner.setAdapter({
      reveal: (_intent, signal) => {
        signal.addEventListener(
          'abort',
          () => {
            fixture.change(
              change === 'source'
                ? exactReplacement(fixture.state, 0, 1, 'z')
                : {
                    effects: StateEffect.appendConfig.of(EditorState.readOnly.of(true)),
                  },
            );
            owner.publish(searchPublication(owner.snapshot().request!, 3));
          },
          { once: true },
        );
        return new Promise(() => {});
      },
    });
    const held = owner.navigate({ ...searchNavigation(request), focus: false });
    assert.deepEqual(owner.publish(searchPublication(request, 2)), {
      status: 'stale',
    });
    assert.deepEqual(await held, { status: 'cancelled' });
    assert.equal(owner.snapshot().resultRevision, 3);
    assert.notEqual(owner.snapshot().request, request);
    assert.equal(owner.snapshot().request!.sourceRevision, change === 'source' ? 1 : 0);
    assert.equal(readExactSource(fixture.state), change === 'source' ? 'zlpha' : 'alpha');
    assert.equal(fixture.state.readOnly, change === 'permission');
    owner.dispose();
  });
