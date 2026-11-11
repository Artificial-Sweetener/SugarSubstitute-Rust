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
/** Actual preview construction retires shared widget mounts when a later external DOM operation fails. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect } from '@codemirror/state';
import { EditorView, type WidgetType } from '@codemirror/view';
import {
  hideReorderChip,
  serializeReorderPreview,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { projectionState } from '../transaction/projection-state.ts';
import { captureReorder } from '../transaction/reorder-fixture.ts';
import {
  initializeNativeStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import { installProjection } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { SourceWidget } from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import {
  installReorderPreview,
  reorderPreview,
  reorderPreviewExtension,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';

/** Literal DOM transport records parent membership and one controlled append boundary, without browser layout. */
class PaintNode {
  className = '';
  contentEditable = '';
  textContent = '';
  readonly dataset: Record<string, string> = {};
  private readonly attributes = new Map<string, string>();
  readonly children: PaintNode[] = [];
  readonly ownerDocument: PaintDocument;
  /** Retain only the actual creating external transport. */
  constructor(document: PaintDocument) {
    this.ownerDocument = document;
  }
  /** Dynamic labels and local visibility remain literal at this external boundary. */
  setAttribute(name: string, value: string): void {
    this.attributes.set(name, value);
  }
  /** The canonical presentation reads actual prior owned metadata before a native write. */
  getAttribute(name: string): string | null {
    return this.attributes.get(name) ?? null;
  }
  /** Removing exact owned metadata preserves unrelated literal attributes. */
  removeAttribute(name: string): void {
    this.attributes.delete(name);
  }
  parent: PaintNode | null = null;
  /** Native child inspection is the same retained literal sequence, without a second tree. */
  get childNodes(): readonly PaintNode[] {
    return this.children;
  }
  /** Native cached paint detaches its exact parent membership independently of canonical widget retirement. */
  remove(): void {
    const at = this.parent?.children.indexOf(this) ?? -1;
    if (at >= 0) this.parent!.children.splice(at, 1);
    this.parent = null;
  }
  /** The genuine carrier-adoption boundary injects one fixed failure after canonical shared mounts exist. */
  private admit(node: PaintNode): void {
    if (
      node.className === 'sp-reorder-gap' &&
      ++this.ownerDocument.carriers === 2 &&
      this.ownerDocument.failAppend
    )
      throw this.ownerDocument.cause;
  }
  /** Append one literal node while preserving actual parent membership. */
  append(node: PaintNode): void {
    this.admit(node);
    node.remove();
    node.parent = this;
    this.children.push(node);
  }
  /** Canonical sibling reconciliation uses native insertion at the same controlled carrier-adoption frontier. */
  insertBefore<T extends PaintNode>(node: T, reference: PaintNode | null): T {
    if (node === reference) return node;
    this.admit(node);
    node.remove();
    const at = reference === null ? this.children.length : this.children.indexOf(reference);
    if (at < 0) throw new Error('paint-insert-reference-missing');
    node.parent = this;
    this.children.splice(at, 0, node);
    return node;
  }
  /** Observe only the production builder's explicit rich fragment lifetime markers. */
  querySelectorAll<T extends HTMLElement>(_selector: string): T[] {
    const result: PaintNode[] = [];
    const visit = (node: PaintNode): void => {
      for (const child of node.children) {
        if (child.dataset.reorderFragment) result.push(child);
        visit(child);
      }
    };
    visit(this);
    return result as unknown as T[];
  }
}
/** A finite external document injects one append failure without replacing projection or serialization rules. */
class PaintDocument {
  carriers = 0;
  readonly failAppend: boolean;
  readonly cause: Error;
  /** Capture the independently controlled DOM failure. */
  constructor(failAppend: boolean, cause: Error) {
    this.failAppend = failAppend;
    this.cause = cause;
  }
  /** The real preview and real fragment renderer create every observed node. */
  createElement(): HTMLElement {
    return new PaintNode(this) as unknown as HTMLElement;
  }
  /** Literal text transport has no source or editing authority. */
  createTextNode(text: string): Node {
    const node = new PaintNode(this);
    node.textContent = text;
    return node as unknown as Node;
  }
}

for (const appendFailure of [false, true])
  test(`complete preview construction retires earlier mounts after later ${appendFailure ? 'carrier adoption' : 'shared paint'} failure`, () => {
    let state = initializeNativeStructure(projectionState('(a:2.00), tail'));
    const canonical = prepareProjection(state, prepareNativeStructure(state));
    state = state.update({
      effects: [
        StateEffect.appendConfig.of(reorderPreviewExtension),
        installProjection.of(canonical),
      ],
    }).state;
    const capture = captureReorder(state, 11),
      order = hideReorderChip(capture.prepared, capture.prepared.initial, capture.active);
    assert.ok(order);
    assert.deepEqual(order.order, [0]);
    const output = serializeReorderPreview(capture.prepared, order);
    assert.ok(output);
    let mounted = 0,
      released = 0;
    const cause = new Error('controlled-complete-preview-failure'),
      document = new PaintDocument(appendFailure, cause);
    canonical.all.between(0, state.doc.length, (_from, _to, value) => {
      const widget: unknown = value.spec.widget;
      if (!(widget instanceof SourceWidget)) return;
      widget.toDOM = () => {
        if (!appendFailure && mounted === 1) throw cause;
        mounted++;
        return document.createElement();
      };
      widget.destroy = () => {
        released++;
      };
    });
    state = state.update({
      effects: installReorderPreview.of({
        token: {},
        capture,
        activeChip: capture.active,
        order,
        output,
        presentation: 'collision',
        scenes: null,
      }),
    }).state;
    assert.equal(state.field(reorderPreview).native, false);
    let preview: WidgetType | null = null;
    state.field(reorderPreview).decorations.between(0, state.doc.length, (_from, _to, value) => {
      preview = value.spec.widget as WidgetType;
    });
    assert.ok(preview);
    const view = Object.create(EditorView.prototype) as EditorView;
    Object.defineProperty(view, 'dom', { value: { ownerDocument: document } });
    assert.throws(
      () => preview!.toDOM(view),
      (error) => error === cause,
    );
    assert.equal(mounted, appendFailure ? 2 : 1);
    assert.equal(released, mounted);
  });
