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
/** One native READ owns positive-axis ancestry composition and its request-local freshness stamps. */
import { requireValue } from '../../core/src/required.js';
interface NativeAxisEntry {
  readonly node: HTMLElement;
  readonly parent: HTMLElement | null;
  readonly root: Node;
  readonly ancestor: HTMLElement | null;
  readonly classes: string;
  readonly inline: string | null;
  readonly transform: string;
  readonly zoom: string;
  readonly x: number;
  readonly y: number;
}
/** A containing context retains the same ordered source operands as native line admission. */
export interface NativeReorderAxisContext {
  readonly x: number;
  readonly y: number;
  readonly context: readonly string[];
  readonly nodes: readonly HTMLElement[];
}
/** Each node's real ancestor style is read once in this synchronous request; no result crosses a READ lifetime. */
export class NativeReorderAxes {
  private readonly entries = new Map<HTMLElement, NativeAxisEntry>();
  private readonly available: () => boolean;
  private readonly style: (node: HTMLElement) => CSSStyleDeclaration;
  /** The caller's exact source issue supplements native ancestry/style stamps around delegated reads. */
  constructor(
    window: Window,
    available: () => boolean,
    style: (node: HTMLElement) => CSSStyleDeclaration = (node) => window.getComputedStyle(node),
  ) {
    this.available = available;
    this.style = style;
  }
  /** Capture a fresh positive axis pair without offsetWidth rounding or a second matrix policy. */
  read(
    node: HTMLElement,
    supplied?: CSSStyleDeclaration,
  ): {
    readonly x: number;
    readonly y: number;
  } | null {
    const pending: Omit<NativeAxisEntry, 'x' | 'y'>[] = [];
    let cursor: HTMLElement | null = node;
    while (cursor && !this.entries.has(cursor)) {
      if (!this.available() || !cursor.isConnected) return null;
      const parent: HTMLElement | null = cursor.parentElement,
        root = cursor.getRootNode(),
        classes = cursor.getAttribute('class') ?? '',
        inline = cursor.getAttribute('style');
      if (!this.available()) return null;
      const style = cursor === node && supplied ? supplied : this.style(cursor);
      if (
        !this.available() ||
        !cursor.isConnected ||
        cursor.parentElement !== parent ||
        cursor.getRootNode() !== root ||
        (cursor.getAttribute('class') ?? '') !== classes ||
        cursor.getAttribute('style') !== inline
      )
        return null;
      const transform = style.getPropertyValue('transform');
      if (!this.available()) return null;
      const zoom = style.getPropertyValue('zoom');
      if (!this.available()) return null;
      const ancestor: HTMLElement | null =
        parent ?? ('host' in root ? ((root as ShadowRoot).host as HTMLElement) : null);
      if (
        !this.available() ||
        !cursor.isConnected ||
        cursor.parentElement !== parent ||
        cursor.getRootNode() !== root ||
        (cursor.getAttribute('class') ?? '') !== classes ||
        cursor.getAttribute('style') !== inline
      )
        return null;
      pending.push({
        node: cursor,
        parent,
        root,
        ancestor,
        classes,
        inline,
        transform,
        zoom,
      });
      cursor = ancestor;
    }
    if (cursor && !this.currentNode(cursor)) return null;
    let x = cursor ? requireValue(this.entries.get(cursor), 'reorder-native-axes').x : 1,
      y = cursor ? requireValue(this.entries.get(cursor), 'reorder-native-axes').y : 1;
    while (pending.length) {
      const entry = requireValue(pending.pop(), 'reorder-native-axes'),
        rawZoom = entry.zoom;
      const zoom =
        !rawZoom || rawZoom === 'normal'
          ? 1
          : Number.parseFloat(rawZoom) * (rawZoom.endsWith('%') ? 0.01 : 1);
      if (entry.transform && entry.transform !== 'none') {
        const matrix = new DOMMatrixReadOnly(entry.transform);
        if (!this.available()) return null;
        const depthAligned =
          matrix.is2D ||
          ([
            matrix.m12,
            matrix.m13,
            matrix.m14,
            matrix.m21,
            matrix.m23,
            matrix.m24,
            matrix.m31,
            matrix.m32,
            matrix.m34,
          ].every((value) => value === 0) &&
            matrix.m33 > 0 &&
            matrix.m44 === 1);
        if (!depthAligned || matrix.b !== 0 || matrix.c !== 0 || matrix.a <= 0 || matrix.d <= 0)
          return null;
        x *= matrix.a;
        y *= matrix.d;
      }
      if (!Number.isFinite(zoom) || zoom <= 0) return null;
      x *= zoom;
      y *= zoom;
      if (!Number.isFinite(x) || !Number.isFinite(y) || x <= 0 || y <= 0) return null;
      this.entries.set(entry.node, { ...entry, x, y });
      if (!this.currentNode(entry.node)) return null;
    }
    return this.available() && this.currentNode(node) ? { x, y } : null;
  }
  /** Check only captured ownership after an external node read; the complete request is fenced before return. */
  currentNode(node: HTMLElement): boolean {
    const entry = this.entries.get(node);
    return (
      !!entry &&
      this.available() &&
      node.isConnected &&
      node.parentElement === entry.parent &&
      node.getRootNode() === entry.root &&
      (node.getAttribute('class') ?? '') === entry.classes &&
      node.getAttribute('style') === entry.inline &&
      this.available()
    );
  }
  /** No result may escape after a literal ancestor mutation that did not create an EditorState. */
  current(): boolean {
    return (
      this.available() &&
      [...this.entries.keys()].every((node) => this.currentNode(node)) &&
      this.available()
    );
  }
  /** The line context keeps the original line-to-root ordering and all of its exact style/identity operands. */
  capture(node: HTMLElement, supplied?: CSSStyleDeclaration): NativeReorderAxisContext | null {
    const axes = this.read(node, supplied);
    if (!axes) return null;
    const context: string[] = [],
      nodes: HTMLElement[] = [];
    let cursor: HTMLElement | null = node;
    while (cursor) {
      const entry: NativeAxisEntry = requireValue(this.entries.get(cursor), 'reorder-native-axes');
      nodes.push(cursor);
      context.push(entry.classes, entry.transform, entry.zoom);
      cursor = entry.ancestor;
    }
    return this.current() ? { ...axes, context, nodes } : null;
  }
}
