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
import { nativeClassListFixture } from './native-class-list-fixture.ts';
import { reorderPaintStyleFixture } from './reorder-paint-style-fixture.ts';
/** Finite literal native paint/DOM transport for the session fixture; it grants no document, gesture or history authority. */
import type { EditorState } from '@codemirror/state';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import type { PassiveNode } from './reorder-passive-geometry-fixture.ts';
/** Independently supplied physical target rectangles retain all original geometry operands. */
interface SessionPaintRect {
  readonly left: number;
  readonly right: number;
  readonly top: number;
  readonly bottom: number;
  readonly width: number;
  readonly height: number;
}
/** Existing session state, scheduling and measured targets remain borrowed exact owners. */
interface SessionPaintPorts {
  /** Independently supplied physical geometry transports the actual external collision tree. */
  passiveGeometry(node: PassiveNode): DOMRect[] | null;
  state(): EditorState;
  line(): object;
  targets(): readonly {
    readonly dataset: Readonly<Record<string, string>>;
    getClientRects(): readonly SessionPaintRect[];
  }[];
  readonly chipGeometry: ReadonlyMap<number, SessionPaintRect>;
  request(callback: FrameRequestCallback): number;
  cancel(id: number): void;
  removed(): void;
}
/** Every allocated literal node observes one mounted/ancestor lease; browser proof still owns actual paint and layout. */
export function createReorderSessionPaintTransport(ports: SessionPaintPorts) {
  const mounted = new WeakSet<object>();
  const connectedRoots = new WeakSet<object>();
  /** Follow intact ancestor leases to an explicitly mounted document root. */
  function connected(
    ancestor: { readonly parentNode?: object | null } | null,
    root: object | null,
  ): boolean {
    while (ancestor) {
      if (root ? ancestor === root : connectedRoots.has(ancestor)) return true;
      ancestor = (ancestor.parentNode ?? null) as typeof ancestor;
    }
    return false;
  }
  const doc = {
    defaultView: {
      /** Observe finite appearance transport; real native proof owns computed typography. */
      getComputedStyle(node: object) {
        return {
          font: '14px Segoe UI',
          lineHeight: '19px',
          color: 'rgb(0, 0, 0)',
          width: '100px',
          height: '20px',
          boxSizing: 'border-box',
          display: node === ports.line() ? 'block' : 'inline',
          length: 0,
          item: (_at: number) => '',
          getPropertyValue: (name: string) =>
            (
              ({
                font: '14px Segoe UI',
                'line-height': '19px',
                width: '100px',
                height: '20px',
                'box-sizing': 'border-box',
                display: node === ports.line() ? 'block' : 'inline',
              }) as Readonly<Record<string, string>>
            )[name] ?? '',
        };
      },
      requestAnimationFrame(callback: FrameRequestCallback) {
        return ports.request(callback);
      },
      cancelAnimationFrame(id: number) {
        ports.cancel(id);
      },
    },
    body: {
      nodeType: 1,
      className: '',
      dataset: {} as Record<string, string>,
      isConnected: true,
      parentElement: null,
      /** This explicit connected body belongs to the finite borrowed document. */
      getRootNode(): object {
        return doc;
      },
      /** Body classes are literal mutable native ancestry operands too. */
      getAttribute(name: string): string | null {
        return name === 'class' ? this.className : null;
      },
      childNodes: [] as unknown[],
      append(node: { parentNode?: { childNodes: unknown[] } | null }) {
        const previous = node.parentNode?.childNodes,
          at = previous?.indexOf(node) ?? -1;
        if (at >= 0) previous!.splice(at, 1);
        node.parentNode = doc.body;
        doc.body.childNodes.push(node);
        mounted.add(node);
      },
    },
    /** Native literal text owns UTF-16 data and intact parent connection, never an editor document. */
    createTextNode(value: string): Text {
      const node = {
        nodeType: 3,
        data: value,
        childNodes: [] as Node[],
        parentNode: null as Node | null,
        ownerDocument: doc,
        /** Native text removal affects only this exact parent's child list. */
        remove(): void {
          const parent = this.parentNode as unknown as {
            childNodes: unknown[];
          } | null;
          const at = parent?.childNodes.indexOf(this) ?? -1;
          if (at >= 0) parent!.childNodes.splice(at, 1);
          this.parentNode = null;
        },
        get length(): number {
          return this.data.length;
        },
        get textContent(): string {
          return this.data;
        },
        get isConnected(): boolean {
          return connected(this, null);
        },
      };
      return node as unknown as Text;
    },
    /** Controlled native caret readings use the independently supplied target boxes; actual browser typography remains native-only proof. */
    createRange(): Range {
      let start: Node | null = null,
        offset = 0;
      return {
        setStart(node: Node, at: number): void {
          start = node;
          offset = at;
        },
        collapse(): void {},
        getClientRects(): DOMRectList {
          let target = start;
          while (
            target &&
            !(
              target.nodeType === 1 &&
              ['reorderChip', 'reorderGap', 'reorderPartition'].some(
                (key) => (target as HTMLElement).dataset[key] !== undefined,
              )
            )
          )
            target = target.parentNode;
          if (!target && start)
            target =
              Array.from(
                start.nodeType === 3 && start.parentNode
                  ? start.parentNode.childNodes
                  : start.childNodes,
              ).find(
                (child) =>
                  child.nodeType === 1 &&
                  ['reorderChip', 'reorderGap', 'reorderPartition'].some(
                    (key) => (child as HTMLElement).dataset[key] !== undefined,
                  ),
              ) ?? null;
          const element = (target ??
            (start?.nodeType === 1 ? start : start?.parentNode)) as HTMLElement | null;
          if (!element) return [] as unknown as DOMRectList;
          const boxes = element.getClientRects(),
            box = offset === 0 ? boxes[0] : Array.from(boxes).at(-1);
          return box
            ? ([
                {
                  // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
                  ...box,
                  left: offset === 0 ? box.left : box.right,
                  right: offset === 0 ? box.left : box.right,
                  width: 0,
                },
              ] as unknown as DOMRectList)
            : ([] as unknown as DOMRectList);
        },
      } as unknown as Range;
    },
    /** Retain each declared element kind for DOM identity without simulating native browser layout. */
    createElement(name: string): HTMLElement {
      const node = {
        nodeType: 1,
        localName: name,
        className: '',

        get textContent(): string {
          return this.childNodes.map((child) => (child as Node).textContent ?? '').join('');
        },
        set textContent(value: string) {
          this.replaceChildren();
          if (value)
            this.append(
              doc.createTextNode(value) as unknown as {
                parentNode?: { childNodes: unknown[] } | null;
              },
            );
        },
        contentEditable: '',
        ownerDocument: doc,
        dataset: {} as Record<string, string>,
        childNodes: [] as unknown[],
        parentNode: null as {
          childNodes: unknown[];
          /** Native parent roots preserve document and detached-subtree identity. */
          getRootNode(): object;
        } | null,
        /** Literal element ancestry follows the same parent linkage as native insertion/removal. */
        get parentElement(): HTMLElement | null {
          return (this.parentNode as { nodeType?: number } | null)?.nodeType === 1
            ? (this.parentNode as unknown as HTMLElement)
            : null;
        },
        /** Connected document roots and detached subtree roots retain their actual distinct identities. */
        getRootNode(): object {
          return this.parentNode ? this.parentNode.getRootNode() : this;
        },
        get isConnected(): boolean {
          return connected(this, null);
        },
        /** Native subtree membership follows the retained parent chain, including detached leases. */
        contains(candidate: { readonly parentNode?: object | null }): boolean {
          return connected(candidate, this);
        },
        style: reorderPaintStyleFixture(),
        /** Native shallow cloning copies isolated inline declarations without children or mounted identities. */
        cloneNode(deep: boolean): HTMLElement {
          if (deep) throw new Error('controlled-paint-deep-clone-unexpected');
          const copy = doc.createElement(name);
          copy.style.cssText = this.style.cssText;
          for (const [property, value] of Object.entries(this.style))
            if (typeof value === 'string') Reflect.set(copy.style, property, value);
          copy.className = this.className;
          copy.contentEditable = this.contentEditable;
          return copy;
        },
        /** The themed external proxy may mount literal child nodes. */
        append(...nodes: { parentNode?: { childNodes: unknown[] } | null }[]) {
          for (const node of nodes) {
            const previous = node.parentNode?.childNodes,
              at = previous?.indexOf(node) ?? -1;
            if (at >= 0) previous!.splice(at, 1);
            node.parentNode = this;
            this.childNodes.push(node);
          }
        },
        /** Native insertion moves exactly one existing child without rebuilding adjacent subtrees. */
        insertBefore<T extends { parentNode?: { childNodes: unknown[] } | null }>(
          node: T,
          reference: unknown | null,
        ): T {
          if (node === reference) return node;
          const previous = node.parentNode?.childNodes,
            old = previous?.indexOf(node) ?? -1;
          if (old >= 0) previous!.splice(old, 1);
          const at =
            reference === null ? this.childNodes.length : this.childNodes.indexOf(reference);
          if (at < 0) throw new Error('native-insert-reference-missing');
          node.parentNode = this;
          this.childNodes.splice(at, 0, node);
          return node;
        },
        replaceChildren() {
          for (const node of this.childNodes) (node as { parentNode: unknown }).parentNode = null;
          this.childNodes.splice(0);
        },
        closest() {
          return ports.line();
        },
        getAttribute(name: string): string | null {
          return name === 'class' ? this.className : null;
        },
        /** Native single-class lookup follows actual descendant order for rich-widget input retirement. */
        querySelector(selector: string): HTMLElement | null {
          if (!/^\.[a-zA-Z][\w-]*$/.test(selector))
            throw new Error('session-paint-selector-unsupported');
          const pending = [...this.childNodes].reverse() as Node[];
          while (pending.length) {
            const child = pending.pop()!;
            if (child.nodeType !== 1) continue;
            const element = child as HTMLElement;
            if (element.className.split(/\s+/).includes(selector.slice(1))) return element;
            pending.push(...Array.from(element.childNodes).reverse());
          }
          return null;
        },
        querySelectorAll() {
          return [];
        },
        getClientRects(): DOMRect[] {
          const passive = ports.passiveGeometry(this);
          if (passive) return passive;
          const identity =
            this.dataset.reorderChip !== undefined
              ? 'reorderChip'
              : this.dataset.reorderPartition !== undefined
                ? 'reorderPartition'
                : 'reorderGap';
          if (this.dataset[identity] !== undefined) {
            const targets = ports.targets();
            const measured = targets.find(
              (node) =>
                node.dataset[identity] === this.dataset[identity] &&
                node.dataset.reorderBlank === this.dataset.reorderBlank,
            );
            if (measured) return Array.from(measured.getClientRects()) as DOMRect[];
          }
          /** Controlled literal descendants share their parent's independently supplied font box. */
          if (this.dataset[identity] === undefined) {
            let ancestor = this.parentNode as {
              readonly dataset?: Readonly<Record<string, string>>;
              readonly parentNode?: unknown;
              getClientRects?(): DOMRect[];
            } | null;
            while (
              ancestor &&
              !(
                ancestor.dataset &&
                ['reorderChip', 'reorderGap', 'reorderPartition'].some(
                  (key) => ancestor!.dataset![key] !== undefined,
                )
              )
            )
              ancestor = (ancestor.parentNode ?? null) as typeof ancestor;
            if (ancestor?.getClientRects) return ancestor.getClientRects();
          }
          const published = ports.state().field(reorderPreview).value,
            id = Number(this.dataset.reorderChip),
            at = published?.order.order.indexOf(id) ?? 0;
          return [
            (id === published?.activeChip && published.presentation !== 'pointer'
              ? ports.chipGeometry.get(id)
              : undefined) ?? {
              left: 10 + at * 30,
              right: 30 + at * 30,
              top: 10,
              bottom: 30,
              width: 20,
              height: 20,
            },
          ] as DOMRect[];
        },
        getBoundingClientRect(): DOMRect {
          const rects = this.getClientRects(),
            left = Math.min(...rects.map((rect) => rect.left)),
            right = Math.max(...rects.map((rect) => rect.right)),
            top = Math.min(...rects.map((rect) => rect.top)),
            bottom = Math.max(...rects.map((rect) => rect.bottom));
          return {
            left,
            right,
            top,
            bottom,
            width: right - left,
            height: bottom - top,
          } as DOMRect;
        },
        /** Transport actual marker accessibility attributes without DOM rendering authority. */
        setAttribute(_name: string, _value: string) {},
        remove() {
          const at = this.parentNode?.childNodes.indexOf(this) ?? -1;
          if (at >= 0) this.parentNode!.childNodes.splice(at, 1);
          this.parentNode = null;
          if (mounted.delete(this) && !this.className.includes('sp-reorder-base-measurement'))
            ports.removed();
        },
      };
      Object.defineProperty(node, 'classList', {
        value: nativeClassListFixture(node),
      });
      return node as unknown as HTMLElement;
    },
    /** Native SVG allocation is a literal resource transport; browser raster proof owns union outline pixels. */
    createElementNS(_namespace: string, name: string): HTMLElement {
      return this.createElement(name);
    },
  };
  connectedRoots.add(doc.body);
  return {
    document: doc,
    connectRoot(root: object) {
      connectedRoots.add(root);
    },
  };
}
