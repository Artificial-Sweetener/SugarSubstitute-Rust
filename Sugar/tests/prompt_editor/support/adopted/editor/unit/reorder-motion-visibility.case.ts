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
/** Actual field/Widget metadata transport and exact native old-value records retain canonical resources. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  currentReorderPaintLease,
  currentReorderPaintTargets,
  measuredReorderWidget,
  ReorderWidget,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import {
  setReorderMotionMask,
  releaseReorderMotionMask,
  type ReorderMotionMask,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-mask.ts';
import { prepareReorderVisibilityReceipt } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-visibility-receipt.ts';
import { reorderMotionLocalAttribute } from '../../../../../../frontend/sugarprompt/editor/src/reorder-chip-presentation.ts';
import { reorderPaintIssueName } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-identity.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';

/** The external observer transport records actual supported setter calls, never reads the mask to synthesize writes. */
function fixture() {
  const f = motionPresentationFixture(),
    preview = f.state.field(reorderPreview).value!;
  const targets = currentReorderPaintTargets(f.view)!,
    records: MutationRecord[] = [];
  for (const target of targets.filter((entry) => entry.kind === 'chip')) {
    const node = target.node,
      set = node.setAttribute.bind(node),
      remove = node.removeAttribute.bind(node);
    /** A native attribute mutation contains its actual target/name/old value. */
    const record = (name: string, oldValue: string | null) =>
      records.push({
        type: 'attributes',
        target: node,
        attributeName: name,
        oldValue,
      } as unknown as MutationRecord);
    node.setAttribute = (name, value) => {
      const old = node.getAttribute(name);
      set(name, value);
      record(name, old);
    };
    node.removeAttribute = (name) => {
      const old = node.getAttribute(name);
      remove(name);
      if (old !== null) record(name, old);
    };
  }
  const content = f.view.contentDOM;
  for (const property of ['height', 'flexBasis'] as const) {
    const descriptor = Object.getOwnPropertyDescriptor(content.style, property)!;
    Object.defineProperty(content.style, property, {
      ...descriptor,
      set(value: string) {
        const old = content.getAttribute('style');
        descriptor.set!.call(content.style, value);
        if (content.getAttribute('style') !== old)
          records.push({
            type: 'attributes',
            target: content,
            attributeName: 'style',
            oldValue: old,
          } as unknown as MutationRecord);
      },
    });
  }
  return {
    f,
    preview,
    targets,
    records,
    /** Each test issues real immutable mask state through the actual field and supported Widget callback. */
    mask(ids: readonly number[], scope: object = {}): ReorderMotionMask {
      return { issue: {}, scope, capture: preview.capture, ids };
    },
    /** Native records are delivered once, as an actual observer's takeRecords operation. */
    take() {
      return records.splice(0);
    },
  };
}

test('supported mask publication retains exact canonical nodes targets geometry and source', () => {
  const { f, preview, targets, mask, take } = fixture();
  try {
    const roots = Array.from(f.root.childNodes),
      lease = currentReorderPaintLease(f.view, preview),
      rects = measuredReorderWidget(f.view, preview);
    assert.ok(lease);
    assert.ok(rects);
    const next = mask([1, 2]),
      receipt = prepareReorderVisibilityReceipt(f.view, next, lease);
    assert.ok(receipt);
    f.view.dispatch({ effects: setReorderMotionMask.of(next) });
    assert.equal(receipt(take()), true);
    assert.equal(lease(), true);
    assert.equal(currentReorderPaintTargets(f.view), targets);
    assert.deepEqual(Array.from(f.root.childNodes), roots);
    assert.equal(measuredReorderWidget(f.view, preview), rects);
    assert.equal(f.state.field(reorderPreview).value, preview);
    assert.deepEqual(
      targets
        .filter(
          (entry) => entry.kind === 'chip' && entry.node.hasAttribute(reorderMotionLocalAttribute),
        )
        .map((entry) => entry.index)
        .sort(),
      [1, 2],
    );
    f.view.dispatch({ effects: releaseReorderMotionMask.of(next.issue) });
    assert.equal(lease(), true);
    assert.equal(currentReorderPaintTargets(f.view), targets);
    assert.equal(measuredReorderWidget(f.view, preview), rects);
    assert.ok(targets.every((entry) => !entry.node.hasAttribute(reorderMotionLocalAttribute)));
  } finally {
    f.destroy();
  }
});

test('same-roster new exact issue advances supported metadata with zero writes and rejects old release', () => {
  const { f, preview, mask, take } = fixture();
  try {
    const first = mask([1]),
      lease = currentReorderPaintLease(f.view, preview);
    assert.ok(lease);
    f.view.dispatch({ effects: setReorderMotionMask.of(first) });
    take();
    const second = mask([1], first.scope),
      receipt = prepareReorderVisibilityReceipt(f.view, second, lease);
    assert.ok(receipt);
    f.view.dispatch({ effects: setReorderMotionMask.of(second) });
    const records = take();
    assert.equal(records.length, 2);
    assert.ok(
      records.every(
        (record) => record.target === f.view.contentDOM && record.attributeName === 'style',
      ),
    );
    assert.equal(receipt(records), true);
    f.view.dispatch({ effects: releaseReorderMotionMask.of(first.issue) });
    assert.equal(f.state.field(reorderPreview).mask?.issue, second.issue);
    assert.deepEqual(take(), []);
    assert.equal(lease(), true);
  } finally {
    f.destroy();
  }
});

for (const fault of ['duplicate', 'foreign-class', 'wrong-old-value'] as const)
  test(`exact receipt rejects ${fault} while canonical lease remains current`, () => {
    const { f, preview, targets, mask, take } = fixture();
    try {
      const next = mask([1]),
        lease = currentReorderPaintLease(f.view, preview);
      assert.ok(lease);
      const receipt = prepareReorderVisibilityReceipt(f.view, next, lease);
      assert.ok(receipt);
      f.view.dispatch({ effects: setReorderMotionMask.of(next) });
      const node = targets.find((entry) => entry.kind === 'chip' && entry.index === 1)!.node;
      if (fault === 'duplicate')
        node.setAttribute(reorderMotionLocalAttribute, reorderPaintIssueName(next.scope));
      if (fault === 'foreign-class') node.setAttribute('class', node.className + ' foreign');
      const records = take();
      if (fault === 'wrong-old-value')
        // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
        records[0] = { ...records[0], oldValue: 'foreign' } as MutationRecord;
      assert.equal(receipt(records), false);
      assert.equal(lease(), true);
    } finally {
      f.destroy();
    }
  });

test('nested same-mount visibility callback preserves newer exact descriptor and canonical lease', () => {
  const { f, preview, targets, mask } = fixture();
  try {
    const lease = currentReorderPaintLease(f.view, preview);
    assert.ok(lease);
    const first = mask([1]),
      newer = mask([2]),
      node = targets.find((entry) => entry.kind === 'chip' && entry.index === 1)!.node;
    const set = node.setAttribute.bind(node);
    let nested = false;
    node.setAttribute = (name, value) => {
      set(name, value);
      if (!nested && name === reorderMotionLocalAttribute) {
        nested = true;
        assert.equal(new ReorderWidget(preview, newer).updateDOM(f.root, f.view), true);
      }
    };
    assert.equal(new ReorderWidget(preview, first).updateDOM(f.root, f.view), false);
    assert.equal(lease(), true);
    assert.equal(currentReorderPaintTargets(f.view), targets);
    assert.equal(node.getAttribute(reorderMotionLocalAttribute), null);
    assert.equal(
      targets
        .find((entry) => entry.kind === 'chip' && entry.index === 2)!
        .node.getAttribute(reorderMotionLocalAttribute),
      reorderPaintIssueName(newer.scope),
    );
  } finally {
    f.destroy();
  }
});
test('incomplete canonical issue refuses metadata and CM replacement fallback preserves its newer complete root', () => {
  const { f, preview, targets, mask } = fixture();
  const next = mask([2]),
    replacement = new ReorderWidget(preview, next);
  let root: HTMLElement | null = null,
    entered = false;
  const target = targets.find((entry) => entry.kind === 'chip')!.node;
  const classes = target.className,
    parent = f.root.parentNode;
  assert.ok(parent);
  Object.defineProperty(target, 'className', {
    configurable: true,
    get() {
      if (!entered) {
        entered = true;
        assert.equal(currentReorderPaintTargets(f.view), null);
        assert.equal(currentReorderPaintLease(f.view, preview), null);
        assert.equal(replacement.updateDOM(f.root, f.view), false);
        root = replacement.toDOM(f.view);
        parent.insertBefore(root, f.root);
        assert.equal(root.parentNode, parent);
        f.widget.destroy(f.root);
        f.root.remove();
      }
      return classes;
    },
    set() {
      throw new Error('old-partial-presentation-write');
    },
  });
  try {
    assert.equal(new ReorderWidget(preview, null).updateDOM(f.root, f.view), false);
    assert.equal(entered, true);
    assert.ok(root);
    assert.equal((root as HTMLElement).isConnected, true);
    assert.equal((root as HTMLElement).textContent, preview.output.text);
    const lease = currentReorderPaintLease(f.view, preview);
    assert.ok(lease);
    assert.equal(lease(), true);
    const current = currentReorderPaintTargets(f.view)!;
    assert.equal(
      current
        .find((entry) => entry.kind === 'chip' && entry.index === 2)!
        .node.getAttribute(reorderMotionLocalAttribute),
      reorderPaintIssueName(next.scope),
    );
  } finally {
    if (root) {
      replacement.destroy(root);
      (root as HTMLElement).remove();
    }
    f.destroy();
  }
});
