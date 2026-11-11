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
/** Accepted native reorder mode owns primary admission; stale or unaccepted mode cannot borrow a past modifier. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, Prec, StateEffect } from '@codemirror/state';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { reorderGrabCursor } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-cursor.ts';
import { nativeInputSettlement } from '../../../../../../frontend/sugarprompt/editor/src/input-settlement.ts';
import { key, pointer } from './reorder-session-fixture.ts';
import { drain, geometryUpdate, pointerTransferFixture } from './reorder-marker-fixture.ts';

/** Preserve ordered raw assertion and fixture-retirement causes while retiring the actual native mode-proof owner once. */
function usingModeAuthorityFixture(
  check: (fixture: ReturnType<typeof pointerTransferFixture>) => void,
): void {
  const fixture = pointerTransferFixture();
  const causes: unknown[] = [];
  try {
    check(fixture);
  } catch (cause) {
    causes.push(cause);
  }
  try {
    fixture.destroy();
  } catch (cause) {
    causes.push(cause);
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'mode-authority-proof-retirement');
}

test('accepted current Alt mode picks the actual chip without repeated pointer Alt and cancels its finite capture/paint without source history', () => {
  usingModeAuthorityFixture((c) => {
    const f = c.f;
    const selection = f.state.selection;
    assert.equal(f.owner.keydown(key('Alt')), true);
    drain(f);
    assert.ok(f.state.field(reorderPreview).value);
    // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
    const press = { ...c.down(1, 7, 20, 45), altKey: false };
    assert.equal(f.owner.pointerdown(press), true);
    assert.deepEqual([...f.captures], [7]);
    assert.equal(f.state.field(reorderPreview).value!.activeChip, 1);
    assert.ok(f.state.field(reorderGrabCursor));
    drain(f);
    assert.equal(f.owner.pointermove({ ...press, clientX: press.clientX + 10 }), true);
    drain(f);
    assert.equal(f.state.field(reorderPreview).value!.presentation, 'pointer');
    assert.equal(f.state.field(reorderPreview).value!.activeChip, 1);
    assert.equal(
      c.nodes.some((node) => node.className.includes('sp-reorder-proxy') && !node.removed),
      true,
    );
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(f.state.selection, selection);
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
    f.owner.keydown(key('Escape'));
    drain(f);
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.captures.size + f.frames.size, 0);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(f.state.field(reorderGrabCursor), null);
    assert.equal(
      c.nodes.every((node) => !node.className.includes('sp-reorder-proxy') || node.removed),
      true,
    );
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(f.state.selection, selection);
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
  });
});

for (const flags of [
  { ctrlKey: true },
  { metaKey: true },
  { isPrimary: false },
  { button: 1 },
] as const)
  test(`accepted mode retains the independent primary/modifier fence ${Object.keys(flags)[0]}`, () => {
    usingModeAuthorityFixture((c) => {
      const f = c.f;
      f.owner.keydown(key('Alt'));
      drain(f);
      const before = f.state;
      // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
      const press = { ...c.down(1), altKey: false, ...flags };
      assert.equal(f.owner.pointerdown(press), false);
      assert.equal(f.state, before);
      assert.equal(f.captures.size, 0);
      assert.equal(f.state.field(reorderGrabCursor), null);
      assert.equal(f.owner.blocksInput(), true);
    });
  });

for (const stage of ['dormant', 'pending', 'alt-graph'] as const)
  test(`unaccepted ${stage} mode cannot admit a primary packet without pointer Alt`, () => {
    usingModeAuthorityFixture((c) => {
      const f = c.f;
      if (stage === 'pending') {
        assert.equal(f.owner.keydown(key('Alt')), true);
        assert.ok(f.jobs.length);
      } else if (stage === 'alt-graph') {
        // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
        assert.equal(f.owner.keydown({ ...key('Alt'), getModifierState: () => true }), false);
      }
      const before = f.state;
      // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
      assert.equal(f.owner.pointerdown({ ...pointer(), altKey: false }), false);
      assert.equal(f.state, before);
      assert.equal(f.captures.size, 0);
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(f.state.field(reorderGrabCursor), null);
    });
  });

for (const retirement of [
  'alt-release',
  'escape',
  'blur',
  'selection',
  'source',
  'read-only',
  'composition',
] as const)
  test(`accepted mode cannot renew pointer authority after ${retirement}`, () => {
    usingModeAuthorityFixture((c) => {
      const f = c.f;
      let composing = false;
      if (retirement === 'composition') {
        const inactive = f.state.facet(nativeInputSettlement);
        f.view.dispatch({
          effects: StateEffect.appendConfig.of(
            nativeInputSettlement.of({
              ...inactive,
              /** The controlled external input boundary changes without a source transaction. */
              hasCompositionInput: () => composing,
            }),
          ),
        });
      }
      f.owner.keydown(key('Alt'));
      drain(f);
      // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
      const press = { ...c.down(1), altKey: false };
      if (retirement === 'alt-release') f.owner.keyup(key('Alt'));
      else if (retirement === 'escape') f.owner.keydown(key('Escape'));
      else if (retirement === 'blur') {
        f.setFocused(false);
        f.notifyUpdate(
          Object.assign(geometryUpdate(f, false, false), {
            focusChanged: true,
          }),
        );
      } else if (retirement === 'selection') f.view.dispatch({ selection: { anchor: 2 } });
      else if (retirement === 'source') f.view.dispatch({ changes: { from: 0, insert: 'x' } });
      else if (retirement === 'read-only')
        f.view.dispatch({
          effects: StateEffect.appendConfig.of(Prec.highest(EditorState.readOnly.of(true))),
        });
      else composing = true;
      drain(f);
      const before = f.state;
      assert.equal(f.owner.pointerdown(press), false);
      assert.equal(f.state, before);
      assert.equal(f.captures.size, 0);
      assert.equal(f.state.field(reorderGrabCursor), null);
      assert.deepEqual(
        [undoDepth(f.state), redoDepth(f.state)],
        [retirement === 'source' ? 1 : 0, 0],
      );
    });
  });

test('Alt release awaiting a real arrow measure rejects a new unmodified press without preventing the reserved commit', () => {
  usingModeAuthorityFixture((c) => {
    const f = c.f;
    f.owner.keydown(key('Alt'));
    drain(f);
    // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
    const press = { ...c.down(1), altKey: false };
    assert.equal(f.owner.keydown(key('ArrowRight')), true);
    assert.ok(f.jobs.length);
    f.owner.keyup(key('Alt'));
    assert.ok(f.state.field(reorderPreview).value);
    const before = f.state;
    assert.equal(f.owner.pointerdown(press), false);
    assert.equal(f.state, before);
    assert.equal(f.captures.size, 0);
    drain(f);
    assert.equal(readExactSource(f.state), 'b, a, c');
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [1, 0]);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(f.owner.blocksInput(), false);
  });
});

test('Alt release awaiting actual pointer placement rejects a new unmodified press and settles only the original held drop', () => {
  usingModeAuthorityFixture((c) => {
    const f = c.f;
    f.owner.keydown(key('Alt'));
    drain(f);
    // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
    const press = { ...c.down(0, 7, 20, 15), altKey: false };
    assert.equal(f.owner.pointerdown(press), true);
    drain(f);
    f.owner.pointermove({ ...press, clientX: 55 });
    assert.ok(f.jobs.length);
    f.owner.keyup(key('Alt'));
    assert.ok(f.state.field(reorderPreview).value);
    const before = f.state;
    // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
    assert.equal(f.owner.pointerdown({ ...c.down(1, 9, 20, 45), altKey: false }), false);
    assert.equal(f.state, before);
    assert.deepEqual([...f.captures], [7]);
    drain(f);
    assert.equal(readExactSource(f.state), 'b, c, a');
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [1, 0]);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.captures.size + f.frames.size, 0);
  });
});

test('a composition observation that retires accepted mode cannot reenter unmodified source admission', () => {
  usingModeAuthorityFixture((c) => {
    const f = c.f;
    let retireOnProbe = false;
    const retiredStates: EditorState[] = [];
    const inactive = f.state.facet(nativeInputSettlement);
    f.view.dispatch({
      effects: StateEffect.appendConfig.of(
        nativeInputSettlement.of({
          ...inactive,
          /** Retire once through the actual injected observation while reporting no composition. */
          hasCompositionInput() {
            if (retireOnProbe) {
              retireOnProbe = false;
              f.owner.close();
              retiredStates.push(f.state);
            }
            return false;
          },
        }),
      ),
    });
    f.owner.keydown(key('Alt'));
    drain(f);
    // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
    const press = { ...c.down(1), altKey: false };
    const source = readExactSource(f.state),
      selection = f.state.selection;
    retireOnProbe = true;
    assert.equal(f.owner.pointerdown(press), false);
    assert.equal(retiredStates.length, 1);
    assert.equal(f.state, retiredStates[0]);
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.captures.size, 0);
    drain(f);
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(f.state.field(reorderGrabCursor), null);
    assert.equal(readExactSource(f.state), source);
    assert.equal(f.state.selection, selection);
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
  });
});

test('a composition observation that releases Alt checks pending-arrow phase after the delegated currentness call', () => {
  usingModeAuthorityFixture((c) => {
    const f = c.f;
    let releaseOnProbe = false;
    const inactive = f.state.facet(nativeInputSettlement);
    f.view.dispatch({
      effects: StateEffect.appendConfig.of(
        nativeInputSettlement.of({
          ...inactive,
          /** Actual Alt release retains only the queued arrow's finite settlement authority. */
          hasCompositionInput() {
            if (releaseOnProbe) {
              releaseOnProbe = false;
              f.owner.keyup(key('Alt'));
            }
            return false;
          },
        }),
      ),
    });
    f.owner.keydown(key('Alt'));
    drain(f);
    // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
    const press = { ...c.down(1), altKey: false };
    f.owner.keydown(key('ArrowRight'));
    assert.ok(f.jobs.length);
    const before = f.state;
    releaseOnProbe = true;
    assert.equal(f.owner.pointerdown(press), false);
    assert.equal(f.state, before);
    assert.equal(f.captures.size, 0);
    drain(f);
    assert.equal(readExactSource(f.state), 'b, a, c');
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [1, 0]);
    assert.equal(f.owner.blocksInput(), false);
    assert.equal(f.state.field(reorderPreview).value, null);
  });
});
