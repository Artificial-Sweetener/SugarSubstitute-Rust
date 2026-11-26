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
/** A pasted Danbooru link becomes its post's tags as one paste, or the link when there are none. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undo, redo, undoDepth } from '@codemirror/commands';
import { Compartment, EditorSelection, StateEffect, Transaction } from '@codemirror/state';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { booruFixture, booruReply, lookupStarted } from './booru-paste-import-fixture.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import {
  defaultPreferences,
  resolveFeatureProfile,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';

const url = 'https://danbooru.donmai.us/posts/123';
const enabled = resolveFeatureProfile(defaultPreferences, { mode: 'library' });
const disabled = resolveFeatureProfile(
  { ...defaultPreferences, allowed: { ...defaultPreferences.allowed, danbooru_url_import: false } },
  { mode: 'library' },
);

/** A fixture whose feature profile can be swapped while a paste waits. */
function fixture(source = '') {
  const f = booruFixture(source);
  const slot = new Compartment();
  f.view.dispatch({
    effects: StateEffect.appendConfig.of(slot.of(featureProfile.of(enabled))),
    annotations: Transaction.addToHistory.of(false),
  });
  return { ...f, slot };
}

test('nothing is pasted while the post is looked up; its tags then paste as one undo step', async () => {
  const f = fixture();
  try {
    const pending = f.owner.start(url);
    assert.ok(pending);
    await lookupStarted();
    assert.equal(readExactSource(f.state()), '');
    assert.equal(undoDepth(f.state()), 0);
    f.resolve(booruReply());
    await pending;
    assert.equal(readExactSource(f.state()), 'cat, blue eyes');
    assert.equal(undoDepth(f.state()), 1);
    assert.equal(undo(f.view), true);
    // Undo removes the tags; the link never was in the prompt.
    assert.equal(readExactSource(f.state()), '');
    assert.equal(redo(f.view), true);
    assert.equal(readExactSource(f.state()), 'cat, blue eyes');
  } finally {
    f.destroy();
  }
});

test('a lookup that finds no tags pastes the link as written, as one undo step', async () => {
  const replies: unknown[] = [
    { status: 'not-found' },
    { status: 'unavailable' },
    { status: 'found', post: { id: 'wrong' } },
    booruReply(''),
  ];
  for (const reply of replies) {
    const f = fixture();
    try {
      const pending = f.owner.start(url);
      assert.ok(pending);
      await lookupStarted();
      f.resolve(reply);
      await pending;
      assert.equal(readExactSource(f.state()), url);
      assert.equal(undoDepth(f.state()), 1);
      assert.equal(undo(f.view), true);
      assert.equal(readExactSource(f.state()), '');
    } finally {
      f.destroy();
    }
  }
});

test('typing while waiting moves the paste point; the tags land where the link was pasted', async () => {
  const f = fixture('suffix');
  try {
    const pending = f.owner.start(url);
    assert.ok(pending);
    await lookupStarted();
    f.view.dispatch({ changes: { from: 0, insert: 'ab, ' }, userEvent: 'input.type' });
    f.resolve(booruReply());
    await pending;
    assert.equal(readExactSource(f.state()), 'cat, blue eyesab, suffix');
  } finally {
    f.destroy();
  }
});

test('a paste replaces the text selected when it was pasted', async () => {
  const f = fixture('old tags, keep');
  try {
    f.view.dispatch({ selection: EditorSelection.single(0, 8) });
    const pending = f.owner.start(url);
    assert.ok(pending);
    await lookupStarted();
    f.resolve(booruReply());
    await pending;
    assert.equal(readExactSource(f.state()), 'cat, blue eyes, keep');
  } finally {
    f.destroy();
  }
});

test('leaving the field, turning the feature off or pasting again drops the waiting paste', async () => {
  const left = fixture();
  try {
    const pending = left.owner.start(url);
    assert.ok(pending);
    await lookupStarted();
    Object.defineProperty(left.view, 'hasFocus', { value: false });
    left.owner.update({ transactions: [], focusChanged: true });
    assert.equal(left.signal()?.aborted, true);
    left.resolve(booruReply());
    await pending;
    assert.equal(readExactSource(left.state()), '');
  } finally {
    left.destroy();
  }
  const off = fixture();
  try {
    const pending = off.owner.start(url);
    assert.ok(pending);
    await lookupStarted();
    off.view.dispatch({
      effects: off.slot.reconfigure(featureProfile.of(disabled)),
      annotations: Transaction.addToHistory.of(false),
    });
    off.resolve(booruReply());
    await pending;
    assert.equal(readExactSource(off.state()), '');
  } finally {
    off.destroy();
  }
  const again = fixture();
  try {
    const first = again.owner.start(url);
    assert.ok(first);
    await lookupStarted();
    const firstSignal = again.signal();
    const second = again.owner.start(url);
    assert.ok(second);
    assert.equal(firstSignal?.aborted, true);
    await first;
    await lookupStarted();
    again.resolve(booruReply('dog'));
    await second;
    assert.equal(readExactSource(again.state()), 'dog');
  } finally {
    again.destroy();
  }
});

test('a disabled feature and text that is no supported link stay with ordinary paste', () => {
  const f = fixture('x');
  try {
    assert.equal(f.owner.start('https://example.com/posts/123'), null);
    assert.equal(f.owner.start('https://danbooru.donmai.us/posts/123/'), null);
    f.view.dispatch({
      effects: f.slot.reconfigure(featureProfile.of(disabled)),
      annotations: Transaction.addToHistory.of(false),
    });
    assert.equal(f.owner.start(url), null);
    assert.equal(f.signal(), null);
    assert.equal(readExactSource(f.state()), 'x');
  } finally {
    f.destroy();
  }
});

test('an unexpected lookup failure stays observable and pastes nothing', async () => {
  const f = fixture();
  try {
    const pending = f.owner.start(url);
    assert.ok(pending);
    await lookupStarted();
    const cause = new Error('host failure');
    f.reject(cause);
    await assert.rejects(pending, (error) => error === cause);
    assert.equal(readExactSource(f.state()), '');
  } finally {
    f.destroy();
  }
});
