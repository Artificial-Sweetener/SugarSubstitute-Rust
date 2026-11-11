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
/** One bounded native read pass freezes literal paint descriptors before any destination allocation or mutation. */
import { requireValue } from '../../core/src/required.js';
import {
  atomicReorderPaintIsland,
  type AtomicReorderPaintRole,
} from './reorder-atomic-paint-island.js';
import { readReorderChipPresentation } from './reorder-chip-presentation.js';
import type { EditorView } from '@codemirror/view';
import { NativeReorderAxes } from './reorder-native-axes.js';
import {
  prepareReorderPaintStyle,
  prepareReorderPaintWidth,
  readReorderPaintProperty,
} from './reorder-paint-style.js';
/** Prepared paint carries only finite derived rendering data and already decoded native pixel sources. */
export type PreparedReorderPaintNode =
  | {
      readonly kind: 'text';
      readonly text: string;
    }
  | {
      readonly kind: 'element';
      readonly tag: string;
      readonly vector: boolean;
      readonly buffer: boolean;
      readonly className: string;
      readonly attributes: readonly (readonly [string, string])[];
      readonly readonly: boolean;
      readonly cssText: string;
      readonly opacity: string;
      readonly display: string;
      readonly transform: string;
      readonly position: string;
      /** Canonical resolved inset comes from the existing prepared native style pass. */
      readonly inset: string;
      readonly generatedOutside: boolean | null;
      readonly children: readonly Node[];
      readonly media: {
        readonly source: HTMLCanvasElement | HTMLImageElement;
        readonly width: number;
        readonly height: number;
      } | null;
    };
/** Full immutable native descriptors expose the same eligibility facts without reading DOM or inventing pixel/width values. */
export function preparedAtomicPaintRole(
  part: PreparedReorderPaintNode | undefined,
): AtomicReorderPaintRole | undefined {
  if (!part || part.kind === 'text') return part;
  return {
    kind: 'element',
    display: part.display,
    position: part.position,
    inset: part.inset,
    transform: part.transform,
    buffer: part.buffer,
    generatedOutside: part.generatedOutside,
    hasMedia: part.media !== null,
    vector: part.vector,
    tag: part.tag,
    children: part.children,
  };
}
/** Original identities index immutable descriptors only; this request-local snapshot owns no editor or cache state. */
export type PreparedReorderPaint = ReadonlyMap<Node, PreparedReorderPaintNode>;
/** Pinned native widget buffers carry zero paint and replaced-inline layout, never host media. */
export function nativeReorderWidgetBuffer(element: HTMLElement): boolean {
  const image = element as HTMLImageElement;
  return (
    element.localName === 'img' &&
    element.className === 'cm-widgetBuffer' &&
    element.getAttribute('src') === null &&
    element.getAttribute('srcset') === null &&
    image.complete &&
    image.naturalWidth === 0 &&
    image.naturalHeight === 0 &&
    image.width === 0
  );
}
/** Admit bounded literal SVG coordinates from the canonical native arrow without interpreting source semantics. */
function vectorCoordinates(value: string | null, count: number): readonly number[] | null {
  if (value === null || value.length > 128) return null;
  const parts = value.trim().split(/[\s,]+/u);
  if (
    parts.length !== count ||
    parts.some((part) => !/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?$/iu.test(part))
  )
    return null;
  const coordinates = parts.map(Number);
  return coordinates.every(Number.isFinite) ? coordinates : null;
}
/** Native used widths retain full DOMRect precision in CSS-local coordinates; ignored inline widths stay ignored. */
function nativePaintWidth(
  element: HTMLElement,
  style: CSSStyleDeclaration,
  axes: NativeReorderAxes,
  replaced: boolean,
  vector: boolean,
  current: () => boolean,
): string | null {
  const width = readReorderPaintProperty(style, 'width');
  if (!current()) return null;
  const display = readReorderPaintProperty(style, 'display');
  if (!current()) return null;
  if (
    vector ||
    !width.endsWith('px') ||
    display === 'none' ||
    display === 'contents' ||
    (display === 'inline' && !replaced)
  )
    return width;
  const scale = axes.read(element, style);
  if (!scale) return null;
  const rect = element.getBoundingClientRect();
  if (!axes.currentNode(element)) return null;
  const borderWidth = rect.width;
  if (!axes.currentNode(element) || !Number.isFinite(borderWidth) || borderWidth < 0) return null;
  return prepareReorderPaintWidth(
    style,
    borderWidth / scale.x,
    () => current() && axes.currentNode(element),
  );
}
/** Complete all original style/attribute/text/media reads for the admitted painted trees before materialization. */
export function prepareNativeReorderPaint(
  view: EditorView,
  roots: readonly Node[],
  admitted: readonly Node[],
  purpose: 'copy' | 'atomic-grab' | 'atomic-peers',
): PreparedReorderPaint | null {
  const state = view.state;
  const sourceStamps: (() => boolean)[] = [];
  const captured = new Map<HTMLElement, () => boolean>();
  const resolved = new Map<HTMLElement, CSSStyleDeclaration>();
  const readStyle = (node: HTMLElement): CSSStyleDeclaration => {
    const previous = resolved.get(node);
    if (previous) return previous;
    const style = requireValue(
      view.dom.ownerDocument.defaultView,
      'reorder-paint-preparation',
    ).getComputedStyle(node);
    resolved.set(node, style);
    return style;
  };
  const axes = new NativeReorderAxes(
    requireValue(view.dom.ownerDocument.defaultView, 'reorder-paint-preparation'),
    () => view.state === state,
    readStyle,
  );
  /** Every delegated eligibility/full read retains exact original native source identity through this same request. */
  const capture = (element: HTMLElement): (() => boolean) => {
    const previous = captured.get(element);
    if (previous) return previous;
    const parent = element.parentElement,
      root = element.getRootNode(),
      classes = element.getAttribute('class') ?? '',
      inline = element.getAttribute('style');
    const current = () =>
      view.state === state &&
      element.isConnected &&
      element.parentElement === parent &&
      element.getRootNode() === root &&
      (element.getAttribute('class') ?? '') === classes &&
      element.getAttribute('style') === inline &&
      view.state === state;
    captured.set(element, current);
    sourceStamps.push(current);
    return current;
  };
  const allowed = new Set(admitted),
    prepared = new Map<Node, PreparedReorderPaintNode>(),
    pending = roots.map((node) => ({ node, inside: false }));
  if (purpose === 'atomic-peers') {
    const roles = new Map<Node, AtomicReorderPaintRole>();
    let unavailable = false;
    /** Resolve only predicate operands; accepted roots later borrow these actual live declarations for complete serialization. */
    const readRole = (node: Node): AtomicReorderPaintRole | undefined => {
      if (unavailable || view.state !== state || !allowed.has(node)) {
        unavailable = true;
        return undefined;
      }
      const previous = roles.get(node);
      if (previous) return previous;
      if (node.nodeType !== 1) {
        const text = node.nodeType === 3 ? (node.textContent ?? '') : '';
        if (view.state !== state || text.length > 16384) {
          unavailable = true;
          return undefined;
        }
        const role = Object.freeze({ kind: 'text' as const, text });
        roles.set(node, role);
        return role;
      }
      const element = node as HTMLElement,
        current = capture(element);
      if (!current()) {
        unavailable = true;
        return undefined;
      }
      const buffer = nativeReorderWidgetBuffer(element);
      if (!current()) {
        unavailable = true;
        return undefined;
      }
      const tag = element.localName,
        vector = element.namespaceURI === 'http://www.w3.org/2000/svg';
      if (!current()) {
        unavailable = true;
        return undefined;
      }
      const hasMedia = !buffer && (tag === 'canvas' || tag === 'img');
      const style = readStyle(element);
      if (!current()) {
        unavailable = true;
        return undefined;
      }
      const values: string[] = [];
      for (const name of ['display', 'position', 'inset', 'transform']) {
        values.push(readReorderPaintProperty(style, name));
        if (!current()) {
          unavailable = true;
          return undefined;
        }
      }
      const island = ['inline-block', 'inline-flex', 'inline-grid'].includes(
        requireValue(values[0], 'reorder-paint-preparation'),
      );
      let generatedOutside: boolean | null = island ? null : false;
      for (const pseudo of island ? [] : ['::before', '::after']) {
        const content = requireValue(
          view.dom.ownerDocument.defaultView,
          'reorder-paint-preparation',
        ).getComputedStyle(element, pseudo).content;
        if (!current()) {
          unavailable = true;
          return undefined;
        }
        if (content !== 'none' && content !== 'normal') {
          generatedOutside = true;
          break;
        }
      }
      const children: Node[] = [];
      if (!buffer && !hasMedia && !island) {
        const native = element.childNodes,
          count = native.length;
        if (!current() || !Number.isSafeInteger(count) || count < 0 || count > allowed.size) {
          unavailable = true;
          return undefined;
        }
        for (let index = 0; index < count; index++) {
          const child = native[index];
          if (!current() || !child || !allowed.has(child)) {
            unavailable = true;
            return undefined;
          }
          children.push(child);
        }
      }
      const role = Object.freeze({
        kind: 'element' as const,
        display: requireValue(values[0], 'reorder-paint-preparation'),
        position: requireValue(values[1], 'reorder-paint-preparation'),
        inset: requireValue(values[2], 'reorder-paint-preparation'),
        transform: requireValue(values[3], 'reorder-paint-preparation'),
        buffer,
        generatedOutside,
        hasMedia,
        vector,
        tag: hasMedia || buffer ? 'canvas' : vector || tag === 'wbr' || tag === 'br' ? tag : 'span',
        children: Object.freeze(children),
      });
      roles.set(node, role);
      return role;
    };
    pending.length = 0;
    for (const node of roots) {
      if (atomicReorderPaintIsland(node, readRole)) pending.push({ node, inside: false });
      if (unavailable) return null;
    }
    // Cross-root mutations must reject before any complete paint serialization.
    if (!sourceStamps.every((current) => current())) return null;
  }
  while (pending.length) {
    const { node: original, inside } = requireValue(pending.pop(), 'reorder-paint-preparation');
    if (!allowed.has(original)) return null;
    if (prepared.has(original)) continue;
    if (original.nodeType !== 1) {
      prepared.set(
        original,
        Object.freeze({
          kind: 'text',
          text: original.nodeType === 3 ? (original.textContent ?? '') : '',
        }),
      );
      continue;
    }
    const element = original as HTMLElement,
      buffer = nativeReorderWidgetBuffer(element),
      media = !buffer && (element.localName === 'canvas' || element.localName === 'img'),
      vector = element.namespaceURI === 'http://www.w3.org/2000/svg';
    if (vector && element.localName !== 'svg' && element.localName !== 'polygon') return null;
    const sourceCurrent = capture(element);
    if (!sourceCurrent()) return null;
    const attributes: (readonly [string, string])[] = [];
    if (vector) {
      const names =
        element.localName === 'svg' ? ['viewBox', 'width', 'height'] : ['points', 'fill'];
      for (const name of names) {
        const value = element.getAttribute(name);
        if (value === null || value.length > 128) return null;
        const coordinates =
          name === 'viewBox'
            ? vectorCoordinates(value, 4)
            : name === 'points'
              ? vectorCoordinates(value, 6)
              : null;
        const valid =
          name === 'fill'
            ? value === 'currentColor'
            : name === 'width' || name === 'height'
              ? /^(?:\d+(?:\.\d*)?|\.\d+)%?$/u.test(value) &&
                Number.isFinite(Number.parseFloat(value))
              : !!coordinates &&
                (name !== 'viewBox' ||
                  (requireValue(coordinates[2], 'reorder-paint-preparation') > 0 &&
                    requireValue(coordinates[3], 'reorder-paint-preparation') > 0));
        if (!valid) return null;
        attributes.push(Object.freeze([name, value] as const));
      }
    }
    for (const [name, allowedValues] of [
      ['data-media', ['ready', 'pending', 'unavailable']],
      [
        'data-resolution',
        ['found', 'missing', 'ambiguous', 'pending', 'pending-authority', 'unavailable'],
      ],
      ['data-region-named', ['true', 'false']],
    ] as const) {
      const value = element.getAttribute(name);
      if (value !== null && (allowedValues as readonly string[]).includes(value))
        attributes.push(Object.freeze([name, value] as const));
    }
    const presentation =
      element.getAttribute('data-reorder-native-value') !== null
        ? readReorderChipPresentation(element)
        : null;
    const className =
      (typeof element.className === 'string'
        ? element.className
        : (element.getAttribute('class') ?? '')) +
      (presentation?.active ? ' sp-reorder-chip-active' : '') +
      (presentation?.held ? ' sp-reorder-held' : '');
    const originalStyle = readStyle(element);
    if (!sourceCurrent()) return null;
    const width = nativePaintWidth(
      element,
      originalStyle,
      axes,
      media || buffer,
      vector,
      sourceCurrent,
    );
    if (width === null || !sourceCurrent()) return null;
    const style = prepareReorderPaintStyle(originalStyle, undefined, undefined, width);
    if (!sourceCurrent()) return null;
    const island = inside || ['inline-block', 'inline-flex', 'inline-grid'].includes(style.display);
    const generatedOutside =
      purpose === 'copy' || island
        ? null
        : ['::before', '::after'].some((pseudo) => {
            const content = requireValue(
              view.dom.ownerDocument.defaultView,
              'reorder-paint-preparation',
            ).getComputedStyle(element, pseudo).content;
            return content !== 'none' && content !== 'normal';
          });
    const children = Object.freeze(media || buffer ? [] : Array.from(original.childNodes));
    let pixels: {
      readonly source: HTMLCanvasElement | HTMLImageElement;
      readonly width: number;
      readonly height: number;
    } | null = null;
    if (media) {
      const source = element as HTMLCanvasElement | HTMLImageElement,
        width =
          element.localName === 'img'
            ? (source as HTMLImageElement).naturalWidth
            : (source as HTMLCanvasElement).width,
        height =
          element.localName === 'img'
            ? (source as HTMLImageElement).naturalHeight
            : (source as HTMLCanvasElement).height;
      if (
        (element.localName === 'img' && !(source as HTMLImageElement).complete) ||
        !Number.isSafeInteger(width) ||
        !Number.isSafeInteger(height) ||
        width <= 0 ||
        height <= 0 ||
        width * height > 1048576
      )
        return null;
      pixels = Object.freeze({ source, width, height });
    }
    prepared.set(
      original,
      Object.freeze({
        kind: 'element',
        tag:
          media || buffer
            ? 'canvas'
            : vector || element.localName === 'wbr' || element.localName === 'br'
              ? element.localName
              : 'span',
        vector,
        buffer,
        className,
        attributes: Object.freeze(attributes),
        readonly: element.getAttribute('contenteditable') === 'false',
        cssText: style.cssText,
        opacity: style.opacity,
        display: style.display,
        transform: style.transform,
        position: style.position,
        inset: style.inset,
        generatedOutside,
        children,
        media: pixels,
      }),
    );
    pending.push(...children.map((node) => ({ node, inside: island })));
  }
  return axes.current() && sourceStamps.every((current) => current()) ? prepared : null;
}
