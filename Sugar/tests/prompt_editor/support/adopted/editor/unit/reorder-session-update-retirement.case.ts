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
/** The actual Session owner must defer publishing refresh/retirement beyond CM's locked update notification. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection } from '@codemirror/state';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { fixture, key, pointer, dragPointer } from './reorder-session-fixture.ts';
import { drain, geometryUpdate } from './reorder-marker-fixture.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  reorderPreview,
  installReorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import {
  reorderGrabCursor,
  setReorderGrabCursor,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-cursor.ts';
import { retireReorderPublication } from '../../../../../../frontend/sugarprompt/editor/src/reorder-retirement.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';

test('geometry notifications coalesce refresh outside the genuine native update lock', () => {
  const f = fixture();
  try {
    f.owner.keydown(key('Alt'));
    drain(f);
    const source = readExactSource(f.state),
      selection = f.state.selection,
      publications: boolean[] = [];
    const refresh = f.owner.refreshGeometry.bind(f.owner);
    f.owner.refreshGeometry = () => {
      publications.push(f.updating);
      if (f.updating) assert.throws(() => f.view.dispatch({}), /nominal-native-update-reentry/u);
      refresh();
    };
    f.notifyUpdate(geometryUpdate(f, true, false));
    f.notifyUpdate(geometryUpdate(f, false, true));
    assert.deepEqual(publications, []);
    assert.equal(f.tasks.length, 1);
    f.task();
    drain(f);
    assert.deepEqual(publications, [false]);
    assert.equal(readExactSource(f.state), source);
    assert.equal(f.state.selection, selection);
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
    assert.equal(f.owner.blocksInput(), true);
    f.owner.refreshGeometry();
    assert.deepEqual(publications, [false, false]);
    drain(f);
  } finally {
    f.destroy();
  }
});

for (const retirement of ['close', 'destroy', 'replacement'] as const)
  test(`queued update refresh cannot survive ${retirement}`, () => {
    const f = fixture();
    try {
      f.owner.keydown(key('Alt'));
      drain(f);
      const source = readExactSource(f.state),
        selection = f.state.selection,
        publications: boolean[] = [];
      const refresh = f.owner.refreshGeometry.bind(f.owner);
      f.owner.refreshGeometry = () => {
        publications.push(f.updating);
        refresh();
      };
      f.notifyUpdate(geometryUpdate(f, true, true));
      if (retirement === 'destroy') f.owner.destroy();
      else {
        f.owner.close();
        if (retirement === 'replacement') f.owner.keydown(key('Alt'));
      }
      drain(f);
      assert.deepEqual(publications, []);
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
      assert.equal(f.owner.blocksInput(), retirement === 'replacement');
    } finally {
      f.destroy();
    }
  });

test('focus-loss notification invalidates authority immediately but retires native capture outside the update lock', () => {
  const f = fixture();
  try {
    f.owner.pointerdown(pointer());
    f.measure();
    f.task();
    dragPointer(f.owner);
    drain(f);
    assert.equal(f.captures.size, 1);
    const source = readExactSource(f.state),
      selection = f.state.selection,
      retired: boolean[] = [];
    const release = f.view.contentDOM.releasePointerCapture.bind(f.view.contentDOM);
    f.view.contentDOM.releasePointerCapture = (id) => {
      retired.push(f.updating);
      release(id);
    };
    f.setFocused(false);
    f.notifyUpdate(Object.assign(geometryUpdate(f, false, false), { focusChanged: true }));
    assert.equal(f.owner.blocksInput(), false);
    assert.deepEqual(retired, []);
    f.setFocused(true);
    assert.equal(f.owner.keydown(key('Alt')), false);
    drain(f);
    assert.deepEqual(retired, [false]);
    assert.equal(f.captures.size, 0);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(readExactSource(f.state), source);
    assert.equal(f.state.selection, selection);
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
  } finally {
    f.destroy();
  }
});

test('foreign native selection invalidates queued geometry refresh without another publication', () => {
  const f = fixture();
  try {
    f.owner.keydown(key('Alt'));
    drain(f);
    const source = readExactSource(f.state),
      refreshes: boolean[] = [];
    const refresh = f.owner.refreshGeometry.bind(f.owner);
    f.owner.refreshGeometry = () => {
      refreshes.push(f.updating);
      refresh();
    };
    f.notifyUpdate(geometryUpdate(f, true, false));
    f.view.dispatch({ selection: EditorSelection.cursor(1) });
    const selection = f.state.selection;
    assert.equal(f.owner.blocksInput(), false);
    drain(f);
    assert.deepEqual(refreshes, []);
    assert.equal(f.state.selection, selection);
    assert.equal(readExactSource(f.state), source);
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
  } finally {
    f.destroy();
  }
});

for (const locked of [false, true])
  test(`session destruction clears its exact preview and cursor on a retained native view, update locked=${locked}`, () => {
    const f = fixture();
    try {
      f.owner.pointerdown(pointer());
      f.measure();
      f.task();
      dragPointer(f.owner);
      drain(f);
      const source = readExactSource(f.state),
        selection = f.state.selection;
      assert.ok(f.state.field(reorderPreview).value);
      assert.ok(f.state.field(reorderGrabCursor));
      if (locked) f.notifyDestroy();
      else f.owner.destroy();
      assert.equal(f.owner.blocksInput(), false);
      drain(f);
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(f.state.field(reorderGrabCursor), null);
      assert.equal(f.captures.size, 0);
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
    } finally {
      f.destroy();
    }
  });

test('exact deferred preview and cursor retirement preserves independently issued replacements', () => {
  const f = fixture();
  try {
    f.owner.pointerdown(pointer());
    f.measure();
    f.task();
    dragPointer(f.owner);
    drain(f);
    const old = f.state.field(reorderPreview).value!,
      cursor = f.state.field(reorderGrabCursor);
    assert.ok(cursor);
    retireReorderPublication(f.view, old.token, cursor, true);
    const replacement = prepareReorderPreview({ ...old, token: {} }, 'complete'),
      replacementCursor = {};
    assert.ok(replacement);
    f.view.dispatch({
      effects: [installReorderPreview.of(replacement), setReorderGrabCursor.of(replacementCursor)],
    });
    drain(f);
    assert.equal(f.state.field(reorderPreview).value, replacement);
    assert.equal(f.state.field(reorderGrabCursor), replacementCursor);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
    f.view.dispatch({
      effects: [installReorderPreview.of(null), setReorderGrabCursor.of(null)],
    });
  } finally {
    f.destroy();
  }
});

for (const fails of [false, true])
  test(`queued cancel honors reentrant destruction and preserves raw cancellation failure=${fails}`, () => {
    const listeners = new Set<string>(),
      cause = new Error('controlled-cancel-upgrade-failure');
    const f = fixture(undefined, (document) => {
      document.defaultView!.matchMedia = () =>
        ({
          matches: false,
          addEventListener: () => listeners.add('media'),
          removeEventListener: () => listeners.delete('media'),
        }) as unknown as MediaQueryList;
      Object.defineProperty(document, 'fonts', {
        value: {
          addEventListener: () => listeners.add('fonts'),
          removeEventListener: () => listeners.delete('fonts'),
        },
      });
    });
    try {
      f.owner.pointerdown(pointer());
      f.measure();
      f.task();
      dragPointer(f.owner);
      drain(f);
      assert.deepEqual([...listeners], ['media', 'fonts']);
      const source = readExactSource(f.state),
        selection = f.state.selection;
      const release = f.view.contentDOM.releasePointerCapture.bind(f.view.contentDOM);
      f.view.contentDOM.releasePointerCapture = (id) => {
        release(id);
        f.owner.destroy();
        if (fails) throw cause;
      };
      f.setFocused(false);
      f.notifyUpdate(Object.assign(geometryUpdate(f, false, false), { focusChanged: true }));
      assert.equal(f.owner.blocksInput(), false);
      if (fails)
        assert.throws(
          () => f.task(),
          (actual) => actual === cause,
        );
      else f.task();
      drain(f);
      assert.equal(listeners.size, 0);
      assert.equal(f.captures.size + f.frames.size, 0);
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(f.state.field(reorderGrabCursor), null);
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
    } finally {
      f.destroy();
    }
  });
