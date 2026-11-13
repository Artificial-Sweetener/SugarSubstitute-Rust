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
/** Canonical prepared roles admit LoRA feedback independently of semantic candidates hidden by feature or structural presentation. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { RangeSet, StateEffect, type EditorState } from '@codemirror/state';
import { Decoration, type DecorationSet } from '@codemirror/view';
import {
  defaultPreferences,
  resolveFeatureProfile,
  type FeatureProfile,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import {
  SourceWidget,
  type SourceToken,
} from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import { LoraWidget } from '../../../../../../frontend/sugarprompt/editor/src/lora-widget.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import { weightStateFor } from '../transaction/weight-command-state.ts';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import {
  installProjection,
  richProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import {
  activeLoraRange,
  loraActiveDecorations,
  loraHoverAt,
  loraIntent,
  setLoraApplicationFocus,
  setLoraFocus,
  setLoraHover,
  type LoraHover,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-active.ts';
/** Compile the actual feature/structural owner before observing feedback; no test-side projection or grammar decision is recreated. */
function prepared(source: string, profile?: FeatureProfile): EditorState {
  let state = weightStateFor(source, source.length);
  state = state.update({
    effects: StateEffect.appendConfig.of([
      loraIntent,
      ...(profile ? [featureProfile.of(profile)] : []),
    ]),
  }).state;
  return state.update({
    effects: [
      installProjection.of(prepareProjection(state, state.field(semanticStructures).native!)),
      setLoraApplicationFocus.of(true),
    ],
  }).state;
}
/** Current semantic occupancy proves that rejection is canonical presentation, not absent parser input. */
function semanticLora(state: EditorState, from: number, to: number): LoraHover {
  let found: LoraHover | null = null;
  state
    .field(semanticStructures)
    .native!.pointerTargets.between(from, from, (start, end, target) => {
      if (start === from && end === to && target.kind === 'lora')
        found = { from, to, target, freshness: state.field(editorFreshness) };
    });
  assert.ok(found);
  return found;
}

test('disabled emphasis and wildcard syntax remain ordinary caret text eligible for a current hovered LoRA', () => {
  for (const specimen of [
    {
      source: '(body:1.20), <lora:first:1.00>, tail',
      disabled: 'emphasis',
      from: 13,
      to: 30,
      head: 2,
    },
    {
      source: '{file}, <lora:first:1.00>, tail',
      disabled: 'wildcard_syntax',
      from: 8,
      to: 25,
      head: 2,
    },
  ] as const) {
    const enabled = prepared(specimen.source);
    const focused = enabled.update({
      selection: { anchor: specimen.head },
      effects: [setLoraFocus.of(true), setLoraHover.of(loraHoverAt(enabled, specimen.from))],
    }).state;
    assert.equal(activeLoraRange(focused), null);
    const profile = resolveFeatureProfile(
      {
        ...defaultPreferences,
        allowed: {
          ...defaultPreferences.allowed,
          [specimen.disabled]: false,
        },
      },
      { mode: 'library' },
    );
    const state = prepared(specimen.source, profile),
      hover = loraHoverAt(state, specimen.from)!;
    assert.ok(hover);
    assert.deepEqual([hover.from, hover.to], [specimen.from, specimen.to]);
    const active = state.update({
      selection: { anchor: specimen.head },
      effects: [setLoraFocus.of(true), setLoraHover.of(hover)],
    }).state;
    assert.deepEqual(activeLoraRange(active), {
      from: specimen.from,
      to: specimen.to,
    });
    assert.equal(active.field(richProjection).all, state.field(richProjection).all);
  }
});

test('Scene outer projection owns focus while suppressed title syntax cannot invent LoRA hover', () => {
  const source =
    '**One (two:0.80), <lora:title:0.90>, {file}\n(cat:1.20), <lora:body:0.70>, {real}';
  const state = prepared(source),
    hidden = semanticLora(state, 18, 35),
    body = loraHoverAt(state, 56)!;
  assert.ok(body);
  assert.deepEqual([body.from, body.to], [56, 72]);
  assert.equal(loraHoverAt(state, 18), null);
  const refused = state.update({ effects: setLoraHover.of(hidden) }).state;
  assert.equal(refused.field(loraIntent).hover, null);
  assert.equal(activeLoraRange(refused), null);
  for (const head of [8, 23, 39]) {
    const active = state.update({
      selection: { anchor: head },
      effects: [setLoraFocus.of(true), setLoraHover.of(body)],
    }).state;
    const outer: number[][] = [];
    state.field(richProjection).tokens.between(head, head, (from, to) => {
      outer.push([from, to]);
    });
    assert.deepEqual(outer, [[0, 43]]);
    assert.equal(activeLoraRange(active), null);
    const marks: number[][] = [];
    loraActiveDecorations(active).between(0, active.doc.length, (from, to) => {
      marks.push([from, to]);
    });
    assert.deepEqual(marks, []);
    assert.equal(active.field(richProjection).all, state.field(richProjection).all);
  }
  const ordinary = state.update({
    selection: { anchor: 55 },
    effects: [setLoraFocus.of(true), setLoraHover.of(body)],
  }).state;
  assert.deepEqual(activeLoraRange(ordinary), { from: 56, to: 72 });
});

/** Real nested projection indexes are observed at their public point-query boundary, with exact instance methods restored after every attempt. */
test('nested focus selects one actual LoRA token with one token query and one atomic classification while reusing canonical output', () => {
  for (const depth of [8, 64]) {
    const source =
      '('.repeat(depth) +
      '<lora:first:1.00>' +
      ':1.20)'.repeat(depth) +
      ', <lora:second:0.50>, tail';
    const state = prepared(source),
      peerFrom = 7 * depth + 19;
    const hovered = loraHoverAt(state, peerFrom)!;
    assert.ok(hovered);
    assert.deepEqual([hovered.from, hovered.to], [peerFrom, peerFrom + 18]);
    const head = depth + 4;
    const active = state.update({
      selection: { anchor: head },
      effects: [setLoraFocus.of(true), setLoraHover.of(hovered)],
    }).state;
    const projection = active.field(richProjection),
      before = state.field(richProjection);
    assert.equal(projection.all, before.all);
    assert.equal(projection.tokens, before.tokens);
    assert.equal(projection.allAtomic, before.allAtomic);
    assert.equal(projection.tokens.size, depth + 2);
    assert.equal(projection.allAtomic.size, 2 * depth + 2);
    let actualToken: SourceToken | null = null;
    projection.tokens.between(depth, depth, (from, to, token) => {
      if (from === depth && to === depth + 17) actualToken = token;
    });
    assert.ok(actualToken);
    let actualWidget: LoraWidget | null = null;
    projection.allAtomic.between(depth, depth, (from, to, value) => {
      const widget: unknown = value.spec.widget;
      if (
        from === depth &&
        to === depth + 17 &&
        widget instanceof LoraWidget &&
        widget.token === actualToken
      )
        actualWidget = widget;
    });
    assert.ok(actualWidget);
    const tokens = projection.tokens,
      atomic = projection.allAtomic;
    const tokenOwn = Object.getOwnPropertyDescriptor(tokens, 'between'),
      atomicOwn = Object.getOwnPropertyDescriptor(atomic, 'between');
    const tokenBetween = tokens.between.bind(tokens),
      atomicBetween = atomic.between.bind(atomic);
    const tokenPoints: number[][] = [],
      atomicPoints: number[][] = [],
      visitedTokens: SourceToken[] = [],
      visitedWidgets: SourceWidget[] = [],
      causes: unknown[] = [];
    try {
      Object.defineProperty(tokens, 'between', {
        configurable: true,
        /** Delegate the real public query unchanged while recording only its closed point and actual token identities. */
        value(from: number, to: number, callback: Parameters<typeof tokenBetween>[2]) {
          tokenPoints.push([from, to]);
          tokenBetween(from, to, (start, end, token) => {
            visitedTokens.push(token);
            return callback(start, end, token);
          });
        },
      });
      Object.defineProperty(atomic, 'between', {
        configurable: true,
        /** Actual canonical widget identities certify classification rather than a synthetic lookup counter. */
        value(from: number, to: number, callback: Parameters<typeof atomicBetween>[2]) {
          atomicPoints.push([from, to]);
          atomicBetween(from, to, (start, end, value) => {
            const widget: unknown = value.spec.widget;
            if (widget instanceof SourceWidget) visitedWidgets.push(widget);
            return callback(start, end, value);
          });
        },
      });
      for (let attempt = 0; attempt < 2; attempt++) {
        tokenPoints.length = atomicPoints.length = visitedTokens.length = visitedWidgets.length = 0;
        assert.deepEqual(activeLoraRange(active), {
          from: depth,
          to: depth + 17,
        });
        assert.deepEqual(tokenPoints, [[head, head]]);
        assert.deepEqual(atomicPoints, [[depth, depth]]);
        assert.equal(visitedTokens.length, depth + 1);
        assert.ok(visitedTokens.includes(actualToken));
        assert.ok(visitedWidgets.includes(actualWidget));
        assert.ok(visitedWidgets.length <= 2);
        assert.equal(active.field(richProjection), projection);
        assert.equal(active.field(richProjection).decorations, projection.decorations);
      }
      tokenPoints.length = atomicPoints.length = 0;
      const marks: number[][] = [];
      loraActiveDecorations(active).between(0, active.doc.length, (from, to) => {
        marks.push([from, to]);
      });
      assert.deepEqual(marks, [[depth, depth + 17]]);
      assert.deepEqual(tokenPoints, [[head, head]]);
      assert.deepEqual(atomicPoints, [[depth, depth]]);
      assert.equal(active.doc, state.doc);
      assert.deepEqual([active.selection.main.anchor, active.selection.main.head], [head, head]);
    } catch (cause) {
      causes.push(cause);
    }
    for (const retire of [
      () => {
        if (tokenOwn) Object.defineProperty(tokens, 'between', tokenOwn);
        else if (!Reflect.deleteProperty(tokens, 'between'))
          throw new Error('LORA_TOKEN_QUERY_OBSERVER_RETIREMENT_FAILED');
      },
      () => {
        if (atomicOwn) Object.defineProperty(atomic, 'between', atomicOwn);
        else if (!Reflect.deleteProperty(atomic, 'between'))
          throw new Error('LORA_ATOMIC_QUERY_OBSERVER_RETIREMENT_FAILED');
      },
    ]) {
      try {
        retire();
      } catch (cause) {
        causes.push(cause);
      }
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'LORA_NESTED_PROJECTION_PROOF_FAILED');
  }
});

/** The renderer's actual public span iterator supplies whole-widget active marks; a present DecorationSet alone cannot certify ancestry. */
test('an inclusive active mark reaches the actual full-range LoRA widget while the former default mark is excluded', () => {
  const state = prepared('lead, <lora:first:1.00>, tail'),
    hover = loraHoverAt(state, 6)!;
  assert.ok(hover);
  const active = state.update({ effects: setLoraHover.of(hover) }).state;
  assert.deepEqual(activeLoraRange(active), { from: 6, to: 23 });
  const projection = active.field(richProjection),
    marks = loraActiveDecorations(active);
  let actualMark: Decoration | null = null;
  marks.between(6, 23, (from, to, value) => {
    assert.deepEqual([from, to], [6, 23]);
    actualMark = value;
  });
  assert.ok(actualMark);
  /** Actual regular-then-outer rank and default point threshold reproduce CM's widget mark context without another renderer. */
  const context = (outer: DecorationSet) => {
    const witnesses: {
      readonly from: number;
      readonly to: number;
      readonly widget: LoraWidget;
      readonly active: readonly Decoration[];
    }[] = [];
    RangeSet.spans([projection.all, outer], 0, active.doc.length, {
      /** Ordinary text has no bearing on the complete replacement's enclosing marks. */
      span() {},
      /** Retain the genuine canonical widget and exact active mark identities delivered to CM's inline-widget builder. */
      point(from, to, value, covering) {
        const widget: unknown = value.spec.widget;
        if (widget instanceof LoraWidget) witnesses.push({ from, to, widget, active: covering });
      },
    });
    assert.equal(witnesses.length, 1);
    assert.deepEqual([witnesses[0]!.from, witnesses[0]!.to], [6, 23]);
    return witnesses[0]!;
  };
  const old = context(Decoration.set([Decoration.mark({ class: 'sp-lora-active' }).range(6, 23)])),
    current = context(marks);
  assert.deepEqual(old.active, []);
  assert.equal(current.widget, old.widget);
  assert.deepEqual(current.active, [actualMark]);
  assert.equal(current.active[0], actualMark);
  assert.equal(projection.all, state.field(richProjection).all);
  assert.equal(projection.tokens, state.field(richProjection).tokens);
  assert.equal(active.doc, state.doc);
  assert.equal(active.selection, state.selection);
});
