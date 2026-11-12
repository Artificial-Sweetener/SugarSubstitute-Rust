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
/** Actual invocation focus admission is characterized independently of browser timing or the native111 failure chronology. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undo, undoDepth, redoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { menuFocusFixture } from './context-menu-focus-fixture.ts';
import {
  installLoraPresentation,
  prepareLoraPresentation,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';

for (const surface of ['local', 'field'] as const)
  test(`${surface} opening retires prior NativeFocus measurements before they can reveal and close the current menu`, async () => {
    const fixture = menuFocusFixture();
    try {
      const prior = fixture.native.surface.focus();
      const source = readExactSource(fixture.view.state),
        selection = fixture.view.state.selection;
      const before = fixture.counts().reveals;
      const field =
        surface === 'field'
          ? fixture.owner.prepareFieldActions({
              left: 10,
              right: 20,
              top: 30,
              bottom: 40,
            })
          : null;
      if (surface === 'local') assert.equal(fixture.local(), true);
      await Promise.resolve();
      await fixture.native.flush();
      if (field) assert.equal((await field).status, 'ready');
      assert.equal(await prior, false);
      assert.equal(fixture.counts().reveals, before);
      assert.equal(fixture.installed()?.surface, surface);
      assert.equal(readExactSource(fixture.view.state), source);
      assert.equal(fixture.view.state.selection, selection);
      assert.deepEqual([undoDepth(fixture.view.state), redoDepth(fixture.view.state)], [1, 0]);
    } finally {
      fixture.destroy();
    }
  });

test('actual CM Undo retains its independent scroll intent even when a later local invocation retires NativeFocus', async () => {
  const fixture = menuFocusFixture();
  try {
    assert.equal(undo(fixture.view), true);
    assert.equal(fixture.counts().undoScrolls, 1);
    const source = readExactSource(fixture.view.state),
      selection = fixture.view.state.selection;
    assert.equal(fixture.local(), true);
    await Promise.resolve();
    await fixture.native.flush();
    assert.equal(fixture.installed()?.surface, 'local');
    fixture.deliverUndoScroll();
    assert.equal(fixture.installed(), null);
    assert.equal(readExactSource(fixture.view.state), source);
    assert.equal(fixture.view.state.selection, selection);
    assert.deepEqual([undoDepth(fixture.view.state), redoDepth(fixture.view.state)], [0, 1]);
  } finally {
    fixture.destroy();
  }
});

test('actual pointer refocus retains one exact invocation capability for later Undo restoration', async () => {
  const fixture = menuFocusFixture();
  try {
    fixture.native.root.activeElement = fixture.native.external;
    assert.equal(fixture.pointer(), true);
    assert.equal(fixture.view.hasFocus, true);
    assert.equal(fixture.counts().captures, 1);
    const captured = fixture.captured();
    assert.ok(captured);
    assert.equal(fixture.native.surface.currentHandoff(captured), true);
    await Promise.resolve();
    await fixture.native.flush();
    const menu = fixture.installed();
    assert.ok(menu);
    const leaf = menu.text.items.find((item) => item.kind === 'text' && item.action === 'undo');
    assert.ok(leaf && leaf.kind === 'text');
    assert.equal(await fixture.owner.activate(menu, leaf), true);
    await fixture.native.flush();
    assert.equal(fixture.counts().captures, 1);
    assert.equal(fixture.native.surface.currentHandoff(captured), false);
    assert.equal(fixture.installed(), null);
    assert.equal(readExactSource(fixture.view.state), 'abcd');
    assert.deepEqual([undoDepth(fixture.view.state), redoDepth(fixture.view.state)], [0, 1]);
  } finally {
    fixture.destroy();
  }
});

for (const retirement of ['outside', 'destroy', 'geometry-refusal'] as const)
  test(`local pending ${retirement} retires its invocation without later publication`, async () => {
    const fixture = menuFocusFixture();
    try {
      if (retirement === 'geometry-refusal')
        Object.assign(fixture.view, { coordsAtPos: () => null });
      assert.equal(fixture.local(), true);
      const captured = fixture.captured();
      assert.ok(captured);
      if (retirement === 'outside') fixture.outside();
      if (retirement === 'destroy') fixture.owner.destroy();
      await Promise.resolve();
      await fixture.native.flush();
      assert.equal(fixture.installed(), null);
      assert.equal(fixture.native.surface.currentHandoff(captured), false);
      assert.equal(readExactSource(fixture.view.state), 'xabcd');
      assert.deepEqual([undoDepth(fixture.view.state), redoDepth(fixture.view.state)], [1, 0]);
    } finally {
      fixture.destroy();
    }
  });

for (const cause of [null, undefined])
  test(`local measured failure preserves thrown ${cause} and cancels the actual capability`, async () => {
    const fixture = menuFocusFixture();
    try {
      Object.assign(fixture.view, {
        coordsAtPos: () => {
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw cause;
        },
      });
      assert.equal(fixture.local(), true);
      const captured = fixture.captured();
      assert.ok(captured);
      await Promise.resolve();
      await assert.rejects(fixture.native.flush(), (observed) => observed === cause);
      assert.equal(fixture.installed(), null);
      assert.equal(fixture.native.surface.currentHandoff(captured), false);
    } finally {
      fixture.destroy();
    }
  });

test('a newer Surface intent refuses an older pending local publication without cancelling that newer capability', async () => {
  const fixture = menuFocusFixture();
  try {
    assert.equal(fixture.local(), true);
    const older = fixture.captured();
    assert.ok(older);
    const newer = fixture.native.surface.handoff();
    await Promise.resolve();
    await fixture.native.flush();
    assert.equal(fixture.installed(), null);
    assert.equal(fixture.native.surface.currentHandoff(older), false);
    assert.equal(fixture.native.surface.currentHandoff(newer), true);
    newer.cancel();
  } finally {
    fixture.destroy();
  }
});

test('Escape consumes the retained pointer capability once and preserves exact source, directional selection and history', async () => {
  const fixture = menuFocusFixture();
  try {
    fixture.native.root.activeElement = fixture.native.external;
    const selection = fixture.view.state.selection;
    assert.equal(fixture.pointer(), true);
    await Promise.resolve();
    await fixture.native.flush();
    const captured = fixture.captured();
    assert.ok(captured);
    assert.equal(fixture.escape(), true);
    await fixture.native.flush();
    assert.equal(fixture.counts().captures, 1);
    assert.equal(fixture.native.surface.currentHandoff(captured), false);
    assert.equal(await captured.restore(), false);
    assert.equal(fixture.view.hasFocus, true);
    assert.equal(fixture.view.state.selection, selection);
    assert.equal(readExactSource(fixture.view.state), 'xabcd');
    assert.deepEqual([undoDepth(fixture.view.state), redoDepth(fixture.view.state)], [1, 0]);
  } finally {
    fixture.destroy();
  }
});

for (const surface of ['local', 'field'] as const)
  test(`pending ${surface} catalog replacement uses its genuine surface classification rather than common capability presence`, async () => {
    const fixture = menuFocusFixture();
    try {
      const field =
        surface === 'field'
          ? fixture.owner.prepareFieldActions({
              left: 10,
              right: 20,
              top: 30,
              bottom: 40,
            })
          : null;
      if (!field) assert.equal(fixture.local(), true);
      const previous = fixture.captured();
      assert.ok(previous);
      fixture.view.dispatch({
        effects: installLoraPresentation.of(
          prepareLoraPresentation(fixture.view.state, {
            status: 'unavailable',
            resources: [],
          }),
        ),
      });
      // Deliver the owned replacement microtask and its newly queued preparation, not an elapsed settling wait.
      await Promise.resolve();
      await Promise.resolve();
      await fixture.native.flush();
      assert.equal(fixture.native.surface.currentHandoff(previous), false);
      if (field) {
        assert.equal((await field).status, 'cancelled');
        assert.equal(fixture.installed(), null);
      } else {
        assert.equal(fixture.installed()?.surface, 'local');
        assert.equal(fixture.counts().captures, 2);
      }
      assert.equal(readExactSource(fixture.view.state), 'xabcd');
      assert.deepEqual([undoDepth(fixture.view.state), redoDepth(fixture.view.state)], [1, 0]);
    } finally {
      fixture.destroy();
    }
  });
