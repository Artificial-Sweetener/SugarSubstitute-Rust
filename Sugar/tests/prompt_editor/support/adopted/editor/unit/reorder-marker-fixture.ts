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
/** Complete external paint/measure/update transport for actual native marker-owner controls. */
import assert from 'node:assert/strict';
import { fixture, pointer } from './reorder-session-fixture.ts';
import { collisionGeometry } from './reorder-passive-geometry-fixture.ts';

const viewport = { left: 0, right: 100, top: 0, bottom: 100 };

/** Observe the real session's owned external paint nodes through a literal finite DOM boundary. */
export function paintFixture() {
  const f = fixture(),
    nodes: {
      className: string;
      style: Record<string, string | ((_name: string, _value: string) => void)>;
      removed: boolean;
      remove(): void;
    }[] = [];
  const document = f.view.dom.ownerDocument,
    createElement = document.createElement.bind(document);
  document.createElement = ((name: string) => {
    const node = createElement(name) as HTMLElement & { removed: boolean },
      remove = node.remove.bind(node),
      append = node.append.bind(node),
      replaceChildren = node.replaceChildren.bind(node);
    /** Native connection changes preserve the complete detached subtree for a later canonical paint lease. */
    const disconnected = (paint: Node, value: boolean): void => {
      (paint as HTMLElement & { removed: boolean }).removed = value;
      for (const child of Array.from(paint.childNodes)) disconnected(child, value);
    };
    node.removed = false;
    /** Native removal disconnects all descendants while preserving the complete base DOM transport. */
    node.remove = () => {
      disconnected(node, true);
      remove();
    };
    /** Adoption reconnects retained descendants without rebuilding their literal paint or shared widget mount. */
    node.append = (...children: (Node | string)[]) => {
      append(...children);
      for (const child of children) if (typeof child !== 'string') disconnected(child, false);
    };
    /** Replacing native children disconnects the old descendant paint just like removing their containing surface. */
    node.replaceChildren = (...children: (Node | string)[]) => {
      for (const child of Array.from(node.childNodes)) disconnected(child, true);
      replaceChildren(...children);
    };
    if (!f.drawing) nodes.push(node as unknown as (typeof nodes)[number]);
    return node;
  }) as unknown as Document['createElement'];
  const line = f.view.domAtPos(0).node as HTMLElement,
    nativeLineBox = line.getBoundingClientRect.bind(line);
  line.getBoundingClientRect = () => {
    const box = nativeLineBox();
    return {
      // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
      ...box,
      left: box.left + dx,
      right: box.right + dx,
      top: box.top + dy,
      bottom: box.bottom + dy,
    } as DOMRect;
  };
  let dx = 0,
    dy = 0;
  const query = f.view.contentDOM.querySelectorAll,
    shifted = new WeakSet<Element>();
  f.view.contentDOM.querySelectorAll = ((selector: string) =>
    Array.from(query.call(f.view.contentDOM, selector)).map((node) => {
      const element = node as HTMLElement;
      if (shifted.has(element)) return element;
      shifted.add(element);
      const read = element.getClientRects.bind(element);
      element.getClientRects = (() =>
        Array.from(read(), (rect) => ({
          // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
          ...rect,
          left: rect.left + dx,
          right: rect.right + dx,
          top: rect.top + dy,
          bottom: rect.bottom + dy,
        }))) as unknown as HTMLElement['getClientRects'];
      return element;
    })) as unknown as typeof f.view.contentDOM.querySelectorAll;
  f.view.scrollDOM.getBoundingClientRect = () => ({
    ...viewport,
    left: dx,
    right: dx + 100,
    width: 100,
    height: 100,
    x: dx,
    y: 0,
    toJSON() {
      return {};
    },
  });
  return {
    ...f,
    f,
    nodes,
    /** Shift the actual published target fragments and their containing viewport without changing semantic state. */
    shiftGeometry(x: number, y: number): void {
      dx = x;
      dy = y;
    },
  };
}
/** Deliver each actual owned measure/deferred callback; no native event or phase is synthesized. */
export function drain(f: ReturnType<typeof fixture>): void {
  for (let turn = 0; turn < 32 && (f.jobs.length || f.tasks.length); turn++) {
    if (f.jobs.length) f.measure();
    if (f.tasks.length) f.task();
  }
  assert.equal(f.jobs.length + f.tasks.length, 0);
}
/** Stop at the real connected calibrated external scene, leaving its actual target read pending for frontier proof. */
export function calibrateCollision(f: ReturnType<typeof fixture>): void {
  const ready = (): boolean =>
    Array.from(f.view.dom.ownerDocument.body.childNodes).some(
      (node) =>
        (node as HTMLElement).className.includes('sp-reorder-base-measurement') &&
        String(Reflect.get((node as HTMLElement).style, 'transform')).startsWith('scale('),
    );
  for (let turn = 0; turn < 16 && !ready(); turn++) {
    if (f.jobs.length) f.measure();
    if (f.tasks.length) f.task();
  }
  assert.equal(ready(), true);
  assert.ok(f.jobs.length);
}

/** Complete public native geometry notification transports the actual fixture state and zero or one actual source-neutral transaction. */
export function geometryUpdate(
  f: ReturnType<typeof fixture>,
  geometry: boolean,
  viewport: boolean,
  transaction?: import('@codemirror/state').Transaction,
): import('@codemirror/view').ViewUpdate {
  return {
    view: f.view,
    state: f.state,
    startState: transaction?.startState ?? f.state,
    transactions: transaction ? [transaction] : [],
    changes: transaction?.changes ?? f.state.changes([]),
    focusChanged: false,
    geometryChanged: geometry,
    viewportChanged: viewport,
    viewportMoved: viewport,
    heightChanged: geometry,
    docChanged: false,
    selectionSet: false,
  };
}

/** Literal peer fragments expose renewed hidden occurrence geometry and independent held-center seed inputs through the actual session. */
export function arrowGeometryFixture() {
  const f = fixture(),
    query = f.view.contentDOM.querySelectorAll;
  const box = (left: number, top: number) => ({
    left,
    right: left + 20,
    top,
    bottom: top + 20,
    width: 20,
    height: 20,
  });
  let layout = [box(10, 10), box(10, 40), box(10, 70)];
  collisionGeometry(f, (id) => [layout[id]!] as DOMRect[]);
  f.view.contentDOM.querySelectorAll = ((selector: string) =>
    Array.from(query.call(f.view.contentDOM, selector), (node) => ({
      dataset: (node as HTMLElement).dataset,
      /** Preserve the original current node attribute protocol while overriding geometry alone. */
      getAttribute(name: string): string | null {
        return (node as HTMLElement).getAttribute(name);
      },
      getClientRects: () => [layout[Number((node as HTMLElement).dataset.reorderChip)]!],
    }))) as unknown as typeof f.view.contentDOM.querySelectorAll;
  return {
    ...f,
    f,
    /** Genuine geometry reflow changes peer row membership while preserving semantic state. */
    reflow(): void {
      layout = [box(10, 10), box(180, 10), box(10, 40)];
    },
    /** Move the held origin below both peers, requiring renewed origin before the hidden target is chosen. */
    shiftOrigin(): void {
      layout = [box(10, 90), box(10, 40), box(10, 70)];
    },
  };
}

/** A finite literal Element boundary sends genuine current preview-chip identity through the supported session pointer path. */
export function pointerTransferFixture() {
  const transport = paintFixture(),
    boundary = globalThis as { Element?: typeof Element },
    previous = boundary.Element;
  /** The controlled external DOM boundary returns the actual published fixture node, never a fabricated Core target. */
  class ChipElement {
    /** Retain only the actual current preview node selected by the proof. */
    private readonly node: Element;
    /** Bind the independently selected actual publication node for this event. */
    constructor(node: Element) {
      this.node = node;
    }
    /** Native closest lookup returns that current source-neutral preview node. */
    closest(): Element {
      return this.node;
    }
  }
  boundary.Element = ChipElement as unknown as typeof Element;
  return {
    ...transport,
    /** Observe the real published chip before delivering its primary pointer fields. */
    down(chip: number, id = 9, y = 50, x = 55): PointerEvent {
      transport.f.setChipGeometry(chip, x, y);
      const node = Array.from(
        transport.f.view.contentDOM.querySelectorAll<HTMLElement>('[data-reorder-chip]'),
      ).find((node) => node.dataset.reorderChip === String(chip));
      assert.ok(node);
      return {
        // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
        ...pointer(id, y, x),
        target: new ChipElement(node),
      } as unknown as PointerEvent;
    },
    /** Both genuine native fixture resources and the finite external DOM constructor are released. */
    destroy(): void {
      try {
        transport.f.destroy();
      } finally {
        if (previous === undefined) delete boundary.Element;
        else boundary.Element = previous;
      }
    },
  };
}

/** Publish the actual pending pointer-paint frame without advancing the independent30ms edge-step clock; then settle its native measure jobs. */
export function paintPointerFrame(f: ReturnType<typeof fixture>): void {
  if (f.frames.size) f.frame(0);
  drain(f);
}
