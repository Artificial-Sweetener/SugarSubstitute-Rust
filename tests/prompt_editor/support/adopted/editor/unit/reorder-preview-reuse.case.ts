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
import { nativeReorderLineClass } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-targets.ts';
import { drain } from './reorder-marker-fixture.ts';
/** Actual session/state/widget boundaries reuse identical display while retaining the latest authoritative publication. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { ChangeSet, RangeSet, Transaction } from '@codemirror/state';
import { EditorView, WidgetType } from '@codemirror/view';
import {
  installReorderPreview,
  reorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import type { ReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-contracts.ts';
import { captureReorder } from '../transaction/reorder-fixture.ts';
import { dragPointer, fixture, key, pointer } from './reorder-session-fixture.ts';

/** The actual field's decoration carries the genuine private widget, without another rendering rule. */
function projected(f: ReturnType<typeof fixture>) {
  const field = f.state.field(reorderPreview);
  assert.equal(field.native, false);
  const widgets: WidgetType[] = [];
  field.decorations.between(0, f.state.doc.length, (_from, _to, value) => {
    if (value.spec.widget instanceof WidgetType) widgets.push(value.spec.widget);
  });
  assert.ok(field.value);
  assert.equal(widgets.length, 1);
  return {
    value: field.value,
    decorations: field.decorations,
    widget: widgets[0]!,
  };
}

/** Genuine initial admission paints marks over native ranges without a replacement widget. */
function initialMarks(f: ReturnType<typeof fixture>) {
  const field = f.state.field(reorderPreview),
    chips: { from: number; to: number; id: string; active: boolean }[] = [],
    carriers: { from: number; to: number }[] = [],
    underlayLines: number[] = [];
  assert.equal(field.native, true);
  assert.ok(field.value);
  field.decorations.between(0, f.state.doc.length, (from, to, value) => {
    assert.equal(value.spec.widget, undefined);
    if (value.spec.class === nativeReorderLineClass(String(value.spec.nativeReorderValue))) {
      assert.equal(from, to);
      assert.equal(from, f.state.doc.lineAt(from).from);
      assert.equal(value.spec.attributes?.['data-reorder-chip'], undefined);
      underlayLines.push(from);
      return;
    }
    if (value.spec.class === 'sp-reorder-gap') {
      assert.equal(value.spec.attributes?.['data-reorder-chip'], undefined);
      carriers.push({ from, to });
      return;
    }
    assert.ok(value.spec.class.split(' ').includes('sp-reorder-chip'));
    assert.equal(typeof value.spec.attributes?.['data-reorder-chip'], 'string');
    chips.push({
      from,
      to,
      id: value.spec.attributes['data-reorder-chip'] as string,
      active: value.spec.attributes['data-reorder-chip'] === String(field.paint?.activeChip),
    });
  });
  assert.deepEqual(underlayLines, [0]);
  assert.deepEqual(chips, [
    { from: 0, to: 1, id: '0', active: true },
    { from: 3, to: 4, id: '1', active: false },
    { from: 6, to: 7, id: '2', active: false },
  ]);
  assert.deepEqual(carriers, [
    { from: 1, to: 3 },
    { from: 4, to: 6 },
  ]);
  return field.value;
}

/** Literal external DOM transport lets the real widget construct its complete tree; it confers no browser proof. */
class ControlledNode {
  className = '';
  contentEditable = '';
  textContent = '';
  readonly dataset: Record<string, string> = {};
  readonly children: (ControlledNode | string)[] = [];
  /** Every node retains only its actual creating external document. */
  readonly ownerDocument: ControlledDocument;
  /** Retain the actual external creating document without another rendering owner. */
  constructor(ownerDocument: ControlledDocument) {
    this.ownerDocument = ownerDocument;
  }
  /** Literal native attributes retain actual renderer-owned dataset values. */
  getAttribute(name: string): string | null {
    return name === 'class'
      ? this.className
      : (this.dataset[
          name.slice(5).replace(/-([a-z])/g, (_match, letter: string) => letter.toUpperCase())
        ] ?? null);
  }
  /** Supported Widget metadata writes use the same actual dataset as native inspection. */
  setAttribute(name: string, value: string): void {
    if (name === 'class') this.className = value;
    else
      this.dataset[
        name.slice(5).replace(/-([a-z])/g, (_match, letter: string) => letter.toUpperCase())
      ] = value;
  }
  /** Supported removal retires only the named dataset output. */
  removeAttribute(name: string): void {
    Reflect.deleteProperty(
      this.dataset,
      name.slice(5).replace(/-([a-z])/g, (_match, letter: string) => letter.toUpperCase()),
    );
  }
  /** The production widget appends literal nodes/text through this external DOM operation. */
  append(...children: (ControlledNode | string)[]): void {
    for (const child of children) {
      if (typeof child !== 'string') {
        child.remove();
        child.parent = this;
      }
      this.children.push(child);
    }
  }
  parent: ControlledNode | null = null;
  /** Native child inspection observes the existing populated literal sequence. */
  get childNodes(): readonly (ControlledNode | string)[] {
    return this.children;
  }
  /** Native sibling insertion moves only this exact retained node. */
  insertBefore<T extends ControlledNode>(node: T, reference: ControlledNode | null): T {
    if (node === reference) return node;
    node.remove();
    const at = reference === null ? this.children.length : this.children.indexOf(reference);
    if (at < 0) throw new Error('controlled-insert-reference-missing');
    node.parent = this;
    this.children.splice(at, 0, node);
    return node;
  }
  /** Native cache detachment changes only the exact literal parent membership. */
  remove(): void {
    const at = this.parent?.children.indexOf(this) ?? -1;
    if (at >= 0) this.parent!.children.splice(at, 1);
    this.parent = null;
  }
}
/** A finite controlled external document counts actual owner construction and retains its complete populated result. */
class ControlledDocument {
  created = 0;
  /** This is the sole external HTML boundary; no production widget or Core output is replaced. */
  createElement(): HTMLElement {
    this.created++;
    return new ControlledNode(this) as unknown as HTMLElement;
  }
}

/** Public native RangeSet comparison drives this literal output transport, never an independent widget equality rule. */
function display(f: ReturnType<typeof fixture>, initial: ReturnType<typeof projected>) {
  const document = new ControlledDocument();
  const view = Object.create(EditorView.prototype) as EditorView;
  Object.defineProperty(view, 'dom', { value: { ownerDocument: document } });
  let tree = initial.widget.toDOM(view);
  let previous = initial.decorations;
  return {
    document,
    /** Expose the exact populated output retained by native range comparison. */
    get tree() {
      return tree;
    },
    /** Only a genuine changed decoration notification constructs replacement output. */
    update(next: ReturnType<typeof projected>) {
      RangeSet.compare([previous], [next.decorations], ChangeSet.empty(f.state.doc.length).desc, {
        /** This fixture owns only point replacement widgets; marks need no DOM adaptation. */
        compareRange() {},
        /** Native comparison decides whether the owner must construct a replacement. */
        comparePoint(_from, _to, _before, after) {
          if (after?.spec.widget instanceof WidgetType) tree = after.spec.widget.toDOM(view);
        },
      });
      previous = next.decorations;
    },
  };
}

/** Observational publication controls use the actual field while keeping native source/history neutral. */
function publish(f: ReturnType<typeof fixture>, value: ReorderPreview) {
  f.view.dispatch({
    effects: installReorderPreview.of(value),
    annotations: Transaction.addToHistory.of(false),
  });
  return projected(f);
}

test('genuine same-drop measurement retains populated targets while the latest publication remains authoritative', () => {
  const f = fixture();
  try {
    f.owner.pointerdown(pointer());
    drain(f);
    dragPointer(f.owner, pointer());
    drain(f);
    // Real changed placement establishes canonical paint; returning to the authored vacancy keeps that genuine mount.
    f.owner.pointermove(pointer(1, 20, 55));
    drain(f);
    assert.deepEqual(f.state.field(reorderPreview).value?.order.order, [1, 2, 0]);
    f.owner.pointermove(pointer(1, 20, 5));
    drain(f);
    assert.equal(f.state.field(reorderPreview).native, false);
    const before = projected(f),
      rendered = display(f, before),
      tree = rendered.tree,
      built = rendered.document.created;
    assert.deepEqual(before.value.order.order, [0, 1, 2]);
    f.owner.pointermove(pointer(1, 20, 5));
    drain(f);
    const after = projected(f);
    assert.equal(after.value, before.value);
    assert.equal(after.value.order, before.value.order);
    assert.equal(after.value.output, before.value.output);
    assert.equal(after.value.token, before.value.token);
    assert.equal(after.value.capture, before.value.capture);
    assert.deepEqual(after.value.order.order, [0, 1, 2]);
    assert.equal(after.widget.eq(before.widget), true);
    assert.equal(RangeSet.eq([before.decorations], [after.decorations]), true);
    rendered.update(after);
    assert.equal(rendered.tree, tree);
    assert.equal(rendered.document.created, built);
    assert.equal(Reflect.get(after.widget, 'preview'), after.value);
    assert.equal(f.state.field(reorderPreview).value, after.value);
    const root = tree as unknown as ControlledNode;
    assert.deepEqual(
      root.children
        .filter(
          (node): node is ControlledNode =>
            node instanceof ControlledNode && node.className.startsWith('sp-reorder-chip'),
        )
        .map((node) => node.dataset.reorderChip),
      ['0', '1', '2'],
    );
  } finally {
    f.destroy();
  }
});

test('genuine external hide preserves complete native paint until changed keyboard placement', () => {
  const f = fixture();
  try {
    f.owner.keydown(key('Alt'));
    f.measure();
    f.task();
    const initial = initialMarks(f);
    assert.equal(initial.activeChip, initial.capture.active);
    assert.equal(initial.activeChip, 0);
    f.owner.keydown(key('ArrowRight'));
    f.measure();
    f.task();
    assert.equal(initialMarks(f), initial);
    drain(f);
    const moved = projected(f);
    assert.equal(moved.value.activeChip, 0);
    assert.deepEqual(moved.value.order.order, [1, 0, 2]);
    const rendered = display(f, moved);
    assert.deepEqual(
      (rendered.tree as unknown as ControlledNode).children
        .filter(
          (node): node is ControlledNode =>
            node instanceof ControlledNode &&
            node.className === 'sp-reorder-chip sp-reorder-chip-active',
        )
        .map((node) => node.dataset.reorderChip),
      ['0'],
    );
    assert.equal(Reflect.get(moved.widget, 'preview'), moved.value);
  } finally {
    f.destroy();
  }
});

test('render equality refuses altered carriers partitions blank targets chip ranges and exact output', () => {
  const f = fixture();
  try {
    f.owner.keydown(key('Alt'));
    f.measure();
    f.task();
    initialMarks(f);
    f.owner.keydown(key('ArrowRight'));
    drain(f);
    const before = projected(f),
      value = before.value;
    const controls: readonly ReorderPreview[] = [
      { ...value, activeChip: 1 },
      {
        ...value,
        order: { ...value.order, prefix: { ...value.order.prefix, text: ' ' } },
      },
      {
        ...value,
        order: { ...value.order, suffix: { ...value.order.suffix, text: ' ' } },
      },
      { ...value, order: { ...value.order, partitions: [1, 0, 0] } },
      {
        ...value,
        order: {
          ...value.order,
          slots: [{ ...value.order.slots[0]!, text: ',\n\n' }, value.order.slots[1]!],
        },
      },
      { ...value, order: { ...value.order, order: [2, 0, 1] } },
      { ...value, output: { ...value.output, text: 'x, b, c' } },
      {
        ...value,
        output: {
          ...value.output,
          renderedRanges: {
            ...value.output.renderedRanges,
            0: { from: 0, to: 2 },
          },
        },
      },
      {
        ...value,
        output: {
          ...value.output,
          renderedRanges: {
            1: value.output.renderedRanges[1]!,
            2: value.output.renderedRanges[2]!,
          },
        },
      },
    ];
    for (const control of controls) {
      const after = publish(f, control);
      assert.equal(after.widget.eq(before.widget), false);
      assert.equal(RangeSet.eq([before.decorations], [after.decorations]), false);
      assert.equal(f.state.field(reorderPreview).value, control);
    }
  } finally {
    f.destroy();
  }
});

test('equal text in another genuine capture or session cannot reuse prior targets', () => {
  const f = fixture();
  try {
    f.owner.keydown(key('Alt'));
    f.measure();
    f.task();
    initialMarks(f);
    f.owner.keydown(key('ArrowRight'));
    drain(f);
    const before = projected(f),
      value = before.value;
    const another = captureReorder(f.state);
    assert.notEqual(another, value.capture);
    for (const control of [
      { ...value, token: {} },
      { ...value, capture: another },
    ]) {
      const after = publish(f, control);
      assert.equal(after.widget.eq(before.widget), false);
      assert.equal(RangeSet.eq([before.decorations], [after.decorations]), false);
    }
  } finally {
    f.destroy();
  }
});
