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
/** Actual native input observation retires host navigation without a rival composition owner. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { searchOwnerFixture, searchPublication, searchNavigation } from './search-owner-fixture.ts';

test('composition start settles an ignored-abort reveal before selection/scroll/focus without source mutation', async () => {
  const fixture = searchOwnerFixture(),
    owner = fixture.owner,
    request = owner.setContext({ fieldId: 'field', queryId: 'query' })!;
  owner.publish(searchPublication(request));
  let signal!: AbortSignal;
  owner.setAdapter({
    reveal: (_intent, current) => {
      signal = current;
      return new Promise(() => {});
    },
  });
  const held = owner.navigate(searchNavigation(request));
  fixture.beginComposition();
  assert.equal(signal.aborted, true);
  assert.deepEqual(await held, { status: 'cancelled' });
  assert.equal(fixture.state.selection.main.head, 0);
  assert.equal(fixture.focuses.length, 0);
  assert.equal(readExactSource(fixture.state), 'alpha');
  assert.equal(undoDepth(fixture.state), 0);
  fixture.endComposition();
  await Promise.resolve();
  assert.notEqual(owner.snapshot().request, request);
  assert.equal(fixture.refreshes.at(-1), owner.snapshot().request);
  owner.dispose();
});

test('post-reveal composition recheck rejects a changed actual input facet even before observation publishes', async () => {
  const fixture = searchOwnerFixture(),
    owner = fixture.owner,
    request = owner.setContext({ fieldId: 'field', queryId: 'query' })!;
  owner.publish(searchPublication(request));
  let finish!: (outcome: 'ready') => void;
  owner.setAdapter({
    reveal: () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  });
  const held = owner.navigate(searchNavigation(request));
  fixture.beginComposition(false);
  finish('ready');
  assert.deepEqual(await held, { status: 'stale' });
  assert.equal(fixture.state.selection.main.head, 0);
  assert.equal(fixture.focuses.length, 0);
  owner.dispose();
});

test('composition supersedes held native focus readiness and clear never starts focus during preedit', async () => {
  const fixture = searchOwnerFixture(true),
    owner = fixture.owner,
    request = owner.setContext({ fieldId: 'field', queryId: 'query' })!;
  owner.publish(searchPublication(request));
  const held = owner.navigate(searchNavigation(request));
  await Promise.resolve();
  await Promise.resolve();
  assert.equal(fixture.pendingFocus, true);
  const before = fixture.state.selection;
  fixture.beginComposition();
  assert.equal(fixture.pendingFocus, false);
  assert.deepEqual(await held, { status: 'stale' });
  assert.equal(fixture.state.selection, before);
  assert.equal(readExactSource(fixture.state), 'alpha');
  assert.equal(undoDepth(fixture.state), 0);
  assert.deepEqual(await owner.clear(true), { status: 'unavailable' });
  assert.equal(fixture.focuses.length, 1);
  owner.dispose();
});
