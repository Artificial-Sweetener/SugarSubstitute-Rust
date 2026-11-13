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
/** Controlled native node/selector/rectangle transport retains literal trees independently of scheduling and fixture assembly. */
import { reorderPaintStyleFixture } from './reorder-paint-style-fixture.ts';
/** Literal native rectangles are independent of Core layout and the motion implementation. */
interface Box {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}
/** This fixture models only the exact native snapshot boundary, not a general DOM or editor. */
export class PaintNode {
  readonly localName: string;
  readonly ownerDocument: object;
  readonly box: Box;
  readonly nodeType = 1;
  clientLeft = 0;
  clientTop = 0;
  clientWidth = 240;
  clientHeight = 100;
  scrollLeft = 0;
  scrollTop = 0;
  scrollWidth = 240;
  scrollHeight = 100;
  readonly dataset: Record<string, string> = {};
  readonly childNodes: (PaintNode | PaintText)[] = [];
  private readonly attributeValues = new Map<string, string>();
  /** Only the literal native attribute iterable is transported; styles/classes retain their genuine DOM identities. */
  get attributes(): readonly {
    readonly name: string;
    readonly value: string;
  }[] {
    const values = new Map(this.attributeValues);
    if (this.className) values.set('class', this.className);
    for (const [name, value] of Object.entries(this.dataset))
      values.set(`data-${name.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}`, value);
    if (this.style.cssText) values.set('style', this.style.cssText);
    return [...values].map(([name, value]) => ({ name, value }));
  }
  /** Literal parent/root APIs expose actual fixture ancestry, never editor source or selection authority. */
  get parentElement(): PaintNode | null {
    return this.parent;
  }
  /** Expose the actual retained parent identity. */
  get parentNode(): PaintNode | null {
    return this.parent;
  }
  /** Return the actual creating external document used by this literal node transport. */
  getRootNode(): object {
    return this.ownerDocument;
  }
  readonly style = reorderPaintStyleFixture();
  className = '';
  readonly classList = nativeClassListFixture(this);
  contentEditable = 'inherit';
  namespaceURI = 'http://www.w3.org/1999/xhtml';
  parent: PaintNode | null = null;
  removed = false;
  /** Determine physical connection from the actual retained parent chain. */
  get isConnected(): boolean {
    if (this.localName === 'body' || this.localName === 'head') return true;
    let node: PaintNode | null = this.parent;
    while (node) {
      if (node.localName === 'body' || node.localName === 'head') return true;
      node = node.parent;
    }
    return false;
  }
  /** Retain the literal node identity, owner document and independently specified physical rectangle. */
  constructor(localName: string, ownerDocument: object, box: Box) {
    this.localName = localName;
    this.ownerDocument = ownerDocument;
    this.box = box;
  }
  /** Shallow native cloning retains isolated declarations/attributes and allocates one new identity. */
  cloneNode(deep: boolean): PaintNode {
    if (deep) throw new Error('controlled-paint-deep-clone-unexpected');
    const document = this.ownerDocument as {
      createElement(name: string): PaintNode;
      createElementNS(namespace: string, name: string): PaintNode;
    };
    const copy =
      this.namespaceURI === 'http://www.w3.org/2000/svg'
        ? document.createElementNS(this.namespaceURI, this.localName)
        : document.createElement(this.localName);
    copy.style.cssText = this.style.cssText;
    for (const [name, value] of Object.entries(this.style))
      if (typeof value === 'string') copy.style[name] = value;
    for (const { name, value } of this.attributes) copy.setAttribute(name, value);
    copy.className = this.className;
    copy.contentEditable = this.contentEditable;
    return copy;
  }
  /** Read or replace literal descendants through the same controlled tree. */
  get textContent(): string {
    return this.childNodes.map((node) => node.textContent).join('');
  }
  /** Read or replace literal descendants through the same controlled tree. */
  set textContent(value: string) {
    this.childNodes.splice(0);
    this.append(new PaintText(value));
  }
  /** Read a literal class, style, dataset or ordinary attribute from the one native transport. */
  getAttribute(name: string): string | null {
    return this.attributes.find((attribute) => attribute.name === name)?.value ?? null;
  }
  /** Test actual transported attribute presence without semantic interpretation. */
  hasAttribute(name: string): boolean {
    return this.getAttribute(name) !== null;
  }
  /** Write only the named literal external attribute in this controlled tree. */
  setAttribute(name: string, value: string): void {
    if (name === 'style') this.style.cssText = value;
    else if (name === 'class') this.className = value;
    else if (name.startsWith('data-'))
      this.dataset[
        name.slice(5).replace(/-([a-z])/g, (_, letter: string) => letter.toUpperCase())
      ] = value;
    else this.attributeValues.set(name, value);
  }
  /** Native owned-attribute removal changes only the exact literal attribute. */
  removeAttribute(name: string): void {
    if (name === 'style') this.style.cssText = '';
    else if (name === 'class') this.className = '';
    else if (name.startsWith('data-'))
      Reflect.deleteProperty(
        this.dataset,
        name.slice(5).replace(/-([a-z])/g, (_match, letter: string) => letter.toUpperCase()),
      );
    else this.attributeValues.delete(name);
  }
  /** Append in native argument order through the same existing-parent transfer boundary as insertion. */
  append(...children: (PaintNode | PaintText)[]): void {
    for (const child of children) this.insertBefore(child, null);
  }
  /** Prepend literal children in native argument order using the same actual reparenting boundary. */
  prepend(...children: (PaintNode | PaintText)[]): void {
    for (let index = children.length - 1; index >= 0; index--)
      this.insertBefore(children[index]!, this.childNodes[0] ?? null);
  }
  /** Native insertion retains one existing child and all of its literal descendants. */
  insertBefore<T extends PaintNode | PaintText>(
    node: T,
    reference: PaintNode | PaintText | null,
  ): T {
    if (node === reference) return node;
    if (node.parent) {
      const old = node.parent.childNodes.indexOf(node);
      if (old >= 0) node.parent.childNodes.splice(old, 1);
    }
    const at = reference === null ? this.childNodes.length : this.childNodes.indexOf(reference);
    if (at < 0) throw new Error('native-insert-reference-missing');
    node.parent = this;
    this.childNodes.splice(at, 0, node);
    return node;
  }
  /** Native replacement detaches each old child before appending the provided existing nodes. */
  replaceChildren(...children: (PaintNode | PaintText)[]): void {
    for (const child of this.childNodes) child.parent = null;
    this.childNodes.splice(0);
    this.append(...children);
  }
  /** Detach this exact physical identity from its retained parent. */
  remove(): void {
    this.removed = true;
    if (this.parent) {
      const at = this.parent.childNodes.indexOf(this);
      if (at >= 0) this.parent.childNodes.splice(at, 1);
    }
    this.parent = null;
  }
  /** Follow actual parent identities to determine subtree membership. */
  contains(node: PaintNode | PaintText): boolean {
    let at: PaintNode | PaintText | null = node;
    while (at) {
      if (at === this) return true;
      at = at.parent;
    }
    return false;
  }
  /** Element collections derive only from the same retained literal children. */
  get children(): readonly PaintNode[] {
    return this.childNodes.filter((node): node is PaintNode => node instanceof PaintNode);
  }
  /** The first actual element child follows native collection identity. */
  get firstElementChild(): PaintNode | null {
    return this.children[0] ?? null;
  }
  /** Resolve the exact native class/attribute selectors used by owned paint ancestry. */
  closest(selector = '.cm-line'): PaintNode | null {
    const attribute = /^\[([^=\]]+)(?:="([^"]*)")?\]$/u.exec(selector);
    const matches = selector.startsWith('.')
      ? this.className.split(' ').includes(selector.slice(1))
      : attribute
        ? attribute[2] === undefined
          ? this.hasAttribute(attribute[1]!)
          : this.getAttribute(attribute[1]!) === attribute[2]
        : false;
    return matches ? this : (this.parent?.closest(selector) ?? null);
  }
  /** Native document-order bits reflect actual retained ancestry and sibling order, never semantic chip IDs. */
  compareDocumentPosition(other: PaintNode): number {
    if (other === this) return 0;
    if (this.contains(other)) return 4 | 16;
    if (other.contains(this)) return 2 | 8;
    /** Compare actual retained ancestry without interpreting semantic target identifiers. */
    const ancestors = (node: PaintNode): PaintNode[] => {
      const result = [node];
      while (node.parent) {
        node = node.parent;
        result.unshift(node);
      }
      return result;
    };
    const a = ancestors(this),
      b = ancestors(other);
    if (a[0] !== b[0]) return 1;
    let index = 0;
    while (a[index] === b[index]) index++;
    const parent = a[index - 1]!;
    return parent.childNodes.indexOf(a[index]!) < parent.childNodes.indexOf(b[index]!) ? 4 : 2;
  }
  /** Selector transport filters native literal classes/attributes only; no scene or rendering outcome is synthesized. */
  querySelectorAll(selector = '*'): PaintNode[] {
    const all = this.childNodes.flatMap((node) =>
      node instanceof PaintNode ? [node, ...node.querySelectorAll()] : [],
    );
    if (selector === '*') return all;
    if (selector.includes(','))
      return [
        ...new Set(selector.split(',').flatMap((part) => this.querySelectorAll(part.trim()))),
      ];
    const simple = /^(?:\[[^\]]+\]|:not\(\[[^\]]+\]\))+$/u.test(selector);
    if (simple) {
      const operands = [...selector.matchAll(/(:not\()?\[([^=\]]+)(?:="([^"]*)")?\]\)?/gu)];
      return all.filter((node) =>
        operands.every((operand) => {
          const matches =
            operand[3] === undefined
              ? node.hasAttribute(operand[2]!)
              : node.getAttribute(operand[2]!) === operand[3];
          return operand[1] ? !matches : matches;
        }),
      );
    }
    if (selector.startsWith('.'))
      return all.filter((node) => node.className.split(' ').includes(selector.slice(1)));
    return all.filter((node) => node.localName === selector);
  }
  /** Return the first literal selector match from this physical subtree. */
  querySelector(selector: string): PaintNode | null {
    return this.querySelectorAll(selector)[0] ?? null;
  }
  /** Transport only the owned scroll sibling and its local clip/Stage offsets; browser proof owns affine transforms and rasterization. */
  getBoundingClientRect(): DOMRect {
    const stage = this.className === 'sp-reorder-motion-stage',
      marker = this.className === 'sp-reorder-motion-mount',
      attachment = this.className === 'sp-reorder-motion-layer';
    const translation = marker
      ? /^translate\(([-\d.]+)px,\s*([-\d.]+)px\)$/u.exec(String(this.style.transform))
      : null;
    if (
      marker &&
      this.style.transform !== 'none' &&
      (!translation || !translation.slice(1).every((value) => Number.isFinite(Number(value))))
    )
      throw new Error('fixture-marker-translation-unavailable');
    const parent = stage || marker ? this.parent?.getBoundingClientRect() : null;
    const scroll = attachment ? this.parent : null;
    const box = attachment
      ? {
          left: (scroll?.box.left ?? 0) + (scroll?.clientLeft ?? 0) - (scroll?.scrollLeft ?? 0),
          top: (scroll?.box.top ?? 0) + (scroll?.clientTop ?? 0) - (scroll?.scrollTop ?? 0),
          width: 0,
          height: 0,
        }
      : stage || marker
        ? {
            left:
              (parent?.left ?? 0) +
              (Number.parseFloat(String(this.style.left)) || 0) +
              (translation ? Number(translation[1]) : 0),
            top:
              (parent?.top ?? 0) +
              (Number.parseFloat(String(this.style.top)) || 0) +
              (translation ? Number(translation[2]) : 0),
            width: marker ? Number.parseFloat(String(this.style.width)) || 0 : 200,
            height: marker ? Number.parseFloat(String(this.style.height)) || 0 : 256,
          }
        : this.box;
    return {
      ...box,
      x: box.left,
      y: box.top,
      right: box.left + box.width,
      bottom: box.top + box.height,
    } as DOMRect;
  }
  /** Expose the independently specified single physical fragment. */
  getClientRects(): DOMRect[] {
    return [this.getBoundingClientRect()];
  }
}
/** Literal text nodes retain authored display bytes through the controlled native boundary. */
export class PaintText {
  readonly textContent: string;
  /** Native text data observes the one immutable display value for every constructor path. */
  get data(): string {
    return this.textContent;
  }
  /** Native traversal aliases expose the same actual parent used by append and remove. */
  get parentNode(): PaintNode | null {
    return this.parent;
  }
  /** Text nodes inherit their actual element parent without another tree representation. */
  get parentElement(): PaintNode | null {
    return this.parent;
  }
  readonly nodeType = 3;
  readonly childNodes: never[] = [];
  /** Native text removal retains the literal identity while detaching only its actual parent. */
  remove(): void {
    if (this.parent) {
      const at = this.parent.childNodes.indexOf(this);
      if (at >= 0) this.parent.childNodes.splice(at, 1);
    }
    this.parent = null;
  }
  /** Native text connection follows only its actual retained parent chain. */
  get isConnected(): boolean {
    return this.parent?.isConnected ?? false;
  }
  /** Return the exact UTF-16 length of the immutable native text value. */
  get length(): number {
    return this.textContent.length;
  }
  parent: PaintNode | null = null;
  /** Retain the literal node identity, owner document and independently specified physical rectangle. */
  constructor(textContent: string) {
    this.textContent = textContent;
  }
}
