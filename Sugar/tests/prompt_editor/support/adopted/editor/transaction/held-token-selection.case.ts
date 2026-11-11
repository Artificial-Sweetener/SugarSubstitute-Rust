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
/** Real native state and supported facet styles prove held exposure without browser geometry or another selection owner. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, EditorSelection, EditorState, StateEffect } from '@codemirror/state';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { projectionState } from './projection-state.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import {
  installSemanticStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  installProjection,
  richProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import {
  heldProjection,
  holdProjectionToken,
  releaseProjectionToken,
  retireRichProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/projection-exposure.ts';
import {
  editorFreshness,
  invalidateCommandContext,
} from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  exactReplacement,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { heldTokenFixture, heldTokenPointer, prepareHeldProjection } from './held-token-fixture.ts';

test('public style reveals a whole LoRA, keeps geometry during forward/reverse continuation, and retires on release', () => {
  const fixture = heldTokenFixture();
  try {
    const original = fixture.state.field(richProjection),
      { event, style } = fixture.press();
    fixture.select(style, event);
    assert.deepEqual(
      [fixture.state.selection.main.anchor, fixture.state.selection.main.head],
      [6, 19],
    );
    assert.equal(fixture.state.field(richProjection).decorations.size, 0);
    fixture.select(style, heldTokenPointer(22));
    assert.deepEqual(
      [fixture.state.selection.main.anchor, fixture.state.selection.main.head],
      [6, 25],
    );
    assert.equal(fixture.state.field(richProjection).revealed, null);
    assert.equal(fixture.state.field(richProjection).decorations.size, 0);
    const exposed = fixture.state.field(richProjection);
    fixture.dispatch({ effects: installProjection.of(original) });
    assert.equal(fixture.state.field(richProjection), exposed);
    fixture.select(style, heldTokenPointer(1));
    assert.deepEqual(
      [fixture.state.selection.main.anchor, fixture.state.selection.main.head],
      [19, 0],
    );
    assert.equal(fixture.state.field(richProjection).all, original.all);
    assert.equal(fixture.state.field(richProjection).tokens, original.tokens);
    fixture.document.dispatchEvent(new Event('mouseup'));
    assert.equal(fixture.state.field(heldProjection), null);
    assert.equal(fixture.state.field(richProjection).decorations, original.all);
    assert.equal(readExactSource(fixture.state), 'alpha <lora:real:1>, tail');
    assert.deepEqual([undoDepth(fixture.state), redoDepth(fixture.state)], [0, 0]);
  } finally {
    fixture.destroy();
  }
});

test('release inside the token retains ordinary reveal, while Shift and multiple ranges retain native anchors', () => {
  const fixture = heldTokenFixture(undefined, 2);
  try {
    const { event, style } = fixture.press();
    fixture.select(style, event);
    fixture.document.dispatchEvent(new Event('mouseup'));
    assert.deepEqual(fixture.state.field(richProjection).revealed, {
      from: 6,
      to: 19,
    });
    assert.equal(fixture.state.field(richProjection).decorations.size, 0);
    assert.deepEqual(
      style.get(heldTokenPointer(25), true, false).ranges.map((r) => [r.anchor, r.head]),
      [[2, 25]],
    );
    assert.deepEqual(
      style.get(heldTokenPointer(25), false, true).ranges.map((r) => [r.anchor, r.head]),
      [
        [2, 2],
        [6, 25],
      ],
    );
  } finally {
    fixture.destroy();
  }
});

for (const boundary of [
  'mouseup',
  'pointercancel',
  'mousedown',
  'buttons-zero',
  'window-blur',
  'keyboard',
  'context',
  'host-selection',
  'source',
  'dispose-rendering',
] as const)
  test(`held projection retires at ${boundary} without stale release affecting readmission`, () => {
    const fixture = heldTokenFixture();
    try {
      const { event, style } = fixture.press();
      fixture.select(style, event);
      fixture.select(style, heldTokenPointer(25));
      const first = fixture.state.field(heldProjection)!.session;
      if (boundary === 'buttons-zero') {
        const released = new Event('mousemove');
        Object.assign(released, { buttons: 0 });
        fixture.document.dispatchEvent(released);
      } else if (boundary === 'window-blur') fixture.window.dispatchEvent(new Event('blur'));
      else if (boundary === 'keyboard') fixture.owner.clear();
      else if (boundary === 'context')
        fixture.dispatch({ effects: invalidateCommandContext.of(null) });
      else if (boundary === 'host-selection')
        fixture.dispatch({ selection: EditorSelection.single(0) });
      else if (boundary === 'source') fixture.dispatch(exactReplacement(fixture.state, 0, 1, 'A'));
      else if (boundary === 'dispose-rendering')
        fixture.dispatch({ effects: retireRichProjection.of(null) });
      else fixture.document.dispatchEvent(new Event(boundary));
      assert.equal(fixture.state.field(heldProjection), null);
      fixture.dispatch({
        effects: installSemanticStructure.of({
          purpose: 'native',
          prepared: prepareNativeStructure(fixture.state),
        }),
      });
      fixture.dispatch({
        effects: installProjection.of(
          prepareProjection(fixture.state, prepareNativeStructure(fixture.state)),
        ),
      });
      fixture.dispatch({ selection: { anchor: 0 } });
      const second = fixture.press();
      fixture.select(second.style, second.event);
      assert.notEqual(fixture.state.field(heldProjection)!.session, first);
      fixture.dispatch({ effects: releaseProjectionToken.of(first) });
      assert.ok(fixture.state.field(heldProjection));
    } finally {
      fixture.destroy();
    }
  });

test('prepared publication cannot revive a stale token hold, and teardown removes all passive callbacks before view access', () => {
  const fixture = heldTokenFixture();
  const projection = fixture.state.field(richProjection),
    freshness = fixture.state.field(editorFreshness);
  const intent = holdProjectionToken.of({
    session: Symbol(),
    freshness,
    range: { from: 6, to: 19 },
    prepared: projection,
  });
  fixture.dispatch({ effects: invalidateCommandContext.of(null) });
  fixture.dispatch({ effects: intent });
  assert.equal(fixture.state.field(heldProjection), null);
  assert.deepEqual([fixture.document.active, fixture.window.active], [4, 1]);
  fixture.destroy();
  assert.deepEqual([fixture.document.active, fixture.window.active], [0, 0]);
  for (const event of ['mouseup', 'mousedown', 'pointercancel', 'mousemove'])
    fixture.document.dispatchEvent(new Event(event));
  fixture.window.dispatchEvent(new Event('blur'));
  assert.equal(fixture.owner.style(heldTokenPointer(8)), null);
});

test('raw presentation rejects holds; source changes and read-only replacement retire only derived geometry', () => {
  const mode = new Compartment();
  let state = prepareHeldProjection(projectionState('<lora:real:1>', mode));
  const intent = holdProjectionToken.of({
    session: Symbol(),
    freshness: state.field(editorFreshness),
    range: { from: 0, to: 13 },
    prepared: state.field(richProjection),
  });
  state = state.update({
    effects: [intent, mode.reconfigure(richRendering.of(false))],
  }).state;
  assert.equal(state.field(heldProjection), null);
  assert.equal(state.field(richProjection).all.size, 0);
  assert.equal(readExactSource(state), '<lora:real:1>');
});

for (const boundary of ['source', 'context', 'dispose', 'derived'] as const)
  test(`actual nested ${boundary} publication during public coordinate read preserves admission freshness`, () => {
    const fixture = heldTokenFixture();
    try {
      fixture.duringRead(() => {
        if (boundary === 'source') fixture.dispatch(exactReplacement(fixture.state, 0, 1, 'A'));
        else if (boundary === 'context')
          fixture.dispatch({ effects: invalidateCommandContext.of(null) });
        else if (boundary === 'dispose') fixture.destroy();
        else
          fixture.dispatch({
            effects: installProjection.of(fixture.state.field(richProjection)),
          });
      });
      const style = fixture.admitted();
      assert.equal(style !== null, boundary === 'derived');
      assert.equal(fixture.state.field(heldProjection) !== null, boundary === 'derived');
    } finally {
      fixture.destroy();
    }
  });

for (const [source, kind, hit, from, to] of [
  ['🙂\r\n<lora:real:1>, tail', 'prompt', 4, 3, 16],
  ['left,right\r\n"🙂<lora:real:1>","{animal}"', 'wildcard-csv', 15, 14, 27],
] as const)
  test(`${kind} held source token uses actual prepared logical UTF16 bounds while exact CRLF/Unicode bytes remain native`, () => {
    const fixture = heldTokenFixture(source, 0, kind);
    try {
      fixture.dispatch({
        effects: StateEffect.appendConfig.of(EditorState.readOnly.of(true)),
      });
      const { event, style } = fixture.press(heldTokenPointer(hit));
      fixture.select(style, event);
      assert.deepEqual(
        [fixture.state.selection.main.anchor, fixture.state.selection.main.head],
        [from, to],
      );
      assert.deepEqual(fixture.state.field(heldProjection)?.range, {
        from,
        to,
      });
      assert.equal(readExactSource(fixture.state), source);
      assert.deepEqual([undoDepth(fixture.state), redoDepth(fixture.state)], [0, 0]);
    } finally {
      fixture.destroy();
    }
  });

test('a replacement read-only context retires an active gesture without changing exact source or native history', () => {
  const fixture = heldTokenFixture();
  try {
    const { event, style } = fixture.press();
    fixture.select(style, event);
    fixture.dispatch({
      effects: StateEffect.appendConfig.of(EditorState.readOnly.of(true)),
    });
    assert.equal(fixture.state.field(heldProjection), null);
    assert.equal(readExactSource(fixture.state), 'alpha <lora:real:1>, tail');
    assert.equal(undoDepth(fixture.state), 0);
  } finally {
    fixture.destroy();
  }
});
