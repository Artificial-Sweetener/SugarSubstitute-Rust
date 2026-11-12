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
/** Controlled native publication proves navigation admission/lifetime without substituting geometry proof. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, type TransactionSpec } from '@codemirror/state';
import { history, undoDepth } from '@codemirror/commands';
import {
  exactSourceExtension,
  exactReplacement,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  editorFreshness,
  invalidateCommandContext,
} from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { searchMatches } from '../../../../../../frontend/sugarprompt/editor/src/search-matches.ts';
import { NativeSearch } from '../../../../../../frontend/sugarprompt/editor/src/native-search.ts';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import type { NativeSurface } from '../../../../../../frontend/sugarprompt/editor/src/native-surface.ts';
import type {
  SearchResults,
  SearchRequest,
} from '../../../../../../frontend/sugarprompt/services/src/search/contracts.ts';

/** Actual state supplies authoritative source/history; the seam records only focus intent and transactions. */
function fixture(source = 'alpha', holdFocus = false) {
  let finishFocus!: (value: boolean) => void;
  const focusReady = new Promise<boolean>((resolve) => {
    finishFocus = resolve;
  });
  let current = EditorState.create({
      doc: encodeSource(source).logical,
      extensions: [exactSourceExtension(source), editorFreshness, searchMatches, history()],
    }),
    revision = 0;
  const refreshes: SearchRequest[] = [],
    focuses: string[] = [];
  const view = {
    get state() {
      return current;
    },
    composing: false,
    dispatch(spec: TransactionSpec) {
      current = current.update(spec).state;
    },
  };
  const surface = {
    view,
    retireHandoff() {},
    focus: (viewport: string, signal?: AbortSignal) => {
      focuses.push(viewport);
      if (!holdFocus) return Promise.resolve(true);
      const cancel = () => finishFocus(false);
      signal?.addEventListener('abort', cancel, { once: true });
      void focusReady.finally(() => signal?.removeEventListener('abort', cancel));
      if (signal?.aborted) cancel();
      return focusReady;
    },
  };
  const owner = new NativeSearch(
    surface as unknown as NativeSurface,
    () => revision,
    (request) => refreshes.push(request),
    () => {},
  );
  return {
    owner,
    refreshes,
    focuses,
    finishFocus,
    get state() {
      return current;
    },
    change() {
      current = current.update(exactReplacement(current, 0, 1, 'z')).state;
      revision++;
      owner.observe();
    },
    context() {
      current = current.update({
        effects: invalidateCommandContext.of(null),
      }).state;
      owner.observe();
    },
    spelling() {
      const transaction = current.update({
        effects: exactReplacement(current, 5, 7, '\n').effects,
      });
      assert.equal(transaction.docChanged, false);
      current = transaction.state;
      revision++;
      owner.observe();
    },
  };
}
/** Independent host match publication binds an authored range to a request ticket. */
function publication(request: SearchRequest, revision = 1): SearchResults {
  return {
    request,
    resultRevision: revision,
    matches: [{ matchId: 'match', from: 0, to: 5 }],
    activeIndex: null,
  };
}

test('accepted metadata preserves source/history and explicit navigation preserves selection/viewport when requested', async () => {
  const fixtureOwner = fixture(),
    owner = fixtureOwner.owner;
  const request = owner.setContext({ fieldId: 'field', queryId: 'query' })!;
  assert.deepEqual(owner.publish(publication(request)), { status: 'applied' });
  const reused = fixtureOwner.state.field(searchMatches);
  assert.deepEqual(owner.publish(publication(request)), { status: 'stale' });
  assert.equal(fixtureOwner.state.field(searchMatches), reused);
  assert.throws(() => owner.setContext({ fieldId: '', queryId: 'query' }), TypeError);
  assert.equal(fixtureOwner.state.field(searchMatches), reused);
  owner.setAdapter({ reveal: () => Promise.resolve('unavailable') });
  assert.deepEqual(
    await owner.navigate({
      request,
      resultRevision: 1,
      matchId: 'match',
      select: true,
      scroll: true,
      focus: true,
    }),
    { status: 'unavailable' },
  );
  assert.equal(fixtureOwner.state.selection.main.head, 0);
  assert.equal(fixtureOwner.focuses.length, 0);
  owner.setAdapter(null);
  assert.equal(readExactSource(fixtureOwner.state), 'alpha');
  assert.equal(undoDepth(fixtureOwner.state), 0);
  assert.deepEqual(
    await owner.navigate({
      request,
      resultRevision: 1,
      matchId: 'match',
      select: false,
      scroll: false,
      focus: true,
    }),
    { status: 'applied' },
  );
  assert.equal(fixtureOwner.state.selection.main.head, 0);
  assert.deepEqual(fixtureOwner.focuses, ['preserve-viewport']);
  assert.equal(owner.snapshot().activeMatchId, 'match');
  assert.deepEqual(await owner.clear(true), { status: 'applied' });
  assert.equal(fixtureOwner.state.field(searchMatches), null);
  assert.equal(undoDepth(fixtureOwner.state), 0);
  owner.dispose();
});

test('spelling-only source retirement emits fresh recomputation and query/field changes reject old ranges', async () => {
  const fixtureOwner = fixture('alpha\r\nend'),
    owner = fixtureOwner.owner;
  const first = owner.setContext({ fieldId: 'first', queryId: 'query' })!;
  owner.publish(publication(first));
  await Promise.resolve();
  fixtureOwner.spelling();
  await Promise.resolve();
  assert.equal(fixtureOwner.refreshes.length, 2);
  assert.equal(fixtureOwner.refreshes[1]!.sourceRevision, 1);
  assert.equal(fixtureOwner.state.field(searchMatches), null);
  assert.deepEqual(owner.publish(publication(first, 2)), { status: 'stale' });
  const changed = owner.setContext({ fieldId: 'other', queryId: 'new' })!;
  assert.deepEqual(owner.publish(publication(fixtureOwner.refreshes[1]!, 2)), {
    status: 'stale',
  });
  assert.equal(changed.fieldId, 'other');
  owner.dispose();
});

test('clear settles held reveal and a later focus completion cannot revive cleared navigation', async () => {
  const fixtureOwner = fixture('alpha', true),
    owner = fixtureOwner.owner;
  const request = owner.setContext({ fieldId: 'field', queryId: 'query' })!;
  owner.publish(publication(request));
  owner.setAdapter({ reveal: () => new Promise(() => {}) });
  const intent = {
    request,
    resultRevision: 1,
    matchId: 'match',
    select: true,
    scroll: true,
    focus: true,
  };
  const held = owner.navigate(intent);
  await owner.clear(false);
  assert.deepEqual(await held, { status: 'cancelled' });
  owner.setAdapter(null);
  const fresh = owner.setContext({ fieldId: 'field', queryId: 'new' })!;
  owner.publish(publication(fresh));
  const focusing = owner.navigate({ ...intent, request: fresh });
  await Promise.resolve();
  await Promise.resolve();
  assert.equal(fixtureOwner.focuses.length, 1);
  await owner.clear(false);
  fixtureOwner.finishFocus(true);
  assert.deepEqual(await focusing, { status: 'stale' });
  assert.equal(fixtureOwner.state.field(searchMatches), null);
  owner.dispose();
});

test('source/context retirement and disposal cancel pending host reveal before native selection or focus', async () => {
  const fixtureOwner = fixture(),
    owner = fixtureOwner.owner;
  const request = owner.setContext({ fieldId: 'field', queryId: 'query' })!;
  owner.publish(publication(request));
  owner.setAdapter({ reveal: () => new Promise(() => {}) });
  const navigation = owner.navigate({
    request,
    resultRevision: 1,
    matchId: 'match',
    select: true,
    scroll: true,
    focus: true,
  });
  fixtureOwner.change();
  assert.deepEqual(await navigation, { status: 'cancelled' });
  assert.equal(fixtureOwner.state.selection.main.head, 1);
  assert.equal(fixtureOwner.focuses.length, 0);
  assert.deepEqual(owner.publish(publication(request, 2)), { status: 'stale' });
  const fresh = owner.snapshot().request!;
  owner.publish(publication(fresh));
  fixtureOwner.context();
  assert.equal(fixtureOwner.state.field(searchMatches), null);
  assert.deepEqual(owner.publish(publication(fresh, 2)), { status: 'stale' });
  const next = owner.snapshot().request!;
  owner.publish(publication(next));
  const pending = owner.navigate({
    request: next,
    resultRevision: 1,
    matchId: 'match',
    select: true,
    scroll: true,
    focus: true,
  });
  owner.dispose();
  assert.deepEqual(await pending, { status: 'cancelled' });
});
