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
/** The actual Session publishes current canonical rich paint with its native source commit, before any refresh or geometry job. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect, Transaction } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { fixture, key } from '../unit/reorder-session-fixture.ts';
import { drain } from '../unit/reorder-marker-fixture.ts';
import { sceneErrors } from '../../../../../../frontend/sugarprompt/editor/src/scene-errors.ts';
import { loraPresentation } from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import { loraMedia } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import {
  currentSourceDisplayProjection,
  installProjection,
  richProjection,
  richProjectionExtension,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import {
  prepareNativeStructure,
  installSemanticStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { LoraWidget } from '../../../../../../frontend/sugarprompt/editor/src/lora-widget.ts';

test('rich reorder commit retains its canonical LoRA at the new native range without a later refresh', () => {
  const f = fixture(),
    documentDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'document'),
    causes: unknown[] = [];
  try {
    Object.defineProperty(globalThis, 'document', {
      configurable: true,
      value: f.view.dom.ownerDocument,
    });
    f.view.dispatch({
      changes: {
        from: 0,
        to: f.state.doc.length,
        insert: '<lora:real:0.70>, beta, tail',
      },
      selection: { anchor: 0 },
      effects: StateEffect.appendConfig.of([
        sceneErrors,
        loraPresentation,
        loraMedia,
        richRendering.of(true),
        richProjectionExtension,
      ]),
      annotations: Transaction.addToHistory.of(false),
    });
    const semantic = prepareNativeStructure(f.state),
      projection = prepareProjection(f.state, semantic);
    f.view.dispatch({
      effects: [
        installSemanticStructure.of({ purpose: 'native', prepared: semantic }),
        installProjection.of(projection),
      ],
      annotations: Transaction.addToHistory.of(false),
    });
    assert.ok(currentSourceDisplayProjection(f.state));
    const depth = undoDepth(f.state);
    assert.equal(f.owner.keydown(key('Alt')), true);
    f.measure();
    f.task();
    assert.equal(f.owner.keydown(key('ArrowRight')), true);
    drain(f);
    assert.equal(readExactSource(f.state), '<lora:real:0.70>, beta, tail');
    f.owner.keyup(key('Alt'));
    assert.equal(readExactSource(f.state), 'beta, <lora:real:0.70>, tail');
    assert.equal(undoDepth(f.state), depth + 1);
    assert.equal(f.state.selection.main.anchor, 6);
    assert.equal(f.state.selection.main.head, 6);
    assert.equal(
      f.state.field(richProjection).sourceIdentity,
      f.state.field(editorFreshness).source,
    );
    const current = currentSourceDisplayProjection(f.state);
    assert.ok(current);
    const loras: { from: number; to: number; weight: string }[] = [];
    current.decorations.between(0, f.state.doc.length, (from, to, value) => {
      const widget: unknown = value.spec.widget;
      if (widget instanceof LoraWidget) loras.push({ from, to, weight: widget.weight });
    });
    assert.deepEqual(loras, [{ from: 6, to: 22, weight: '0.70' }]);
  } catch (cause) {
    causes.push(cause);
  }
  for (const release of [
    () => f.destroy(),
    () => {
      if (documentDescriptor) Object.defineProperty(globalThis, 'document', documentDescriptor);
      else Reflect.deleteProperty(globalThis, 'document');
    },
  ])
    try {
      release();
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'reorder-commit-projection-proof-cleanup');
});
