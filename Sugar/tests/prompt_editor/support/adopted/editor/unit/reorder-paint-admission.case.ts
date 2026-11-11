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
/** Independent literal mark/widget artifacts exercise the actual canonical plan's missing-resource admission. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Decoration, type DecorationSet } from '@codemirror/view';
import { ReorderPaintAdmission } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-admission.ts';
import { ReorderLineLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-line-layout.ts';
import { ReorderPaintRetired } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget-block.ts';
import {
  EmphasisWidget,
  SourceToken,
} from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import {
  prepareReorderWidgetPlan,
  type ReorderPaintLine,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget-plan.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { captureReorder, reorderState } from '../transaction/reorder-fixture.ts';
/** Use the real Core plan and independently replace only its first native DecorationSet boundary. */
function specimen(firstPaint: DecorationSet) {
  const capture = captureReorder(reorderState('a, b, c').state),
    preview = prepareReorderPreview(
      { capture, token: {}, activeChip: 0, order: capture.prepared.initial },
      'complete',
    );
  assert.ok(preview);
  const original = prepareReorderWidgetPlan(preview),
    chips = original.blocks.filter((block) => block.kind === 'chip'),
    first = chips[0]!.parts.find((part) => part.kind === 'line'),
    second = chips[1]!.parts.find((part) => part.kind === 'line');
  assert.ok(first?.kind === 'line' && second?.kind === 'line');
  const replacement: ReorderPaintLine = Object.freeze({
    ...first,
    decorations: firstPaint,
  });
  const plan = {
    ...original,
    blocks: original.blocks.map((block) =>
      block.kind === 'chip'
        ? {
            ...block,
            parts: block.parts.map((part) => (part === first ? replacement : part)),
          }
        : block,
    ),
  };
  return { plan, first: replacement, second };
}
for (const kind of ['mark', 'widget'] as const)
  test(
    'actual ' + kind + ' decoration refuses mixed missing construction before any action',
    () => {
      const decoration =
          kind === 'mark'
            ? Decoration.mark({ class: 'proof-mark' })
            : Decoration.replace({
                widget: new EmphasisWidget(new SourceToken(), 'prefix'),
              }),
        decoded = Decoration.set([decoration.range(0, 1)]),
        { plan, first, second } = specimen(decoded),
        admission = new ReorderPaintAdmission(plan, () => true),
        owner = new ReorderLineLayout();
      let action = false;
      assert.equal(admission.allowMissing(second.key), true);
      assert.equal(
        owner.reconcilePrepared(
          [second.key, first.key],
          (key) => admission.allowMissing(key),
          () => {
            action = true;
          },
        ),
        false,
      );
      assert.equal(action, false);
      assert.equal(admission.allowMissing('unknown'), false);
      owner.destroy();
    },
  );
test('canonical factory consumes the exact classified empty artifact without projecting it again', () => {
  const { plan, second } = specimen(
      Decoration.set([Decoration.mark({ class: 'proof-mark' }).range(0, 1)]),
    ),
    empty = second.decorations,
    original = empty.between,
    descriptor = Object.getOwnPropertyDescriptor(empty, 'between');
  let calls = 0;
  Object.defineProperty(empty, 'between', {
    configurable: true,
    value(...args: Parameters<typeof empty.between>) {
      calls++;
      return original.apply(empty, args);
    },
  });
  try {
    const admission = new ReorderPaintAdmission(plan, () => true);
    assert.equal(admission.allowMissing(second.key), true);
    const before = calls,
      artifact = admission.decorations(second);
    assert.equal(artifact.size, 0);
    assert.equal(artifact, empty);
    assert.equal(admission.decorations(second), artifact);
    assert.equal(calls, before);
  } finally {
    if (descriptor) Object.defineProperty(empty, 'between', descriptor);
    else Reflect.deleteProperty(empty, 'between');
  }
});
test('stale request returns typed retirement after both empty admission and actual mark refusal', () => {
  const { plan, first, second } = specimen(
    Decoration.set([Decoration.mark({ class: 'proof-mark' }).range(0, 1)]),
  );
  for (const part of [first, second]) {
    const admission = new ReorderPaintAdmission(plan, () => false);
    assert.throws(() => admission.allowMissing(part.key), ReorderPaintRetired);
  }
});

/** Scene row-edge acquisition is a real owned DOM resource even when a plain body fragment has no rich marks or widgets. */
test('missing Scene edge resources retain the existing fail-closed warm construction contract', () => {
  const f = specimen(Decoration.none),
    first = { ...f.first, rowPaint: { sceneFill: true, region: null } },
    plan = {
      ...f.plan,
      blocks: f.plan.blocks.map((block) =>
        block.kind === 'chip'
          ? {
              ...block,
              parts: block.parts.map((part) => (part === f.first ? first : part)),
            }
          : block,
      ),
    };
  const admission = new ReorderPaintAdmission(plan, () => true);
  assert.equal(admission.allowMissing(first.key), false);
  assert.equal(admission.allowMissing(f.second.key), true);
});

/** Regional-only row edges are owned resources even when no Scene stripe or inline rich widget is present. */
test('missing regional-only canonical row resources refuse warm construction before mutation', () => {
  const f = specimen(Decoration.none),
    first = {
      ...f.first,
      rowPaint: {
        sceneFill: false,
        region: {
          scopeId: 'scope',
          ordinal: 0,
          normal: '#4f6df5',
          hover: 'rgb(158, 176, 255)',
        },
      },
    },
    plan = {
      ...f.plan,
      blocks: f.plan.blocks.map((block) =>
        block.kind === 'chip'
          ? {
              ...block,
              parts: block.parts.map((part) => (part === f.first ? first : part)),
            }
          : block,
      ),
    };
  const admission = new ReorderPaintAdmission(plan, () => true);
  assert.equal(admission.allowMissing(first.key), false);
  assert.equal(admission.allowMissing(f.second.key), true);
});
