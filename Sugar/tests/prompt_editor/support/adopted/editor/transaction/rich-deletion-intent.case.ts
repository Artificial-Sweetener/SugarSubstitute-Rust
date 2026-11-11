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
/** Actual native history settles source input before withheld measurement; browser proof owns trusted events and DOM. */
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { EditorState } from '@codemirror/state';
import { redo, undo, undoDepth, redoDepth } from '@codemirror/commands';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  currentUnmeasuredProjection,
  richProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { retireRichProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-exposure.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { deleteGrapheme } from '../../../../../../frontend/sugarprompt/editor/src/input-policy.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import {
  defaultPreferences,
  resolveFeatureProfile,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import { installLoraMedia } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import {
  installSceneErrors,
  prepareSceneErrors,
} from '../../../../../../frontend/sugarprompt/editor/src/scene-errors.ts';
import { deletionFixture } from './rich-deletion-fixture.ts';

/** Actual source replacement and native undo create a new source epoch, even when bytes return to the original specimen. */
function restored(fixture: ReturnType<typeof deletionFixture>, to: number, insert: string) {
  fixture.view.dispatch({
    changes: { from: 0, to, insert },
    userEvent: 'input.host',
  });
  assert.equal(undo(fixture.view), true);
  assert.equal(fixture.state().field(semanticStructures).native, null);
  assert.notEqual(
    fixture.state().field(richProjection).sourceIdentity,
    fixture.state().field(editorFreshness).source,
  );
}

test('history microtask prepares actual current output while painting stays pending, then measured publication reuses that exact populated output', () => {
  const fixture = deletionFixture('(cat:1.20), tail');
  try {
    const original = fixture.state().field(richProjection);
    restored(fixture, 10, 'plain');
    fixture.drain();
    const prepared = currentUnmeasuredProjection(fixture.state());
    assert.ok(prepared);
    assert.notEqual(prepared, original);
    assert.ok(prepared.tokens.size > 0);
    assert.ok(fixture.state().field(semanticStructures).native);
    assert.notEqual(
      fixture.state().field(richProjection).sourceIdentity,
      fixture.state().field(editorFreshness).source,
    );
    assert.equal(undoDepth(fixture.state()), 0);
    assert.equal(redoDepth(fixture.state()), 1);
    fixture.measure();
    fixture.drain();
    assert.equal(fixture.state().field(richProjection), prepared);
    assert.equal(currentUnmeasuredProjection(fixture.state()), null);
  } finally {
    fixture.destroy();
  }
});

test('pending native undo permits source-neutral wrapper reveal, whole selection deletion and exact undo/redo', () => {
  const source = '(cat:1.20), tail',
    fixture = deletionFixture(source);
  try {
    restored(fixture, 10, 'plain');
    fixture.drain();
    fixture.view.dispatch({ selection: { anchor: 10 } });
    assert.equal(deleteGrapheme(fixture.view, false), true);
    assert.equal(fixture.state().doc.toString(), source);
    assert.deepEqual(
      [fixture.state().selection.main.anchor, fixture.state().selection.main.head],
      [0, 10],
    );
    assert.equal(undoDepth(fixture.state()), 0);
    assert.deepEqual(fixture.state().field(richProjection).revealed, {
      from: 0,
      to: 10,
    });
    assert.equal(deleteGrapheme(fixture.view, false), true);
    assert.equal(fixture.state().doc.toString(), ', tail');
    assert.equal(undo(fixture.view), true);
    fixture.drain();
    assert.equal(fixture.state().doc.toString(), source);
    assert.deepEqual(
      [fixture.state().selection.main.anchor, fixture.state().selection.main.head],
      [0, 10],
    );
    assert.equal(redo(fixture.view), true);
    fixture.drain();
    assert.equal(fixture.state().doc.toString(), ', tail');
  } finally {
    fixture.destroy();
  }
});

test('partial slash deletion followed by pending undo removes one complete escaped glyph in both directions', () => {
  for (const [position, forward] of [
    [0, true],
    [2, false],
  ] as const) {
    const source = String.raw`\(cat\)`,
      fixture = deletionFixture(source);
    try {
      restored(fixture, 1, '');
      fixture.drain();
      fixture.view.dispatch({ selection: { anchor: position } });
      assert.equal(deleteGrapheme(fixture.view, forward), true);
      assert.equal(fixture.state().doc.toString(), String.raw`cat\)`);
      assert.equal(undoDepth(fixture.state()), 1);
      assert.equal(undo(fixture.view), true);
      fixture.drain();
      assert.equal(fixture.state().doc.toString(), source);
      assert.equal(undoDepth(fixture.state()), 0);
    } finally {
      fixture.destroy();
    }
  }
});

test('current pending output keeps outer nested reveal and literal regional ownership', () => {
  for (const [source, position, expected] of [
    [String.raw`((e\(lan\):1.20):1.30), tail`, 22, { from: 0, to: 22 }],
    ['[cat]\nbody', 5, null],
  ] as const) {
    const fixture = deletionFixture(source);
    try {
      restored(fixture, position, 'plain');
      fixture.drain();
      fixture.view.dispatch({ selection: { anchor: position } });
      assert.equal(deleteGrapheme(fixture.view, false), true);
      if (expected) {
        assert.equal(fixture.state().doc.toString(), source);
        assert.deepEqual(fixture.state().field(richProjection).revealed, expected);
      } else assert.equal(fixture.state().doc.toString(), '[cat\nbody');
    } finally {
      fixture.destroy();
    }
  }
});

test('raw/profile/read-only retirement cannot retain previously current input projection', () => {
  for (const kind of ['raw', 'profile', 'permission', 'dispose'] as const) {
    const fixture = deletionFixture('(cat:1.20)');
    try {
      restored(fixture, 10, 'plain');
      fixture.drain();
      assert.ok(currentUnmeasuredProjection(fixture.state()));
      fixture.view.dispatch({
        effects:
          kind === 'raw'
            ? fixture.rendering.reconfigure(richRendering.of(false))
            : kind === 'profile'
              ? fixture.profile.reconfigure(
                  featureProfile.of(
                    resolveFeatureProfile(
                      {
                        ...defaultPreferences,
                        allowed: {
                          ...defaultPreferences.allowed,
                          emphasis: false,
                        },
                      },
                      { mode: 'library' },
                    ),
                  ),
                )
              : kind === 'permission'
                ? fixture.permission.reconfigure(EditorState.readOnly.of(true))
                : retireRichProjection.of(null),
      });
      assert.equal(currentUnmeasuredProjection(fixture.state()), null);
      fixture.view.dispatch({ selection: { anchor: 10 } });
      if (kind === 'permission') assert.equal(deleteGrapheme(fixture.view, false), false);
    } finally {
      fixture.destroy();
    }
  }
});

test('held/composition and superseded source prevent history publication; valid following history remains current', () => {
  for (const kind of ['held', 'composition', 'source', 'destroy'] as const) {
    const fixture = deletionFixture('(cat:1.20)');
    try {
      restored(fixture, 10, 'plain');
      if (kind === 'held' || kind === 'composition') fixture.block(kind);
      else if (kind === 'source')
        fixture.view.dispatch({ changes: { from: 0, to: 10, insert: 'next' } });
      else fixture.destroy();
      fixture.drain();
      assert.equal(currentUnmeasuredProjection(fixture.state()), null);
      if (kind === 'source') {
        assert.equal(undo(fixture.view), true);
        fixture.drain();
        const prepared = currentUnmeasuredProjection(fixture.state());
        assert.ok(prepared);
        assert.equal(prepared.sourceIdentity, fixture.state().field(editorFreshness).source);
      }
    } finally {
      fixture.destroy();
    }
  }
});

test('resource and scene publications retire the sole unmeasured output without altering source or history', () => {
  for (const kind of ['media', 'scene'] as const) {
    const fixture = deletionFixture('(cat:1.20)');
    try {
      restored(fixture, 10, 'plain');
      fixture.drain();
      const state = fixture.state(),
        prepared = currentUnmeasuredProjection(state);
      assert.ok(prepared);
      fixture.view.dispatch({
        effects:
          kind === 'media'
            ? installLoraMedia.of({
                freshness: state.field(editorFreshness),
                media: new Map(),
              })
            : installSceneErrors.of(prepareSceneErrors(state.field(editorFreshness).source, [])),
      });
      assert.equal(currentUnmeasuredProjection(fixture.state()), null);
      assert.equal(fixture.state().doc, state.doc);
      assert.equal(fixture.state().selection, state.selection);
      assert.equal(undoDepth(fixture.state()), 0);
    } finally {
      fixture.destroy();
    }
  }
});

test('history preparation fences synchronous observers before and after compilation and accepts a genuine subsequent history sibling', () => {
  for (const after of [0, 1]) {
    const fixture = deletionFixture('(cat:1.20)');
    try {
      restored(fixture, 10, 'plain');
      fixture.reenter(
        () =>
          fixture.view.dispatch({
            changes: { from: 0, to: 10, insert: 'next' },
            userEvent: 'input.host',
          }),
        after,
      );
      fixture.drain();
      assert.equal(currentUnmeasuredProjection(fixture.state()), null);
      assert.equal(fixture.state().field(semanticStructures).native, null);
      assert.equal(undo(fixture.view), true);
      fixture.drain();
      const prepared = currentUnmeasuredProjection(fixture.state());
      assert.ok(prepared);
      assert.equal(prepared.sourceIdentity, fixture.state().field(editorFreshness).source);
    } finally {
      fixture.destroy();
    }
  }
});

test('selected slash backward deletion and held native undo settle before adjacent glyph deletion without measurement', () => {
  for (const signal of ['key', 'blur', 'pointer'] as const) {
    const source = String.raw`\(cat\)`,
      fixture = deletionFixture(source);
    try {
      fixture.view.dispatch({ selection: { anchor: 1, head: 0 } });
      assert.equal(deleteGrapheme(fixture.view, false), true);
      assert.equal(fixture.state().doc.toString(), String.raw`(cat\)`);
      fixture.block('held');
      assert.equal(undo(fixture.view), true);
      fixture.drain();
      assert.equal(currentUnmeasuredProjection(fixture.state()), null);
      fixture.view.dispatch({ selection: { anchor: 0 } });
      fixture.release(signal);
      fixture.drain();
      const prepared = currentUnmeasuredProjection(fixture.state());
      assert.ok(prepared);
      assert.notEqual(
        fixture.state().field(richProjection).sourceIdentity,
        fixture.state().field(editorFreshness).source,
      );
      assert.equal(deleteGrapheme(fixture.view, true), true);
      assert.equal(fixture.state().doc.toString(), String.raw`cat\)`);
      assert.equal(undoDepth(fixture.state()), 1);
      assert.equal(undo(fixture.view), true);
      fixture.drain();
      assert.equal(fixture.state().doc.toString(), source);
      assert.equal(undoDepth(fixture.state()), 0);
    } finally {
      fixture.destroy();
    }
  }
});

test('held history reservations retire on source, permission, profile, resource, composition or disposal before settlement', () => {
  for (const reason of [
    'source',
    'permission',
    'profile',
    'media',
    'composition',
    'dispose',
  ] as const) {
    const fixture = deletionFixture(String.raw`\(cat\)`);
    try {
      restored(fixture, 1, '');
      fixture.block('held');
      fixture.drain();
      if (reason === 'source')
        fixture.view.dispatch({ changes: { from: 0, to: 7, insert: 'next' } });
      else if (reason === 'permission')
        fixture.view.dispatch({
          effects: fixture.permission.reconfigure(EditorState.readOnly.of(true)),
        });
      else if (reason === 'profile')
        fixture.view.dispatch({
          effects: fixture.profile.reconfigure(
            featureProfile.of(resolveFeatureProfile(defaultPreferences, { mode: 'library' })),
          ),
        });
      else if (reason === 'media')
        fixture.view.dispatch({
          effects: installLoraMedia.of({
            freshness: fixture.state().field(editorFreshness),
            media: new Map(),
          }),
        });
      else if (reason === 'composition') fixture.compositionStart();
      else fixture.destroy();
      fixture.release('key');
      fixture.drain();
      assert.equal(currentUnmeasuredProjection(fixture.state()), null);
    } finally {
      fixture.destroy();
    }
  }
});

test('unexpected held-reservation observer failure preserves the cause and releases it before a genuine following undo', () => {
  for (const afterObservations of [0, 1]) {
    const fixture = deletionFixture(String.raw`\(cat\)`),
      cause = new Error('history-observation-failure');
    try {
      fixture.block('held');
      restored(fixture, 1, '');
      fixture.drain();
      fixture.reenter(() => {
        throw cause;
      }, afterObservations);
      fixture.release('key');
      assert.throws(
        () => fixture.drain(),
        (error) => error === cause,
      );
      fixture.release('blur');
      fixture.drain();
      assert.equal(currentUnmeasuredProjection(fixture.state()), null);
      fixture.view.dispatch({
        changes: { from: 0, to: 7, insert: 'next' },
        userEvent: 'input.host',
      });
      assert.equal(undo(fixture.view), true);
      fixture.drain();
      const prepared = currentUnmeasuredProjection(fixture.state());
      assert.ok(prepared);
      assert.equal(prepared.sourceIdentity, fixture.state().field(editorFreshness).source);
    } finally {
      fixture.destroy();
    }
  }
});
