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
/** Canonical edge paint and controlled native context units prove passive admission without browser geometry claims. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, StateEffect } from '@codemirror/state';
import { Decoration } from '@codemirror/view';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import { canonicalReorderFragment } from '../../../../../../frontend/sugarprompt/editor/src/reorder-projection.ts';
import { EmphasisWidget } from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import {
  currentSourceDisplayProjection,
  installProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { projectWeightInput } from '../../../../../../frontend/sugarprompt/editor/src/exact-weight-projection.ts';
import {
  exactWeightDraft,
  setExactWeightDraft,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-weight-draft.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { weightStateFor } from '../transaction/weight-command-state.ts';
import { ReorderCollisionBase } from '../../../../../../frontend/sugarprompt/editor/src/reorder-collision-base.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { hideReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { fixture, key } from './reorder-session-fixture.ts';

/** Structured value rails retain their original bounded containing-line attributes through exact clipping. */
test('bounded CSV regional line attributes survive a midline canonical edge interval', () => {
  const state = EditorState.create({
      doc: 'before, inside, after',
      extensions: documentKind.of('wildcard-csv'),
    }),
    attrs = { class: 'sp-region-content', style: '--sp-region-color:red' },
    canonical = Decoration.set([Decoration.line({ attributes: attrs }).range(0)]),
    fragment = canonicalReorderFragment(state, canonical, { from: 8, to: 14 }),
    spans: { from: number; to: number; attributes: unknown }[] = [];
  fragment.between(0, state.doc.length, (from, to, value) => {
    spans.push({ from, to, attributes: value.spec.attributes });
  });
  assert.deepEqual(spans, [{ from: 8, to: 14, attributes: attrs }]);
});

/** Prompt full-row regional paint is excluded at the fragment adapter, independently of the row-paint planner. */
test('prompt full-row regional attributes do not become clipped inline marks', () => {
  const state = EditorState.create({
      doc: 'before, inside, after',
      extensions: documentKind.of('prompt'),
    }),
    attrs = { class: 'sp-region-content', style: '--sp-region-color:red' },
    canonical = Decoration.set([Decoration.line({ attributes: attrs }).range(0)]),
    fragment = canonicalReorderFragment(state, canonical, { from: 8, to: 14 }),
    spans: { from: number; to: number; attributes: unknown }[] = [];
  fragment.between(0, state.doc.length, (from, to, value) => {
    spans.push({ from, to, attributes: value.spec.attributes });
  });
  assert.deepEqual(spans, []);
});

/** Existing source-line chrome remains ordinary prepared line paint and retains exact containing-line clipping. */
test('nonregional source-line attributes retain the exact midline canonical interval', () => {
  const state = EditorState.create({
      doc: 'before, inside, after',
      extensions: documentKind.of('prompt'),
    }),
    attrs = { class: 'sp-source-line' },
    canonical = Decoration.set([Decoration.line({ attributes: attrs }).range(0)]),
    fragment = canonicalReorderFragment(state, canonical, { from: 8, to: 14 }),
    spans: { from: number; to: number; attributes: unknown }[] = [];
  fragment.between(0, state.doc.length, (from, to, value) => {
    spans.push({ from, to, attributes: value.spec.attributes });
  });
  assert.deepEqual(spans, [{ from: 8, to: 14, attributes: attrs }]);
});

test('active numeric draft paint leaves the full canonical edge widgets display-only', () => {
  let state = weightStateFor('(outside:1.25), a, b', 16);
  state = state.update({
    effects: StateEffect.appendConfig.of(exactWeightDraft),
  }).state;
  const canonical = prepareProjection(state, state.field(semanticStructures).native!),
    draft = {
      key: Symbol(),
      freshness: state.field(editorFreshness),
      phase: 'editing' as const,
      kind: 'emphasis' as const,
      range: { from: 1, to: 8 },
    };
  state = state.update({ effects: installProjection.of(canonical) }).state;
  state = state.update({ effects: setExactWeightDraft.of(draft) }).state;
  assert.equal(state.field(exactWeightDraft), draft);
  const painted = projectWeightInput(canonical, state),
    live: (symbol | null)[] = [],
    passive: (symbol | null)[] = [];
  painted.decorations.between(0, state.doc.length, (_from, _to, value) => {
    if (value.spec.widget instanceof EmphasisWidget) live.push(value.spec.widget.draftKey);
  });
  canonicalReorderFragment(state, painted.all, { from: 0, to: 14 }).between(
    0,
    state.doc.length,
    (_from, _to, value) => {
      if (value.spec.widget instanceof EmphasisWidget) passive.push(value.spec.widget.draftKey);
    },
  );
  assert.equal(painted.all, canonical.all);
  assert.ok(live.includes(draft.key));
  assert.deepEqual(passive, [null, null]);
  const readonly = currentSourceDisplayProjection(state)!;
  const draftKeys: (symbol | null)[] = [];
  readonly.decorations.between(0, state.doc.length, (_from, _to, value) => {
    if (value.spec.widget instanceof EmphasisWidget) draftKeys.push(value.spec.widget.draftKey);
  });
  assert.equal(readonly.weightDraft, null);
  assert.deepEqual(draftKeys, [null, null]);
});

test('fractional native line width and genuine axis scale stay keyed independently of proposed height', () => {
  const f = fixture(),
    base = new ReorderCollisionBase(f.view);
  const boundary = globalThis as {
      DOMMatrixReadOnly?: typeof DOMMatrixReadOnly;
    },
    previous = boundary.DOMMatrixReadOnly;
  /** The controlled native matrix boundary returns the exact declared transform, independently of rounded dimensions. */
  class Matrix {
    readonly is2D = true;
    readonly a = 1.25;
    readonly d = 1.25;
    readonly b = 0;
    readonly c = 0;
    constructor(value: string) {
      assert.equal(value, 'matrix(1.25,0,0,1.25,0,0)');
    }
  }
  boundary.DOMMatrixReadOnly = Matrix as unknown as typeof DOMMatrixReadOnly;
  try {
    f.owner.keydown(key('Alt'));
    f.measure();
    f.task();
    const admitted = f.state.field(reorderPreview).value!,
      hidden = hideReorderChip(admitted.capture.prepared, admitted.order, admitted.activeChip)!,
      input = {
        token: admitted.token,
        capture: admitted.capture,
        order: admitted.order,
        hidden,
        held: admitted.activeChip,
        pointer: 9,
        coordinates: { x: 55, y: 50 },
      };
    let width = 240.375,
      height = 20,
      top = 10,
      font = '14px Segoe UI';
    const line = f.view.domAtPos(0).node as HTMLElement,
      window = f.view.dom.ownerDocument.defaultView!,
      old = window.getComputedStyle;
    line.getBoundingClientRect = () =>
      ({
        left: 10,
        right: 10 + width * 1.25,
        top,
        bottom: top + height * 1.25,
        width: width * 1.25,
        height: height * 1.25,
      }) as DOMRect;
    window.getComputedStyle = ((node: Element) =>
      node === line
        ? {
            width: `${width}px`,
            height: `${height}px`,
            font,
            boxSizing: 'border-box',
            length: 0,
            getPropertyValue: (name: string) =>
              (
                ({
                  width: `${width}px`,
                  height: `${height}px`,
                  font,
                  'box-sizing': 'border-box',
                  transform: 'matrix(1.25,0,0,1.25,0,0)',
                }) as Record<string, string>
              )[name] ?? '',
          }
        : old(node)) as typeof old;
    assert.equal(base.prepare(input), true);
    const first = base.contextAt(input)!;
    assert.ok(first);
    assert.equal(first.width, 240.375);
    assert.equal(first.scaleX, 1.25);
    assert.equal(first.scaleY, 1.25);
    height = 40;
    assert.equal(base.contextAt(input)!.key, first.key);
    assert.equal(base.contextAt(input)!.paintKey, first.paintKey);
    width = 320.25;
    assert.notEqual(base.contextAt(input)!.key, first.key);
    assert.notEqual(base.contextAt(input)!.paintKey, first.paintKey);
    const wider = base.contextAt(input)!;
    top = 50;
    assert.notEqual(base.contextAt(input)!.key, wider.key);
    assert.equal(base.contextAt(input)!.paintKey, wider.paintKey);
    const scrolled = base.contextAt(input)!;
    font = '18px serif';
    assert.notEqual(base.contextAt(input)!.key, scrolled.key);
    assert.notEqual(base.contextAt(input)!.paintKey, scrolled.paintKey);
  } finally {
    base.destroy();
    f.destroy();
    if (previous) boundary.DOMMatrixReadOnly = previous;
    else delete boundary.DOMMatrixReadOnly;
  }
});
