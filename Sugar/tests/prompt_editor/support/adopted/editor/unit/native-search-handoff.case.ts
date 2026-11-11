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
/** Controller-style search observation cannot invalidate an accepted command's actual surface continuation. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { searchSurfaceFixture } from './search-surface-fixture.ts';
import { searchPublication, searchNavigation } from './search-owner-fixture.ts';

for (const active of [false, true])
  test(`passive ${active ? 'active' : 'inactive'} search publication preserves an accepted menu handoff`, async () => {
    const fixture = searchSurfaceFixture();
    try {
      const request = active
        ? fixture.search.setContext({ fieldId: 'field', queryId: 'query' })
        : null;
      const handoff = fixture.surface.handoff();
      const command = fixture.commands.api.prepareSourceEdit(0, 1, 'z');
      assert.equal(command.status, 'ready');
      if (command.status !== 'ready') return;
      assert.deepEqual(fixture.commands.api.applySourceEdit(command.command, handoff), {
        status: 'applied',
      });
      assert.equal(fixture.surface.currentHandoff(handoff), true);
      assert.equal(readExactSource(fixture.surface.view.state), 'zlpha');
      if (active) assert.notEqual(fixture.search.snapshot().request, request);
      const restored = handoff.restore('preserve-viewport');
      await fixture.flush();
      assert.equal(await restored, true);
      assert.equal(fixture.counts().focuses, 1);
    } finally {
      fixture.dispose();
    }
  });

test('passive composition observation cancels only pending search focus, leaving a newer menu continuation current', async () => {
  const fixture = searchSurfaceFixture();
  try {
    const request = fixture.search.setContext({
      fieldId: 'field',
      queryId: 'query',
    })!;
    fixture.search.publish(searchPublication(request));
    const pending = fixture.search.navigate(searchNavigation(request));
    await Promise.resolve();
    await Promise.resolve();
    assert.equal(fixture.counts().focuses, 1);
    const menu = fixture.surface.handoff();
    fixture.composition(true);
    assert.equal(fixture.surface.currentHandoff(menu), true);
    assert.deepEqual(await pending, { status: 'stale' });
    fixture.composition(false);
    const restored = menu.restore('preserve-viewport');
    await fixture.flush();
    assert.equal(await restored, true);
  } finally {
    fixture.dispose();
  }
});

test('an already cancelled search focus cannot invalidate a newer actual surface handoff', async () => {
  const fixture = searchSurfaceFixture(),
    cancellation = new AbortController();
  try {
    const menu = fixture.surface.handoff();
    cancellation.abort();
    assert.equal(await fixture.surface.focus('preserve-viewport', cancellation.signal), false);
    assert.equal(fixture.surface.currentHandoff(menu), true);
    assert.equal(fixture.counts().focuses, 0);
  } finally {
    fixture.dispose();
  }
});

test('deliberate context and navigation continue to supersede current panel handoffs', async () => {
  const fixture = searchSurfaceFixture();
  try {
    const beforeContext = fixture.surface.handoff();
    const request = fixture.search.setContext({
      fieldId: 'field',
      queryId: 'query',
    })!;
    assert.equal(fixture.surface.currentHandoff(beforeContext), false);
    fixture.search.publish(searchPublication(request));
    const beforeNavigation = fixture.surface.handoff();
    const navigation = fixture.search.navigate({
      ...searchNavigation(request),
      focus: false,
    });
    assert.equal(fixture.surface.currentHandoff(beforeNavigation), false);
    assert.deepEqual(await navigation, { status: 'applied' });
  } finally {
    fixture.dispose();
  }
});

test('passive native composition observation settles real held search focus before late measurements', async () => {
  const fixture = searchSurfaceFixture();
  try {
    const request = fixture.search.setContext({
      fieldId: 'field',
      queryId: 'query',
    })!;
    fixture.search.publish(searchPublication(request));
    const pending = fixture.search.navigate(searchNavigation(request));
    await Promise.resolve();
    await Promise.resolve();
    assert.equal(fixture.counts().focuses, 1);
    fixture.composition(true);
    assert.deepEqual(await pending, { status: 'stale' });
    await fixture.flush();
    assert.equal(fixture.counts().focuses, 1);
    assert.equal(fixture.counts().queuedMeasurements, 0);
  } finally {
    fixture.dispose();
  }
});

test("a stale search abort leaves a newer menu restoration's canonical readiness current", async () => {
  const fixture = searchSurfaceFixture();
  try {
    const request = fixture.search.setContext({
      fieldId: 'field',
      queryId: 'query',
    })!;
    fixture.search.publish(searchPublication(request));
    const pending = fixture.search.navigate(searchNavigation(request));
    await Promise.resolve();
    await Promise.resolve();
    const menu = fixture.surface.handoff();
    const restored = menu.restore('preserve-viewport');
    fixture.surface.view.dispatch({
      effects: invalidateCommandContext.of(null),
    });
    assert.deepEqual(await pending, { status: 'stale' });
    await fixture.flush();
    assert.equal(await restored, true);
    assert.equal(fixture.counts().focuses, 2);
  } finally {
    fixture.dispose();
  }
});

test('search teardown cancels its pending request without retiring a newer live menu handoff', async () => {
  const fixture = searchSurfaceFixture();
  try {
    const request = fixture.search.setContext({
      fieldId: 'field',
      queryId: 'query',
    })!;
    fixture.search.publish(searchPublication(request));
    const pending = fixture.search.navigate(searchNavigation(request));
    await Promise.resolve();
    await Promise.resolve();
    const menu = fixture.surface.handoff();
    fixture.search.dispose();
    assert.deepEqual(await pending, { status: 'stale' });
    assert.equal(fixture.surface.currentHandoff(menu), true);
    const restored = menu.restore('preserve-viewport');
    await fixture.flush();
    assert.equal(await restored, true);
  } finally {
    fixture.dispose();
  }
});
