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
/** Actual CM accepted paste, source freshness and native history independently characterize import lifetime. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undo, redo, undoDepth } from '@codemirror/commands';
import { Compartment, StateEffect, Transaction } from '@codemirror/state';
import { enteredTextInsertion } from '../../../../../../frontend/sugarprompt/editor/src/ingestion.ts';
import { dispatchNativeIngestion } from '../../../../../../frontend/sugarprompt/editor/src/native-ingestion.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { booruFixture, booruReply, lookupStarted } from './booru-paste-import-fixture.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import {
  defaultPreferences,
  featureEnabled,
  resolveFeatureProfile,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
const url = 'https://danbooru.donmai.us/posts/123';
/** Real service decisions exercise preference and field-scope admission independently. */
const disabledImportProfiles = [
  resolveFeatureProfile(
    {
      ...defaultPreferences,
      allowed: { ...defaultPreferences.allowed, danbooru_url_import: false },
    },
    { mode: 'library' },
  ),
  resolveFeatureProfile(defaultPreferences, {
    mode: 'workflow',
    field: { prompt_features: ['emphasis'] },
  }),
];

test('disabled preference and field scope leave supported URL paste to the ordinary owner', async () => {
  for (const profile of disabledImportProfiles) {
    const f = booruFixture('suffix');
    try {
      f.view.dispatch({
        effects: StateEffect.appendConfig.of(featureProfile.of(profile)),
        annotations: Transaction.addToHistory.of(false),
      });
      const before = f.state();
      assert.equal(featureEnabled(before.facet(featureProfile), 'danbooru_url_import'), false);
      assert.equal(f.owner.start(url), null);
      await lookupStarted();
      assert.equal(f.signal(), null);
      assert.equal(f.state(), before);
      assert.equal(readExactSource(f.state()), 'suffix');
      assert.equal(undoDepth(f.state()), 0);
      assert.equal(
        dispatchNativeIngestion(f.view, enteredTextInsertion(f.state(), 0, 0, url)),
        true,
      );
      assert.equal(readExactSource(f.state()), url + 'suffix');
      assert.equal(undoDepth(f.state()), 1);
      assert.equal(undo(f.view), true);
      assert.equal(readExactSource(f.state()), 'suffix');
    } finally {
      f.destroy();
    }
  }
});

test('disabling a genuine held import retires ignored-abort tags without changing the literal event', async () => {
  for (const profile of disabledImportProfiles) {
    const f = booruFixture();
    const slot = new Compartment();
    try {
      f.view.dispatch({
        effects: StateEffect.appendConfig.of(
          slot.of(
            featureProfile.of(resolveFeatureProfile(defaultPreferences, { mode: 'library' })),
          ),
        ),
        annotations: Transaction.addToHistory.of(false),
      });
      const pending = f.owner.start(url);
      assert.ok(pending);
      await lookupStarted();
      f.view.dispatch({
        effects: slot.reconfigure(featureProfile.of(profile)),
        annotations: Transaction.addToHistory.of(false),
      });
      const retired = f.state();
      assert.equal(f.signal()?.aborted, true);
      f.resolve(booruReply());
      await pending;
      assert.equal(f.state(), retired);
      assert.equal(readExactSource(f.state()), url);
      assert.equal(undoDepth(f.state()), 1);
      assert.equal(undo(f.view), true);
      assert.equal(readExactSource(f.state()), '');
      assert.equal(redo(f.view), true);
      assert.equal(readExactSource(f.state()), url);
    } finally {
      f.destroy();
    }
  }
});

test('an unrelated disabled feature leaves allowed import and its single history event intact', async () => {
  const f = booruFixture();
  try {
    const profile = resolveFeatureProfile(
      {
        ...defaultPreferences,
        allowed: { ...defaultPreferences.allowed, spellcheck: false },
      },
      { mode: 'library' },
    );
    f.view.dispatch({
      effects: StateEffect.appendConfig.of(featureProfile.of(profile)),
      annotations: Transaction.addToHistory.of(false),
    });
    assert.equal(featureEnabled(f.state().facet(featureProfile), 'danbooru_url_import'), true);
    const pending = f.owner.start(url);
    assert.ok(pending);
    await lookupStarted();
    f.resolve(booruReply());
    await pending;
    assert.equal(readExactSource(f.state()), 'cat, blue eyes');
    assert.equal(undoDepth(f.state()), 1);
    assert.equal(undo(f.view), true);
    assert.equal(readExactSource(f.state()), '');
    assert.equal(redo(f.view), true);
    assert.equal(readExactSource(f.state()), 'cat, blue eyes');
  } finally {
    f.destroy();
  }
});

test('literal paste is synchronous and ready current tags form one native undo/redo group', async () => {
  const f = booruFixture();
  const pending = f.owner.start(url);
  assert.ok(pending);
  assert.equal(readExactSource(f.state()), url);
  assert.equal(undoDepth(f.state()), 1);
  await lookupStarted();
  f.resolve(booruReply());
  await pending;
  assert.equal(readExactSource(f.state()), 'cat, blue eyes');
  assert.equal(undoDepth(f.state()), 1);
  assert.equal(undo(f.view), true);
  assert.equal(readExactSource(f.state()), '');
  assert.equal(redo(f.view), true);
  assert.equal(readExactSource(f.state()), 'cat, blue eyes');
  f.destroy();
});
test('unavailable literal paste and following ordinary typing remain independent native events', async () => {
  const f = booruFixture();
  const pending = f.owner.start(url);
  assert.ok(pending);
  await lookupStarted();
  f.view.dispatch({
    changes: { from: f.state().doc.length, insert: 'x' },
    userEvent: 'input.type',
  });
  assert.equal(f.signal()?.aborted, true);
  f.resolve({ status: 'unavailable' });
  await pending;
  assert.equal(readExactSource(f.state()), url + 'x');
  assert.equal(undo(f.view), true);
  assert.equal(readExactSource(f.state()), url);
  assert.equal(undo(f.view), true);
  assert.equal(readExactSource(f.state()), '');
  f.destroy();
});
test('undo and byte-equal source restoration cannot resurrect accepted paste authority', async () => {
  const f = booruFixture();
  const pending = f.owner.start(url);
  assert.ok(pending);
  await lookupStarted();
  assert.equal(undo(f.view), true);
  assert.equal(redo(f.view), true);
  f.resolve(booruReply());
  await pending;
  assert.equal(readExactSource(f.state()), url);
  f.destroy();
});
test('native selection, context, permission, composition and teardown each retire ignored-abort publication', async () => {
  for (const mode of ['selection', 'context', 'permission', 'composition', 'destroy']) {
    const f = booruFixture();
    const pending = f.owner.start(url);
    assert.ok(pending);
    await lookupStarted();
    if (mode === 'selection') f.view.dispatch({ selection: { anchor: 0 } });
    else if (mode === 'context')
      f.view.dispatch({
        effects: invalidateCommandContext.of(null),
        annotations: Transaction.addToHistory.of(false),
      });
    else if (mode === 'permission')
      f.view.dispatch({
        effects: f.initial.permission.reconfigure(
          (await import('@codemirror/state')).EditorState.readOnly.of(true),
        ),
      });
    else if (mode === 'composition') {
      Object.defineProperty(f.view, 'compositionStarted', { value: true });
      f.owner.update({ transactions: [], focusChanged: false });
    } else f.destroy();
    f.resolve(booruReply());
    await pending;
    assert.equal(readExactSource(f.state()), url);
    assert.equal(f.signal()?.aborted, true);
    f.destroy();
  }
});
test('unexpected external cause stays observable and releases accepted history capability', async () => {
  const f = booruFixture();
  const pending = f.owner.start(url);
  assert.ok(pending);
  await lookupStarted();
  const cause = new Error('host-boundary-failure');
  f.reject(cause);
  await assert.rejects(pending, (error) => error === cause);
  assert.equal(readExactSource(f.state()), url);
  assert.equal(f.signal()?.aborted, true);
  f.destroy();
});
test('unsupported links do not acquire a request or change native source', () => {
  const f = booruFixture();
  assert.equal(f.owner.start('ordinary text'), null);
  assert.equal(f.signal(), null);
  assert.equal(readExactSource(f.state()), '');
  f.destroy();
});

test('accepted native timestamp joins an arbitrarily delayed current lookup without joining following typing', async () => {
  const f = booruFixture();
  const now = Date.now;
  let clock = 1000;
  Date.now = () => clock;
  try {
    const pending = f.owner.start(url);
    assert.ok(pending);
    const initial = f.transactions.find((t) => t.isUserEvent('input.paste'));
    assert.ok(initial);
    assert.equal(initial.annotation(Transaction.time), 1000);
    await lookupStarted();
    clock = 100000;
    f.resolve(booruReply());
    await pending;
    const replacement = f.transactions.find((t) => t.docChanged && t !== initial);
    assert.ok(replacement);
    assert.equal(replacement.annotation(Transaction.time), initial.annotation(Transaction.time));
    assert.equal(replacement.annotation(Transaction.userEvent), undefined);
    assert.equal(undoDepth(f.state()), 1);
    f.view.dispatch({
      changes: { from: f.state().doc.length, insert: 'x' },
      userEvent: 'input.type',
    });
    assert.equal(undoDepth(f.state()), 2);
    assert.equal(undo(f.view), true);
    assert.equal(readExactSource(f.state()), 'cat, blue eyes');
    assert.equal(undo(f.view), true);
    assert.equal(readExactSource(f.state()), '');
  } finally {
    Date.now = now;
    f.destroy();
  }
});

test('supersession cancels the first external request and only genuine following paste can publish', async () => {
  const f = booruFixture();
  const first = f.owner.start(url);
  assert.ok(first);
  await lookupStarted();
  const oldSignal = f.signal();
  f.view.dispatch({ selection: { anchor: 0, head: f.state().doc.length } });
  const second = f.owner.start(url);
  assert.ok(second);
  await lookupStarted();
  assert.equal(oldSignal?.aborted, true);
  f.resolve(booruReply('dog'));
  await Promise.all([first, second]);
  assert.equal(readExactSource(f.state()), 'dog');
  f.destroy();
});

test('synchronous initial observer navigation prevents lookup acquisition after accepted literal publication', async () => {
  const f = booruFixture();
  let navigated = false;
  f.observe((transaction) => {
    if (!navigated && transaction.isUserEvent('input.paste')) {
      navigated = true;
      f.view.dispatch({ selection: { anchor: 0 } });
    }
  });
  const pending = f.owner.start(url);
  assert.ok(pending);
  await pending;
  assert.equal(readExactSource(f.state()), url);
  assert.equal(f.state().selection.main.head, 0);
  assert.equal(f.signal(), null);
  f.destroy();
});

test('synchronous neutral preparation observer reentry retires final replacement before publication', async () => {
  const f = booruFixture();
  const pending = f.owner.start(url);
  assert.ok(pending);
  await lookupStarted();
  let navigated = false;
  f.observe((transaction) => {
    if (!navigated && !transaction.docChanged) {
      navigated = true;
      f.view.dispatch({ selection: { anchor: 0 } });
    }
  });
  f.resolve(booruReply());
  await pending;
  assert.equal(navigated, true);
  assert.equal(readExactSource(f.state()), url);
  assert.equal(f.state().selection.main.head, 0);
  f.destroy();
});

test('same-source literal acceptance imports as one fresh event and undo restores original reverse selection', async () => {
  const f = booruFixture(url);
  f.view.dispatch({ selection: { anchor: url.length, head: 0 } });
  const pending = f.owner.start(url);
  assert.ok(pending);
  assert.equal(readExactSource(f.state()), url);
  assert.equal(undoDepth(f.state()), 0);
  await lookupStarted();
  f.resolve(booruReply('dog'));
  await pending;
  assert.equal(readExactSource(f.state()), 'dog');
  assert.equal(undoDepth(f.state()), 1);
  assert.equal(undo(f.view), true);
  assert.equal(readExactSource(f.state()), url);
  assert.equal(f.state().selection.main.anchor, url.length);
  assert.equal(f.state().selection.main.head, 0);
  f.destroy();
});

test('actual synchronous neutral publication after the literal remains a genuine current import tip', async () => {
  const f = booruFixture();
  let published = false;
  f.observe((transaction) => {
    if (!published && transaction.isUserEvent('input.paste')) {
      published = true;
      f.view.dispatch({ annotations: Transaction.addToHistory.of(false) });
    }
  });
  const pending = f.owner.start(url);
  assert.ok(pending);
  assert.equal(published, true);
  await lookupStarted();
  f.resolve(booruReply());
  await pending;
  assert.equal(readExactSource(f.state()), 'cat, blue eyes');
  assert.equal(undoDepth(f.state()), 1);
  assert.equal(undo(f.view), true);
  assert.equal(readExactSource(f.state()), '');
  f.destroy();
});

test('rich supported post and CDN spellings remain literal before unavailable lookup', async () => {
  for (const link of [
    url + '?tag=blue_eyes',
    'https://danbooru.donmai.us/posts/\u0661\u0662\u0663',
    'https://cdn.donmai.us/original/0123456789abcdef0123456789abcdef.webp',
  ]) {
    const f = booruFixture('', true);
    const pending = f.owner.start(link);
    assert.ok(pending);
    assert.equal(readExactSource(f.state()), link);
    await lookupStarted();
    f.resolve({ status: 'unavailable' });
    await pending;
    assert.equal(readExactSource(f.state()), link);
    assert.equal(undoDepth(f.state()), 1);
    assert.equal(undo(f.view), true);
    assert.equal(readExactSource(f.state()), '');
    f.destroy();
  }
});
test('focus loss at the actual lookup completion boundary refuses publication without waiting for a view notification', async () => {
  const f = booruFixture();
  const pending = f.owner.start(url);
  assert.ok(pending);
  await lookupStarted();
  assert.equal(Reflect.set(f.view, 'hasFocus', false), true);
  f.resolve(booruReply());
  await pending;
  assert.equal(readExactSource(f.state()), url);
  assert.equal(f.signal()?.aborted, true);
  f.destroy();
});

test('completed unavailable missing invalid and empty replies leave one literal event independent of later typing', async () => {
  for (const reply of [
    { status: 'unavailable' },
    { status: 'not-found' },
    { status: 'found', post: { id: -1 } },
    booruReply(''),
  ]) {
    const f = booruFixture();
    const pending = f.owner.start(url);
    assert.ok(pending);
    await lookupStarted();
    f.resolve(reply);
    await pending;
    assert.equal(readExactSource(f.state()), url);
    assert.equal(undoDepth(f.state()), 1);
    f.view.dispatch({
      changes: { from: f.state().doc.length, insert: 'x' },
      userEvent: 'input.type',
    });
    assert.equal(undoDepth(f.state()), 2);
    assert.equal(undo(f.view), true);
    assert.equal(readExactSource(f.state()), url);
    assert.equal(undo(f.view), true);
    assert.equal(readExactSource(f.state()), '');
    f.destroy();
  }
});

test('meta-only post keeps the exact literal paste and its native undo redo event', async () => {
  const f = booruFixture();
  try {
    const pending = f.owner.start(url);
    assert.ok(pending);
    await lookupStarted();
    const reply = booruReply('');
    reply.post.tag_string_meta = 'commentary translation_request';
    f.resolve(reply);
    await pending;
    assert.equal(readExactSource(f.state()), url);
    assert.equal(f.state().selection.main.head, url.length);
    assert.equal(undoDepth(f.state()), 1);
    assert.equal(undo(f.view), true);
    assert.equal(readExactSource(f.state()), '');
    assert.equal(redo(f.view), true);
    assert.equal(readExactSource(f.state()), url);
    assert.equal(f.state().selection.main.head, url.length);
  } finally {
    f.destroy();
  }
});
