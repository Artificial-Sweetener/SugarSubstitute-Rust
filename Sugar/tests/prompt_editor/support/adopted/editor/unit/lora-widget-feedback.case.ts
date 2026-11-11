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
/** Actual immutable feedback descriptors update only their existing native widget class; geometry remains a native browser proof. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect, type EditorState } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { LoraWidget } from '../../../../../../frontend/sugarprompt/editor/src/lora-widget.ts';
import { SourceToken } from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import type { PreparedLoraMedia } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-contracts.ts';
import {
  wheelFeedback,
  showWheelFeedback,
} from '../../../../../../frontend/sugarprompt/editor/src/wheel-feedback.ts';
import {
  installWeightSession,
  retireWeightSession,
} from '../../../../../../frontend/sugarprompt/editor/src/weight-session.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  richProjection,
  installProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { weightStateFor } from '../transaction/weight-command-state.ts';

/** Literal finite DOM transport retains actual widget children and mirrors only native class token operations. */
class PaintElement {
  className = '';
  textContent = '';
  title = '';
  width = 0;
  height = 0;
  readonly dataset: Record<string, string> = {};
  readonly attributes = new Map<string, string>();
  readonly children: PaintElement[] = [];
  readonly writes: string[] = [];
  readonly classList = {
    /** A native toggle retains every unrelated class and the complete original child graph. */
    toggle: (name: string, force: boolean): boolean => {
      this.writes.push(name);
      const tokens = new Set(this.className.split(/\s+/u).filter(Boolean));
      if (force) tokens.add(name);
      else tokens.delete(name);
      this.className = [...tokens].join(' ');
      return force;
    },
  };
  /** Record literal emitted accessibility and role attributes. */
  setAttribute(name: string, value: string): void {
    this.attributes.set(name, value);
  }
  /** Retain actual constructor children rather than copying a renderer. */
  append(...children: PaintElement[]): void {
    this.children.push(...children);
  }
}

/** Read the actual prepared replacement descriptor rather than constructing an expected feedback widget. */
function widget(state: EditorState): LoraWidget {
  const found: LoraWidget[] = [];
  state.field(richProjection).all.between(0, state.doc.length, (_from, _to, value) => {
    if (value.spec.widget instanceof LoraWidget) found.push(value.spec.widget);
  });
  assert.equal(found.length, 1);
  return found[0]!;
}

/** One fake document allocates only literal nodes; restoration preserves raw body and cleanup causes. */
function withDocument(body: () => void): void {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'document');
  const causes: unknown[] = [];
  try {
    Object.defineProperty(globalThis, 'document', {
      configurable: true,
      value: { createElement: () => new PaintElement() },
    });
    body();
  } catch (cause) {
    causes.push(cause);
  }
  try {
    if (descriptor) Object.defineProperty(globalThis, 'document', descriptor);
    else Reflect.deleteProperty(globalThis, 'document');
  } catch (cause) {
    causes.push(cause);
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'LORA_WIDGET_FEEDBACK_PROOF_RETIREMENT');
}

test('real wheel/pointer feedback and retirement reuse the existing child graph without source/history changes', () => {
  withDocument(() => {
    let state = weightStateFor('<lora:real:0.50>');
    state = state.update({
      effects: StateEffect.appendConfig.of(wheelFeedback),
    }).state;
    state = state.update({
      effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
    }).state;
    let previous = widget(state);
    const dom = previous.toDOM({} as EditorView),
      node = dom as unknown as PaintElement;
    node.className += ' external-host-class';
    const children = [...node.children],
      descendants = children.flatMap((child) => child.children),
      attributes = [...node.attributes];
    const freshness = state.field(editorFreshness),
      range = { from: 0, to: 16 };
    for (const [effect, expected] of [
      [showWheelFeedback.of({ kind: 'lora', range, freshness }), 'wheel'],
      [
        installWeightSession.of({
          owner: 'pointer',
          kind: 'lora',
          range,
          freshness,
          boundary: 'end',
          neutral: false,
        }),
        'pointer',
      ],
      [retireWeightSession.of(null), 'wheel'],
      [showWheelFeedback.of(null), 'none'],
    ] as const) {
      state = state.update({ effects: effect }).state;
      const current = widget(state);
      assert.equal(current.adjustment, expected);
      assert.equal(current.eq(previous), false);
      assert.equal(current.updateDOM(dom, {} as EditorView, previous), true);
      assert.equal(
        node.className,
        'sp-lora-chip external-host-class' +
          (expected === 'wheel'
            ? ' sp-wheel-ready'
            : expected === 'pointer'
              ? ' sp-adjustment-content'
              : ''),
      );
      assert.ok(node.children.every((child, index) => child === children[index]));
      assert.ok(
        node.children
          .flatMap((child) => child.children)
          .every((child, index) => child === descendants[index]),
      );
      assert.deepEqual([...node.attributes], attributes);
      assert.equal(state.doc.toString(), '<lora:real:0.50>');
      assert.deepEqual([undoDepth(state), redoDepth(state)], [0, 0]);
      previous = current;
    }
  });
});

test('prepared ready pixels are painted once and retained through adjustment-only native publication', () => {
  withDocument(() => {
    const painted: HTMLCanvasElement[] = [];
    let disposed = 0;
    const media: PreparedLoraMedia = {
      status: 'ready',
      raster: {
        width: 16,
        height: 8,
        /** Only initial canonical construction consumes the prepared raster. */
        paint(canvas) {
          painted.push(canvas);
        },
        /** Widget feedback never owns pool disposal. */
        dispose() {
          disposed++;
        },
      },
    };
    const original = new LoraWidget(
      new SourceToken(),
      { name: 'real', resolution: 'unavailable' },
      '0.50',
      'none',
      media,
    );
    const dom = original.toDOM({} as EditorView),
      node = dom as unknown as PaintElement;
    const canvas = node.children[0]!.children[0];
    let previous = original;
    for (const adjustment of ['wheel', 'pointer', 'none'] as const) {
      const next: LoraWidget = new LoraWidget(
        original.token,
        original.resource,
        original.weight,
        adjustment,
        media,
      );
      assert.equal(next.updateDOM(dom, {} as EditorView, previous), true);
      assert.equal(node.children[0]!.children[0], canvas);
      previous = next;
    }
    assert.equal(painted.length, 1);
    assert.equal(Object.is(painted[0], canvas), true);
    assert.deepEqual([canvas!.width, canvas!.height], [16, 8]);
    assert.equal(disposed, 0);
  });
});

test('content, media and draft identity changes refuse without changing the mounted class or descendants', () => {
  withDocument(() => {
    const original = new LoraWidget(
      new SourceToken(),
      { name: 'real', resolution: 'unavailable' },
      '0.50',
    );
    const dom = original.toDOM({} as EditorView),
      node = dom as unknown as PaintElement,
      children = [...node.children];
    for (const changed of [
      new LoraWidget(original.token, original.resource, '0.75', 'wheel', original.preparedMedia),
      new LoraWidget(
        original.token,
        { name: 'different', resolution: 'unavailable' },
        original.weight,
        'wheel',
        original.preparedMedia,
      ),
      new LoraWidget(original.token, original.resource, original.weight, 'wheel', {
        status: 'pending',
      }),
      new LoraWidget(
        original.token,
        original.resource,
        original.weight,
        'wheel',
        original.preparedMedia,
        Symbol(),
      ),
    ]) {
      assert.equal(changed.updateDOM(dom, {} as EditorView, original), false);
      assert.equal(node.className, 'sp-lora-chip');
      assert.deepEqual(node.writes, []);
      assert.ok(node.children.every((child, index) => child === children[index]));
    }
  });
});
