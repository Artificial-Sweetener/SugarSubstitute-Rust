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
/** Genuine opening ownership retires prior focus reveal before native numeric activation. */
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { exactWeightDraft } from '../../../../../../frontend/sugarprompt/editor/src/exact-weight-draft.ts';
import { openingControl } from './exact-weight-opening-fixture.ts';

test('unretired genuine focus readiness dispatches its admitted late reveal', async () => {
  const c = openingControl();
  try {
    const ready = c.surface.focus();
    assert.equal(c.reveals, 1);
    await c.measure();
    await c.measure();
    assert.equal(c.reveals, 2);
    await c.drain();
    assert.equal(await ready, true);
  } finally {
    c.dispose();
  }
});
test('actual opening retires prior reveal without source selection or history mutation', async () => {
  const c = openingControl();
  try {
    const before = c.state,
      ready = c.surface.focus();
    await c.measure();
    assert.equal(c.open(), true);
    await Promise.resolve();
    await c.drain();
    assert.equal(await ready, false);
    assert.equal(c.reveals, 1);
    assert.equal(c.state.doc.toString(), before.doc.toString());
    assert.equal(c.state.selection.eq(before.selection), true);
    assert.equal(undoDepth(c.state), 0);
  } finally {
    c.dispose();
  }
});
test('accepted input transfers opening capability to the genuine synchronous source queue', async () => {
  const c = openingControl();
  try {
    c.open();
    await Promise.resolve();
    c.key('Enter');
    assert.equal(c.state.doc.toString(), '(cat:2.40), tail');
    assert.equal(c.state.selection.main.head, 10);
    assert.equal(undoDepth(c.state), 1);
    assert.equal(c.state.field(exactWeightDraft), null);
    await c.drain();
  } finally {
    c.dispose();
  }
});
test('new host focus supersedes the opening capability instead of being replaced at finish', async () => {
  const c = openingControl();
  try {
    c.open();
    await Promise.resolve();
    const ready = c.surface.focus();
    c.key('Enter');
    assert.equal(c.state.doc.toString(), '(cat:1.20), tail');
    assert.equal(undoDepth(c.state), 0);
    await c.drain();
    assert.equal(await ready, true);
  } finally {
    c.dispose();
  }
});
test('source retirement cancels only the captured input capability', async () => {
  const c = openingControl();
  try {
    c.open();
    await Promise.resolve();
    const ready = c.surface.focus();
    c.replace();
    await Promise.resolve();
    c.key('Enter');
    await c.drain();
    assert.equal(await ready, true);
    assert.equal(c.state.doc.toString(), 'other');
    assert.equal(c.state.field(exactWeightDraft), null);
  } finally {
    c.dispose();
  }
});
test('older widget eviction cannot cancel a genuine replacement input', async () => {
  const c = openingControl();
  try {
    c.open();
    const old = c.draftKey;
    c.open();
    assert.notEqual(c.draftKey, old);
    await Promise.resolve();
    assert.notEqual(c.state.field(exactWeightDraft), null);
    c.key('Enter');
    assert.equal(c.state.doc.toString(), '(cat:2.40), tail');
    await c.drain();
  } finally {
    c.dispose();
  }
});
test('evicted current widget retires draft and leaves future host focus live', async () => {
  const c = openingControl();
  try {
    c.open();
    await Promise.resolve();
    c.evict();
    await Promise.resolve();
    assert.equal(c.state.field(exactWeightDraft), null);
    const ready = c.surface.focus();
    await c.drain();
    assert.equal(await ready, true);
    assert.equal(undoDepth(c.state), 0);
  } finally {
    c.dispose();
  }
});
test('cancelled and invalid input settle captured restoration without source history', async () => {
  for (const key of ['Escape', 'Enter']) {
    const c = openingControl();
    try {
      c.open();
      await Promise.resolve();
      c.key(key, 'invalid');
      await c.drain();
      assert.equal(c.state.doc.toString(), '(cat:1.20), tail');
      assert.equal(undoDepth(c.state), 0);
      assert.equal(c.state.field(exactWeightDraft), null);
    } finally {
      c.dispose();
    }
  }
});
test('interaction disposal preserves newer focus and aborts old input callbacks', async () => {
  const c = openingControl();
  try {
    c.open();
    await Promise.resolve();
    const ready = c.surface.focus();
    c.destroyOpening();
    c.key('Enter');
    await c.drain();
    assert.equal(await ready, true);
    assert.equal(c.state.doc.toString(), '(cat:1.20), tail');
    assert.equal(undoDepth(c.state), 0);
  } finally {
    c.dispose();
  }
});

test('cancelled or invalid numeric input restores focus without a current or late caret reveal', async () => {
  for (const key of ['Escape', 'Enter']) {
    const c = openingControl();
    try {
      const before = c.state;
      c.open();
      await Promise.resolve();
      c.key(key, 'invalid');
      assert.equal(c.reveals, 0);
      assert.equal(c.surface.view.hasFocus, true);
      await c.measure();
      await c.measure();
      await c.drain();
      assert.equal(c.reveals, 0);
      assert.equal(c.state.doc.toString(), before.doc.toString());
      assert.equal(c.state.selection.eq(before.selection), true);
      assert.equal(undoDepth(c.state), 0);
      assert.equal(c.state.field(exactWeightDraft), null);
    } finally {
      c.dispose();
    }
  }
});

test('cancelled input cannot replace a newer explicit host reveal or its finite readiness', async () => {
  const c = openingControl();
  try {
    c.open();
    await Promise.resolve();
    const ready = c.surface.focus();
    assert.equal(c.reveals, 1);
    c.key('Escape');
    await c.drain();
    assert.equal(await ready, true);
    assert.equal(c.reveals, 2);
    assert.equal(c.state.doc.toString(), '(cat:1.20), tail');
    assert.equal(undoDepth(c.state), 0);
  } finally {
    c.dispose();
  }
});

test('replacement numeric input cancels only its current spatial capability before future host reveal', async () => {
  const c = openingControl();
  try {
    c.open();
    const old = c.draftKey;
    c.open();
    assert.notEqual(c.draftKey, old);
    await Promise.resolve();
    c.key('Escape');
    await c.drain();
    assert.equal(c.reveals, 0);
    const ready = c.surface.focus();
    await c.drain();
    assert.equal(await ready, true);
    assert.equal(c.reveals, 2);
    assert.equal(undoDepth(c.state), 0);
  } finally {
    c.dispose();
  }
});
