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
/** Actual immutable CodeMirror state supplies snapshot truth; only native focus/composition observations are finite transports. */
import assert from 'node:assert/strict';
import { mock, test } from 'node:test';
import {
  Compartment,
  EditorState,
  StateEffect,
  Transaction,
  type TransactionSpec,
} from '@codemirror/state';
import { invertedEffects, redo, undo } from '@codemirror/commands';
import { EditorView } from '@codemirror/view';
import type { EditorSnapshot } from '../../../../../../frontend/sugarprompt/editor/src/contracts.ts';
import {
  exactReplacement,
  lineEndings,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { nativeInputSettlement } from '../../../../../../frontend/sugarprompt/editor/src/input-settlement.ts';
import {
  installSemanticStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { retireRichProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-exposure.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import {
  reorderGrabCursor,
  setReorderGrabCursor,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-cursor.ts';
import {
  materializeNativeSnapshot,
  prepareNativeSnapshot,
  sameNativeSnapshot,
} from '../../../../../../frontend/sugarprompt/editor/src/native-snapshot.ts';
import { projectionState } from './projection-state.ts';

/** Native document/selection/history/readiness fields are real; this nominal view allocates no editor, DOM, clock or source buffer. */
function fixture(source = 'a\r\nb\rc\nd') {
  const presentation = new Compartment(),
    permission = new Compartment(),
    kind = new Compartment();
  let state = projectionState(source, presentation),
    focused = true,
    composing = false,
    nativeComposing = false;
  state = state.update({
    effects: StateEffect.appendConfig.of([
      permission.of(EditorState.readOnly.of(false)),
      kind.of([]),
      reorderGrabCursor,
      nativeInputSettlement.of({
        /** Observe the finite native event boundary without replacing CodeMirror state. */
        hasCompositionInput: () => composing,
        hasHeldKey: () => false,
        cancelCompositionInput() {},
        reserveCommand() {
          throw new Error('SNAPSHOT_PROOF_UNEXPECTED_COMMAND');
        },
      }),
      invertedEffects.of((transaction) =>
        transaction.effects.some((effect) => effect.is(setReorderGrabCursor))
          ? [setReorderGrabCursor.of(transaction.startState.field(reorderGrabCursor))]
          : [],
      ),
    ]),
    annotations: Transaction.addToHistory.of(false),
  }).state;
  const boundary: unknown = Object.create(EditorView.prototype);
  if (!(boundary instanceof EditorView)) throw new Error('SNAPSHOT_PROOF_VIEW_BOUNDARY');
  /** Supported dispatch preserves native history effects and source ownership. */
  function dispatch(spec: TransactionSpec | Transaction): void {
    const transaction = spec instanceof Transaction ? spec : state.update(spec);
    assert.equal(transaction.startState === state, true);
    state = transaction.state;
  }
  Object.defineProperties(boundary, {
    state: { get: () => state },
    hasFocus: { get: () => focused },
    compositionStarted: { get: () => nativeComposing },
    dispatch: { value: dispatch },
  });
  return {
    view: boundary,
    /** Observe the real current native state without constructing a parallel source snapshot. */
    state: () => state,
    dispatch,
    presentation,
    permission,
    kind,
    /** Supply the native view's focus observation. */
    focus(value: boolean) {
      focused = value;
    },
    /** Supply the supported input settlement's pre-mutation composition observation. */
    composition(value: boolean) {
      composing = value;
    },
    /** Supply CodeMirror's own composition-start observation independently of the input facet. */
    nativeComposition(value: boolean) {
      nativeComposing = value;
    },
    /** Delegate projection to its actual owner with an explicit host revision. */
    prepare(revision = 7) {
      return prepareNativeSnapshot(boundary, revision);
    },
  };
}

/** Literal public fields are independent of the production metadata projection/key enumeration. */
function expected(source = 'a\r\nb\rc\nd', changes: Partial<EditorSnapshot> = {}): EditorSnapshot {
  return {
    source,
    documentKind: 'prompt',
    anchor: 0,
    head: 0,
    documentAnchor: 0,
    documentHead: 0,
    revision: 7,
    composing: false,
    focused: true,
    readOnly: false,
    undoDepth: 0,
    redoDepth: 0,
    richRendering: true,
    semantics: 'pending',
    rendering: 'pending',
    ...changes,
  };
}

/** Source equality remains boolean-only on failure; the independent public metadata oracle contains no source payload. */
function assertSnapshot(actual: EditorSnapshot, expected: EditorSnapshot): void {
  const { source: actualSource, ...actualMetadata } = actual;
  const { source: expectedSource, ...expectedMetadata } = expected;
  assert.equal(actualSource === expectedSource, true);
  assert.deepEqual(actualMetadata, expectedMetadata);
}

test('metadata projection and comparison avoid native source serialization; repeated queries cannot consume a later materialization', () => {
  const f = fixture(),
    first = f.prepare(),
    document = f.state().doc,
    token = {};
  f.dispatch({
    effects: setReorderGrabCursor.of(token),
    annotations: Transaction.addToHistory.of(false),
  });
  assert.equal(f.state().field(reorderGrabCursor) === token, true);
  const nativeString = document.toString.bind(document);
  let reads = 0;
  const serialization = mock.method(document, 'toString', () => {
    reads++;
    return nativeString();
  });
  try {
    const next = f.prepare();
    for (let at = 0; at < 4; at++) assert.equal(sameNativeSnapshot(first, f.prepare()), true);
    assert.equal(reads, 0);
    assert.equal(next.document === document, true);
    assert.equal(next.endings === f.state().field(lineEndings), true);
    assertSnapshot(materializeNativeSnapshot(next), expected());
    assert.equal(reads, 1);
    assertSnapshot(materializeNativeSnapshot(next), expected());
    assert.equal(reads, 2);
    assert.equal(sameNativeSnapshot(first, next), true);
  } finally {
    serialization.mock.restore();
  }
});

test('directional logical selection maps independently to exact CRLF/CR/LF source UTF16', () => {
  const f = fixture(),
    before = f.prepare();
  f.dispatch({
    selection: { anchor: 7, head: 2 },
    annotations: Transaction.addToHistory.of(false),
  });
  const after = f.prepare();
  assert.equal(sameNativeSnapshot(before, after), false);
  assertSnapshot(
    materializeNativeSnapshot(after),
    expected(undefined, {
      anchor: 8,
      head: 3,
      documentAnchor: 7,
      documentHead: 2,
    }),
  );
});

test('genuine newline metadata rewrites invalidate while the native logical document identity is unchanged', () => {
  const f = fixture('a\nb'),
    document = f.state().doc;
  let previous = f.prepare();
  for (const source of ['a\rb', 'a\r\nb', 'a\nb']) {
    const replacement = exactReplacement(
      f.state(),
      0,
      materializeNativeSnapshot(previous).source.length,
      source,
    );
    f.dispatch({
      effects: replacement.effects,
      annotations: Transaction.addToHistory.of(false),
    });
    const next = f.prepare();
    assert.equal(f.state().doc === document, true);
    assert.equal(sameNativeSnapshot(previous, next), false);
    assertSnapshot(materializeNativeSnapshot(next), expected(source));
    previous = next;
  }
});

test('source replacement invalidates when source alone changes in the public observation', () => {
  const f = fixture('old'),
    before = f.prepare(),
    endings = f.state().field(lineEndings);
  f.dispatch({
    ...exactReplacement(f.state(), 0, 3, 'new'),
    selection: { anchor: 0 },
    annotations: Transaction.addToHistory.of(false),
  });
  const next = f.prepare();
  assert.equal(f.state().doc === before.document, false);
  assert.equal(f.state().field(lineEndings) === endings, true);
  assert.equal(sameNativeSnapshot(before, next), false);
  assertSnapshot(materializeNativeSnapshot(next), expected('new'));
});

test('native history-only events and Undo/Redo invalidate without changing source/document/selection', () => {
  const f = fixture('a'),
    before = f.prepare(),
    document = f.state().doc;
  f.dispatch({ effects: setReorderGrabCursor.of({}) });
  const changed = f.prepare();
  assert.equal(f.state().doc === document, true);
  assert.equal(sameNativeSnapshot(before, changed), false);
  assertSnapshot(materializeNativeSnapshot(changed), expected('a', { undoDepth: 1 }));
  assert.equal(undo(f.view), true);
  const undone = f.prepare();
  assert.equal(f.state().doc === document, true);
  assert.equal(sameNativeSnapshot(changed, undone), false);
  assertSnapshot(materializeNativeSnapshot(undone), expected('a', { redoDepth: 1 }));
  assert.equal(redo(f.view), true);
  assertSnapshot(materializeNativeSnapshot(f.prepare()), expected('a', { undoDepth: 1 }));
});

test('native focus and composition-start observations invalidate before any document transaction', () => {
  const f = fixture('a'),
    before = f.prepare(),
    state = f.state();
  f.focus(false);
  const blurred = f.prepare();
  assert.equal(sameNativeSnapshot(before, blurred), false);
  assertSnapshot(materializeNativeSnapshot(blurred), expected('a', { focused: false }));
  f.composition(true);
  const composing = f.prepare();
  assert.equal(sameNativeSnapshot(blurred, composing), false);
  assertSnapshot(
    materializeNativeSnapshot(composing),
    expected('a', {
      focused: false,
      composing: true,
      semantics: 'composing',
      rendering: 'composing',
    }),
  );
  f.nativeComposition(true);
  f.composition(false);
  assert.equal(sameNativeSnapshot(composing, f.prepare()), true);
  assertSnapshot(
    materializeNativeSnapshot(f.prepare()),
    expected('a', {
      focused: false,
      composing: true,
      semantics: 'composing',
      rendering: 'composing',
    }),
  );
  f.nativeComposition(false);
  f.composition(false);
  f.focus(true);
  assert.equal(f.state() === state, true);
  assert.equal(sameNativeSnapshot(before, f.prepare()), true);
});

test('readOnly, document kind, rich/raw intent and host revision each publish their actual public fields', () => {
  const f = fixture('a');
  const transitions: [() => void, Partial<EditorSnapshot>][] = [
    [
      () =>
        f.dispatch({
          effects: f.permission.reconfigure(EditorState.readOnly.of(true)),
        }),
      { readOnly: true },
    ],
    [
      () =>
        f.dispatch({
          effects: f.kind.reconfigure(documentKind.of('wildcard-txt')),
        }),
      { readOnly: true, documentKind: 'wildcard-txt' },
    ],
    [
      () =>
        f.dispatch({
          effects: f.presentation.reconfigure(richRendering.of(false)),
        }),
      {
        readOnly: true,
        documentKind: 'wildcard-txt',
        richRendering: false,
        rendering: 'raw',
      },
    ],
  ];
  let previous = f.prepare();
  for (const [apply, fields] of transitions) {
    apply();
    const next = f.prepare();
    assert.equal(sameNativeSnapshot(previous, next), false);
    assertSnapshot(materializeNativeSnapshot(next), expected('a', fields));
    previous = next;
  }
  assert.equal(sameNativeSnapshot(previous, f.prepare(8)), false);
  assertSnapshot(
    materializeNativeSnapshot(f.prepare(8)),
    expected('a', {
      readOnly: true,
      documentKind: 'wildcard-txt',
      richRendering: false,
      rendering: 'raw',
      revision: 8,
    }),
  );
});

test('actual independent semantic and measured-render readiness publish without source/selection/history changes', () => {
  const f = fixture('(a:1.20)'),
    pending = f.prepare(),
    document = f.state().doc;
  const semantic = prepareNativeStructure(f.state());
  f.dispatch({
    effects: installSemanticStructure.of({
      purpose: 'native',
      prepared: semantic,
    }),
    annotations: Transaction.addToHistory.of(false),
  });
  const semanticReady = f.prepare();
  assert.equal(sameNativeSnapshot(pending, semanticReady), false);
  assertSnapshot(
    materializeNativeSnapshot(semanticReady),
    expected('(a:1.20)', { semantics: 'ready' }),
  );
  f.dispatch({
    effects: installProjection.of(prepareProjection(f.state(), semantic)),
    annotations: Transaction.addToHistory.of(false),
  });
  const ready = f.prepare();
  assert.equal(sameNativeSnapshot(semanticReady, ready), false);
  assertSnapshot(
    materializeNativeSnapshot(ready),
    expected('(a:1.20)', { semantics: 'ready', rendering: 'ready' }),
  );
  f.dispatch({
    effects: retireRichProjection.of(null),
    annotations: Transaction.addToHistory.of(false),
  });
  const retired = f.prepare();
  assert.equal(sameNativeSnapshot(ready, retired), false);
  assertSnapshot(materializeNativeSnapshot(retired), expected('(a:1.20)', { semantics: 'ready' }));
  assert.equal(f.state().doc === document, true);
});
