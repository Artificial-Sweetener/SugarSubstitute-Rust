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
/** Native attribute transport characterizes exact appearance matching without another prepared-array allocation. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  motionViewAppearance,
  matchesMotionViewAppearance,
  preparePassiveMotion,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { undoDepth, redoDepth } from '@codemirror/commands';

/** This external boundary supplies only literal Attr operands, never source, layout or editor policy. */
function node(attributes: ArrayLike<Pick<Attr, 'name' | 'value'>>): HTMLElement {
  return { attributes } as unknown as HTMLElement;
}

const issued = [
  'data-reorder-motion',
  'data-reorder-motion-issue',
  'data-reorder-motion-scope',
  'data-reorder-motion-coverage',
  'data-reorder-pointer-paint',
];

test('in-place appearance preserves exact filtered order, ordinary values and the raw64 cap', () => {
  const attributes = [
      { name: 'class', value: 'editor' },
      ...issued.map((name) => ({ name, value: 'issued' })),
      { name: 'style', value: 'color:red' },
    ],
    root = node(attributes),
    prepared = motionViewAppearance(root);
  assert.deepEqual(prepared, [
    ['class', 'editor'],
    ['style', 'color:red'],
  ]);
  assert.ok(prepared);
  assert.equal(Object.isFrozen(prepared), true);
  assert.equal(prepared.every(Object.isFrozen), true);
  assert.equal(matchesMotionViewAppearance(root, prepared), true);
  for (const attribute of attributes.slice(1, -1)) attribute.value = 'changed';
  assert.equal(matchesMotionViewAppearance(root, prepared), true);
  attributes[0]!.value = 'replacement';
  assert.equal(matchesMotionViewAppearance(root, prepared), false);
  attributes[0]!.value = 'editor';
  attributes.reverse();
  assert.equal(matchesMotionViewAppearance(root, prepared), false);
  assert.equal(matchesMotionViewAppearance(node([]), []), true);
  const bounded = Array.from({ length: 64 }, () => ({
    name: issued[0]!,
    value: '',
  }));
  assert.equal(matchesMotionViewAppearance(node(bounded), []), true);
  bounded.push({ name: issued[0]!, value: '' });
  assert.equal(motionViewAppearance(node(bounded)), null);
  assert.equal(matchesMotionViewAppearance(node(bounded), []), false);
  assert.equal(matchesMotionViewAppearance(node([]), prepared), false);
});

test('currentness consumes fixed native indices without enumeration, copying or freezing another snapshot', () => {
  const attributes = {
      0: { name: 'class', value: 'editor' },
      1: { name: 'style', value: 'color:red' },
      length: 2,
      [Symbol.iterator](): never {
        assert.fail('appearance-currentness-enumeration');
      },
    },
    prepared = Object.freeze([
      Object.freeze(['class', 'editor'] as const),
      Object.freeze(['style', 'color:red'] as const),
    ]);
  for (let repeat = 0; repeat < 8; repeat++)
    assert.equal(matchesMotionViewAppearance(node(attributes), prepared), true);
  assert.equal(prepared[0]![1], 'editor');
  attributes[1].value = 'color:blue';
  assert.equal(matchesMotionViewAppearance(node(attributes), prepared), false);
});

for (const raw of [null, undefined])
  test(`attribute getter preserves raw ${raw === null ? 'null' : 'undefined'} failure`, () => {
    let caught = false;
    try {
      matchesMotionViewAppearance(
        node([
          {
            name: 'class',
            get value(): string {
              // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
              throw raw;
            },
          },
        ]),
        [['class', 'editor']],
      );
    } catch (cause) {
      caught = true;
      assert.equal(cause, raw);
    }
    assert.equal(caught, true);
  });

for (const phase of ['attribute', 'layer'] as const)
  test(`actual Stage rejects retirement in the final ${phase} native delegate`, () => {
    const f = motionPresentationFixture(),
      failures: unknown[] = [];
    let stage: ReorderPassiveStage | null = null;
    try {
      f.paint.editor.setAttribute('data-appearance-witness', 'fixed');
      const prepared = preparePassiveMotion(f.view, 0);
      assert.ok(prepared);
      stage = new ReorderPassiveStage(f.view, prepared);
      const source = readExactSource(f.state),
        selection = f.state.selection;
      if (phase === 'attribute') {
        const attributes = f.paint.editor.attributes.map((attribute) =>
          attribute.name === 'data-appearance-witness'
            ? {
                name: attribute.name,
                get value(): string {
                  stage!.destroy();
                  return attribute.value;
                },
              }
            : attribute,
        );
        Object.defineProperty(f.paint.editor, 'attributes', {
          configurable: true,
          value: attributes,
        });
      } else {
        const parent = stage.node.parentElement;
        Object.defineProperty(stage.node, 'parentElement', {
          configurable: true,
          get() {
            stage!.destroy();
            return parent;
          },
        });
      }
      assert.equal(stage.current(), false);
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
      assert.equal(redoDepth(f.state), 0);
    } catch (cause) {
      failures.push(cause);
    }
    for (const release of [() => stage?.destroy(), () => f.destroy()])
      try {
        release();
      } catch (cause) {
        failures.push(cause);
      }
    if (failures.length === 1) throw failures[0];
    if (failures.length)
      throw new AggregateError(failures, 'appearance-currentness-proof-retirement');
  });
