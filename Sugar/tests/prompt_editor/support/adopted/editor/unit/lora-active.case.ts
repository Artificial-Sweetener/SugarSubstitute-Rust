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
/** Actual prepared LoRA intent proves token precedence, stale refusal and canonical output reuse without browser claims. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, EditorSelection, EditorState, StateEffect } from '@codemirror/state';
import { undoDepth, redoDepth } from '@codemirror/commands';
import type { EditorView, ViewUpdate } from '@codemirror/view';
import {
  encodeSource,
  documentRangeToSource,
} from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import type { PromptDocumentKind } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  defaultPreferences,
  resolveFeatureProfile,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import {
  exactWeightDraft,
  setExactWeightDraft,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-weight-draft.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { weightStateFor } from '../transaction/weight-command-state.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import {
  richProjection,
  installProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import {
  reorderGrabCursor,
  setReorderGrabCursor,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-cursor.ts';
import {
  activeLoraRange,
  loraActiveDecorations,
  loraHoverAt,
  loraIntent,
  setLoraFocus,
  setLoraApplicationFocus,
  setLoraHover,
  LoraActiveView,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-active.ts';

const source = 'lead, <lora:first:1.00>, <lora:second:0.50>, tail';
const first = { from: 6, to: 23 },
  second = { from: 25, to: 43 };
/** Existing real source/semantic/projection owners supply expected literal intervals; tests install only passive intent. */
function prepared(text = source, kind: PromptDocumentKind = 'prompt'): EditorState {
  const length = encodeSource(text).logical.length;
  let state = weightStateFor(text, length, length, kind);
  state = state.update({
    effects: StateEffect.appendConfig.of([loraIntent, reorderGrabCursor]),
  }).state;
  const structure = state.field(semanticStructures).native!;
  return state.update({
    effects: [
      installProjection.of(prepareProjection(state, structure)),
      setLoraApplicationFocus.of(true),
    ],
  }).state;
}
/** Independent expected spans observe the complete supported mark output, rather than an internal counter. */
function marks(state: EditorState): readonly (readonly number[])[] {
  const spans: number[][] = [];
  loraActiveDecorations(state).between(0, state.doc.length, (from, to, value) => {
    assert.equal(value.spec.class, 'sp-lora-active');
    spans.push([from, to]);
  });
  return spans;
}
/** Source, native selection and both history directions remain untouched by passive publication. */
function conserved(before: EditorState, after: EditorState): void {
  assert.equal(readExactSource(after), readExactSource(before));
  assert.ok(before.selection.eq(after.selection, true));
  assert.deepEqual([undoDepth(after), redoDepth(after)], [0, 0]);
}

test('current title hover publishes one real LoRA mark immediately and departure clears without another frame', () => {
  const before = prepared(),
    hover = loraHoverAt(before, 6)!;
  assert.deepEqual([hover.from, hover.to, hover.target.kind], [6, 23, 'lora']);
  const hovered = before.update({ effects: setLoraHover.of(hover) }).state;
  assert.deepEqual(marks(hovered), [[6, 23]]);
  assert.equal(hovered.field(richProjection).all, before.field(richProjection).all);
  assert.equal(hovered.field(richProjection).decorations, before.field(richProjection).decorations);
  conserved(before, hovered);
  const departed = hovered.update({ effects: setLoraHover.of(null) }).state;
  assert.deepEqual(marks(departed), []);
  conserved(before, departed);
});

test('native caret association gives its focused projection token precedence over another hovered LoRA', () => {
  const state = prepared(),
    hover = loraHoverAt(state, second.from)!;
  for (const selection of [
    EditorSelection.cursor(6, 1),
    EditorSelection.cursor(23, -1),
    EditorSelection.cursor(6, 0),
  ]) {
    const focused = state.update({
      selection: EditorSelection.create([selection]),
      effects: [setLoraHover.of(hover), setLoraFocus.of(true)],
    }).state;
    assert.equal(focused.selection.main.assoc, selection.assoc);
    assert.deepEqual(activeLoraRange(focused), first);
    assert.deepEqual(marks(focused), [[6, 23]]);
  }
  const outside = state.update({
    selection: EditorSelection.create([EditorSelection.cursor(6, -1)]),
    effects: [setLoraHover.of(hover), setLoraFocus.of(true)],
  }).state;
  assert.equal(outside.selection.main.assoc, -1);
  assert.deepEqual(activeLoraRange(outside), second);
  const reset = outside.update({
    selection: { anchor: source.length },
    effects: setLoraHover.of(null),
  }).state;
  assert.equal(activeLoraRange(reset), null);
  assert.deepEqual(marks(reset), []);
});

test('non-LoRA projection focus suppresses LoRA hover while ordinary native text leaves hover eligible', () => {
  const state = prepared('(body:1.20), <lora:first:1.00>, tail'),
    hover = loraHoverAt(state, 13)!;
  assert.ok(hover);
  const focused = state.update({
    selection: { anchor: 2 },
    effects: [setLoraFocus.of(true), setLoraHover.of(hover)],
  }).state;
  assert.equal(activeLoraRange(focused), null);
  const ordinary = focused.update({
    selection: { anchor: state.doc.length },
  }).state;
  assert.deepEqual(activeLoraRange(ordinary), {
    from: hover.from,
    to: hover.to,
  });
});

test('source/context changes reject old index intent, including an unchanged-range late publication', () => {
  const state = prepared(),
    hover = loraHoverAt(state, 6)!;
  const active = state.update({ effects: setLoraHover.of(hover) }).state;
  const context = active.update({
    effects: invalidateCommandContext.of(null),
  }).state;
  assert.equal(context.field(loraIntent).hover, null);
  assert.deepEqual(marks(context), []);
  const late = context.update({ effects: setLoraHover.of(hover) }).state;
  assert.equal(late.field(loraIntent).hover, null);
  const changed = active.update({ changes: { from: 0, insert: 'x' } }).state;
  assert.equal(changed.field(loraIntent).hover, null);
  assert.deepEqual(marks(changed), []);
});

test('read-only, rich-mode and actual pending-grab owners remove passive paint without changing canonical media', () => {
  const permission = new Compartment(),
    presentation = new Compartment(),
    profile = new Compartment();
  let state = prepared();
  state = state.update({
    effects: StateEffect.appendConfig.of([
      permission.of(EditorState.readOnly.of(false)),
      presentation.of(richRendering.of(true)),
      profile.of(featureProfile.of(resolveFeatureProfile(defaultPreferences, { mode: 'library' }))),
    ]),
  }).state;
  const hover = loraHoverAt(state, 6)!,
    active = state.update({
      effects: [setLoraHover.of(hover), setLoraFocus.of(true)],
      selection: { anchor: 6 },
    }).state;
  for (const effect of [
    permission.reconfigure(EditorState.readOnly.of(true)),
    presentation.reconfigure(richRendering.of(false)),
    setReorderGrabCursor.of({}),
    profile.reconfigure(
      featureProfile.of(
        resolveFeatureProfile(
          {
            ...defaultPreferences,
            allowed: { ...defaultPreferences.allowed, lora_syntax: false },
          },
          { mode: 'library' },
        ),
      ),
    ),
  ]) {
    const disabled = active.update({ effects: effect }).state;
    assert.equal(disabled.field(loraIntent).hover, null);
    assert.deepEqual(marks(disabled), []);
    assert.equal(readExactSource(disabled), source);
    assert.deepEqual([undoDepth(disabled), redoDepth(disabled)], [0, 0]);
  }
});

/** The literal native transport exposes only supported state/dispatch/listener boundaries and no browser geometry. */
function transport(failSecondAdd = false, removals: unknown[] = []) {
  let state = prepared(),
    readsAllowed = true,
    additions = 0;
  const listeners = new Map<string, EventListenerOrEventListenerObject>(),
    removed: string[] = [];
  const window = {
    addEventListener(name: string, listener: EventListenerOrEventListenerObject) {
      listeners.set(name, listener);
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      if (++additions === 2 && failSecondAdd) throw null;
    },
    removeEventListener(name: string) {
      removed.push(name);
      listeners.delete(name);
      if (removals.length) throw removals.shift();
    },
  };
  const view = {
    get state() {
      if (!readsAllowed) throw new Error('RETIRED_LORA_STATE_ACCESS');
      return state;
    },
    get hasFocus() {
      if (!readsAllowed) throw new Error('RETIRED_LORA_FOCUS_ACCESS');
      return true;
    },
    dom: { ownerDocument: { defaultView: window, hasFocus: () => true } },
    dispatch(spec: Parameters<EditorView['dispatch']>[0]) {
      state = state.update(spec).state;
    },
  } as unknown as EditorView;
  return {
    view,
    listeners,
    removed,
    retire: () => {
      readsAllowed = false;
    },
    get state() {
      return state;
    },
  };
}

test('native admission and retirement attempt both listener releases and preserve raw primary causes', () => {
  const failing = transport(true, [undefined, 7]);
  assert.throws(
    () => new LoraActiveView(failing.view),
    (cause: unknown) => {
      assert.ok(cause instanceof AggregateError);
      assert.deepEqual(cause.errors, [null, undefined, 7]);
      return true;
    },
  );
  assert.deepEqual(failing.removed, ['blur', 'focus']);
  const f = transport(false, [null, undefined]),
    owner = new LoraActiveView(f.view);
  assert.throws(
    () => owner.destroy(),
    (cause: unknown) => {
      assert.ok(cause instanceof AggregateError);
      assert.deepEqual(cause.errors, [null, undefined]);
      return true;
    },
  );
  assert.deepEqual(f.removed, ['blur', 'focus']);
});

test('late focus and post-mount callbacks cannot access a retired native editor', async () => {
  const f = transport(),
    owner = new LoraActiveView(f.view);
  const causes: unknown[] = [];
  try {
    const focus = f.listeners.get('focus')!,
      blur = f.listeners.get('blur')!;
    const state = f.state,
      hover = loraHoverAt(state, 6)!;
    f.view.dispatch({ effects: setLoraHover.of(hover) });
    owner.update({
      state: f.state,
      geometryChanged: true,
      viewportChanged: false,
    } as ViewUpdate);
    owner.destroy();
    f.retire();
    assert.equal(typeof focus, 'function');
    assert.equal(typeof blur, 'function');
    (focus as EventListener)(new Event('focus'));
    (blur as EventListener)(new Event('blur'));
    await Promise.resolve();
    assert.equal(f.listeners.size, 0);
  } catch (cause) {
    causes.push(cause);
  }
  try {
    owner.destroy();
  } catch (cause) {
    causes.push(cause);
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'LORA_ACTIVE_RETIREMENT_PROOF_FAILED');
});

/** Native logical UTF-16 ranges consume existing source/scalar/scope conversion, independently of exact CRLF bytes. */
test('astral CRLF prefixes and actual TXT/CSV value scopes preserve current LoRA target identity', () => {
  for (const specimen of [
    {
      source: '🧭\r\nlead, <lora:first:1.00>, tail',
      kind: 'prompt',
      logical: [9, 26],
      exact: [10, 27],
    },
    {
      source: 'header\r\n🧭 <lora:first:1.00>\r\nlater',
      kind: 'wildcard-txt',
      logical: [10, 27],
      exact: [11, 28],
    },
    {
      source: 'prompt\r\n"🧭 <lora:first:1.00>, tail"\r\n',
      kind: 'wildcard-csv',
      logical: [11, 28],
      exact: [12, 29],
    },
  ] as const) {
    const text = specimen.source,
      state = prepared(text, specimen.kind),
      hover = loraHoverAt(state, specimen.logical[0])!;
    assert.ok(hover);
    assert.deepEqual([hover.from, hover.to], specimen.logical);
    const encoded = encodeSource(text),
      exact = documentRangeToSource(encoded.logical, encoded.endings, hover);
    assert.deepEqual([exact.from, exact.to], specimen.exact);
    const active = state.update({ effects: setLoraHover.of(hover) }).state;
    assert.deepEqual(marks(active), [specimen.logical]);
    conserved(state, active);
    assert.equal(readExactSource(active), text);
  }
});

test('the actual finite exact-input draft supplies LoRA focus without inheriting numeric wheel authorization', () => {
  let state = prepared();
  state = state.update({
    effects: StateEffect.appendConfig.of(exactWeightDraft),
  }).state;
  const hover = loraHoverAt(state, second.from)!;
  const active = state.update({
    effects: [
      setLoraHover.of(hover),
      setLoraApplicationFocus.of(true),
      setExactWeightDraft.of({
        key: Symbol(),
        phase: 'editing',
        kind: 'lora',
        range: first,
        freshness: state.field(editorFreshness),
      }),
    ],
  }).state;
  assert.deepEqual(activeLoraRange(active), first);
  const awaiting = active.update({
    effects: [
      setLoraFocus.of(false),
      setLoraApplicationFocus.of(true),
      setLoraHover.of(null),
      setExactWeightDraft.of({
        key: Symbol(),
        phase: 'awaiting-commit',
        kind: 'lora',
        range: first,
        freshness: active.field(editorFreshness),
      }),
    ],
  }).state;
  assert.equal(activeLoraRange(awaiting), null);
  assert.deepEqual(marks(awaiting), []);
  conserved(state, awaiting);
  const closed = active.update({ effects: setExactWeightDraft.of(null) }).state;
  assert.deepEqual(activeLoraRange(closed), second);
  conserved(state, closed);
  let emphasis = prepared('(body:1.20), <lora:first:1.00>, tail');
  emphasis = emphasis.update({
    effects: StateEffect.appendConfig.of(exactWeightDraft),
  }).state;
  const editing = emphasis.update({
    effects: [
      setLoraHover.of(loraHoverAt(emphasis, 13)),
      setExactWeightDraft.of({
        key: Symbol(),
        phase: 'editing',
        kind: 'emphasis',
        range: { from: 0, to: 11 },
        freshness: emphasis.field(editorFreshness),
      }),
    ],
  }).state;
  assert.equal(activeLoraRange(editing), null);
  conserved(emphasis, editing);
});

test('a rebuilt focused native view synchronizes from its existing focus owner after mount without a new focus event', async () => {
  const f = transport();
  f.view.dispatch({ selection: { anchor: 6 } });
  const owner = new LoraActiveView(f.view);
  const causes: unknown[] = [];
  try {
    assert.equal(f.state.field(loraIntent).focused, false);
    await Promise.resolve();
    assert.equal(f.state.field(loraIntent).focused, true);
    assert.deepEqual(activeLoraRange(f.state), first);
    f.view.dispatch({ effects: StateEffect.appendConfig.of(exactWeightDraft) });
    f.view.dispatch({
      effects: setExactWeightDraft.of({
        key: Symbol(),
        phase: 'editing',
        kind: 'lora',
        range: first,
        freshness: f.state.field(editorFreshness),
      }),
    });
    const blur = f.listeners.get('blur') as EventListener,
      focus = f.listeners.get('focus') as EventListener;
    blur(new Event('blur'));
    assert.equal(f.state.field(loraIntent).focused, false);
    assert.equal(f.state.field(exactWeightDraft)?.phase, 'editing');
    assert.equal(activeLoraRange(f.state), null);
    focus(new Event('focus'));
    assert.equal(f.state.field(loraIntent).focused, true);
    assert.deepEqual(activeLoraRange(f.state), first);
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
    assert.equal(readExactSource(f.state), source);
  } catch (cause) {
    causes.push(cause);
  }
  try {
    owner.destroy();
  } catch (cause) {
    causes.push(cause);
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'LORA_ACTIVE_FOCUS_PROOF_FAILED');
});

test('superseded post-mount focus work cannot publish against a changed source state', async () => {
  const f = transport(),
    owner = new LoraActiveView(f.view),
    causes: unknown[] = [];
  try {
    f.view.dispatch({ changes: { from: 0, insert: 'x' } });
    await Promise.resolve();
    assert.equal(f.state.field(loraIntent).focused, false);
    assert.equal(activeLoraRange(f.state), null);
    owner.update({
      state: f.state,
      geometryChanged: false,
      viewportChanged: false,
    } as ViewUpdate);
    await Promise.resolve();
    assert.equal(f.state.field(loraIntent).focused, true);
    assert.equal(activeLoraRange(f.state), null);
  } catch (cause) {
    causes.push(cause);
  }
  try {
    owner.destroy();
  } catch (cause) {
    causes.push(cause);
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'LORA_ACTIVE_STALE_FOCUS_PROOF_FAILED');
});
