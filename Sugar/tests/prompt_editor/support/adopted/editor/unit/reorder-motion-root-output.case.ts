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
/** Actual native mutation queues and appearance stamps share one root-output policy without weakening foreign-input admission. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { ReorderMotionObservation } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-observation.ts';
import {
  motionViewAppearance,
  matchesMotionViewAppearance,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { visibilityObserverFixture } from './reorder-visibility-observer-fixture.ts';

/** One proof owns actual CM state, native subscription transport and primary-first independent retirement. */
function withObservation(
  action: (value: {
    readonly f: ReturnType<typeof motionPresentationFixture>;
    readonly observation: ReorderMotionObservation;
    readonly transport: ReturnType<typeof visibilityObserverFixture>;
    readonly control: { owned: boolean; current: boolean; changed: number };
  }) => void,
): void {
  let f: ReturnType<typeof motionPresentationFixture> | null = null,
    observation: ReorderMotionObservation | null = null;
  const transport: {
      value: ReturnType<typeof visibilityObserverFixture> | null;
    } = { value: null },
    control = { owned: true, current: true, changed: 0 },
    causes: unknown[] = [];
  try {
    f = motionPresentationFixture(undefined, (document) => {
      transport.value = visibilityObserverFixture(document);
    });
    assert.ok(transport.value);
    observation = new ReorderMotionObservation(
      f.view,
      f.state.field(reorderPreview).value!.capture,
      {
        owned: () => control.owned,
        current: () => control.current,
        observe: (action) => action(),
        changed: () => control.changed++,
      },
    );
    observation.start();
    action({ f, observation, transport: transport.value, control });
  } catch (cause) {
    causes.push(cause);
  }
  try {
    if (observation) causes.push(...observation.disconnect());
  } catch (cause) {
    causes.push(cause);
  }
  try {
    f?.destroy();
  } catch (cause) {
    causes.push(cause);
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'root-output-observation-proof-retirement');
}

test('setting and removing each issued root output preserves actual pending admission and the same native appearance', () =>
  withObservation(({ f, observation, transport, control }) => {
    const source = readExactSource(f.state),
      selection = f.state.selection,
      appearance = motionViewAppearance(f.view.dom);
    assert.ok(appearance);
    assert.equal(transport.active, 2);
    for (const name of [
      'data-reorder-grab',
      'data-reorder-motion',
      'data-reorder-motion-issue',
      'data-reorder-motion-scope',
      'data-reorder-motion-coverage',
      'data-reorder-pointer-paint',
    ]) {
      f.view.dom.setAttribute(name, 'true');
      assert.equal(f.view.dom.getAttribute(name), 'true');
      assert.equal(
        observation.cleanPending(() => true),
        true,
      );
      assert.deepEqual(motionViewAppearance(f.view.dom), appearance);
      assert.equal(matchesMotionViewAppearance(f.view.dom, appearance), true);
      f.view.dom.removeAttribute(name);
      assert.equal(f.view.dom.hasAttribute(name), false);
      assert.equal(
        observation.cleanPending(() => true),
        true,
      );
      assert.equal(matchesMotionViewAppearance(f.view.dom, appearance), true);
    }
    control.current = false;
    f.view.dom.setAttribute('data-reorder-grab', 'true');
    transport.deliver();
    f.view.dom.removeAttribute('data-reorder-grab');
    transport.deliver();
    assert.equal(control.changed, 0);
    f.view.dom.setAttribute('data-host-appearance', 'changed');
    assert.equal(matchesMotionViewAppearance(f.view.dom, appearance), false);
    transport.deliver();
    assert.equal(control.changed, 1);
    assert.equal(readExactSource(f.state) === source, true);
    assert.equal(f.state.selection, selection);
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
  }));

test('grab output retains the exact64 record cap, foreign root/content rejection and request-currentness fence', () =>
  withObservation(({ f, observation, transport }) => {
    for (let index = 0; index < 64; index++)
      f.view.dom.setAttribute('data-reorder-grab', String(index));
    assert.equal(
      observation.cleanPending(() => true),
      true,
    );
    for (let index = 0; index < 65; index++)
      f.view.dom.setAttribute('data-reorder-grab', String(index));
    assert.equal(
      observation.cleanPending(() => true),
      false,
    );
    f.view.dom.setAttribute('data-host-appearance', 'changed');
    assert.equal(
      observation.cleanPending(() => true),
      false,
    );
    f.root.setAttribute('data-reorder-grab', 'true');
    f.view.dom.removeAttribute('data-reorder-grab');
    assert.equal(
      observation.cleanPending(() => true),
      false,
    );
    assert.equal(
      observation.cleanPending(() => true),
      true,
    );
    f.view.dom.setAttribute('data-reorder-grab', 'true');
    let reads = 0;
    assert.equal(
      observation.cleanPending(() => ++reads === 1),
      false,
    );
    assert.equal(reads, 2);
    assert.deepEqual(observation.disconnect(), []);
    assert.equal(transport.active, 0);
    f.view.dom.removeAttribute('data-reorder-grab');
    transport.deliver();
    assert.equal(
      observation.cleanPending(() => true),
      false,
    );
  }));
