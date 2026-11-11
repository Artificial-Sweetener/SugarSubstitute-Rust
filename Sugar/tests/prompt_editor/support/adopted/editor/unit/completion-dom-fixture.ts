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
/** Literal native DOM and pointer transport; no layout, rendering, scrolling or completion policy is reproduced. */
import assert from 'node:assert/strict';

/** A finite native document supplies only the event/window endpoints consumed by the actual owners. */
export class CompletionDocument extends EventTarget {
  readsAllowed = false;
  hidden = false;
  readonly defaultView = {
    performance: { now: () => 0 },
    requestAnimationFrame: (_callback: FrameRequestCallback) => 1,
    cancelAnimationFrame: (_handle: number) => {},
    getComputedStyle: (node: CompletionNode) => {
      assert.ok(this.readsAllowed);
      return node.style;
    },
  };
  /** Return empty literal nodes sharing one native document identity. */
  createElement(): CompletionNode {
    return new CompletionNode(this);
  }
}

/** A small event target stores native outputs without implementing any scrolling or layout policy. */
export class CompletionNode extends EventTarget {
  readonly children: CompletionNode[] = [];
  readonly style: Record<string, string> = {
    height: '200px',
    borderTopWidth: '1px',
    borderBottomWidth: '1px',
    paddingTop: '0px',
    paddingBottom: '0px',
  };
  readonly attributes = new Map<string, string>();
  readonly captures = new Set<number>();
  parentElement: CompletionNode | null = null;
  className = '';
  hidden = false;
  isConnected = true;
  clientHeight = 100;
  scrollHeight = 400;
  scrollTop = 0;
  physicalHeight = 200;
  readsAllowed = false;
  released = 0;
  removed = 0;
  removeFailure: { cause: unknown } | null = null;
  readonly ownerDocument: CompletionDocument;
  readonly dataset: Record<string, string> = {};
  textContent = '';
  id = '';
  tabIndex = 0;
  clientWidth = 200;
  offsetTop = 0;
  offsetHeight = 20;
  readonly classList = {
    contains: (name: string) => this.className.split(' ').includes(name),
    toggle: (name: string, force: boolean) => {
      const names = new Set(this.className.split(' ').filter(Boolean));
      if (force) names.add(name);
      else names.delete(name);
      this.className = [...names].join(' ');
      return force;
    },
  };
  /** Native allocation creates only empty event targets, with explicit caller-owned geometry. */
  constructor(ownerDocument = new CompletionDocument()) {
    super();
    Object.defineProperties(this.style, {
      getPropertyValue: { value: (name: string) => this.style[name] ?? '' },
      setProperty: {
        value: (name: string, value: string) => {
          this.style[name] = value;
        },
      },
    });
    this.ownerDocument = ownerDocument;
  }
  /** Literal parent/child insertion preserves the real scroll-ancestor contract. */
  append(...children: CompletionNode[]): void {
    for (const child of children) {
      this.children.push(child);
      child.parentElement = this;
    }
  }
  /** Replace only the literal owned child list. */
  replaceChildren(...children: CompletionNode[]): void {
    for (const child of this.children) child.parentElement = null;
    this.children.length = 0;
    this.textContent = '';
    this.append(...children);
  }
  /** Match only the class selectors read by the canonical title owner. */
  querySelector(selector: string): CompletionNode | null {
    for (const node of this.children) {
      if (node.className.split(' ').includes(selector.slice(1))) return node;
      const nested = node.querySelector(selector);
      if (nested) return nested;
    }
    return null;
  }
  /** Native membership is actual node identity. */
  contains(target: CompletionNode): boolean {
    return target === this || this.children.some((child) => child.contains(target));
  }
  /** Record literal inert/accessibility attributes. */
  setAttribute(name: string, value: string): void {
    this.attributes.set(name, value);
  }
  /** Geometry is hostile outside the explicit read phase. */
  getBoundingClientRect(): {
    height: number;
    left: number;
    top: number;
    width: number;
  } {
    assert.ok(this.readsAllowed || this.ownerDocument.readsAllowed, 'unexpected input geometry');
    return { height: this.physicalHeight, left: 0, top: 0, width: 200 };
  }
  /** Capture belongs to the exact owned pointer target. */
  setPointerCapture(pointer: number): void {
    this.captures.add(pointer);
  }
  /** Inspect literal capture membership. */
  hasPointerCapture(pointer: number): boolean {
    return this.captures.has(pointer);
  }
  /** Release actual pointer identity independently of removal. */
  releasePointerCapture(pointer: number): void {
    this.captures.delete(pointer);
    this.released++;
  }
  /** Physical removal has an explicit hostile external boundary. */
  remove(): void {
    this.removed++;
    if (this.removeFailure) throw this.removeFailure.cause;
    const parent = this.parentElement;
    if (parent) parent.children.splice(parent.children.indexOf(this), 1);
    this.parentElement = null;
  }
}
