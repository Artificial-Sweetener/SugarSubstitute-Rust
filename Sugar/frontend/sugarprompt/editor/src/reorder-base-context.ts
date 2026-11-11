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
/** Native containing-line context supplies an inert collision scene without inspecting proposed fragments. */
import { requireValue } from '../../core/src/required.js';
import { prepareReorderPaintWidth, readReorderPaintSizeWitness } from './reorder-paint-style.js';
import { NativeReorderAxes } from './reorder-native-axes.js';
import type { EditorView } from '@codemirror/view';
import type { SourceRange } from '../../core/src/index.js';
import type { ReorderPreview } from './reorder-preview-contracts.js';
import { reorderPreview } from './reorder-preview.js';
import { prepareReorderStoragePaint } from './reorder-storage-paint.js';
import type { ReorderMarkerRect } from './reorder-geometry.js';
const typography = [
  'font',
  'line-height',
  'letter-spacing',
  'word-spacing',
  'font-feature-settings',
  'font-variation-settings',
  'font-kerning',
  'font-optical-sizing',
  'font-variant-ligatures',
  'font-variant-numeric',
  'font-variant-east-asian',
  'white-space',
  'word-break',
  'overflow-wrap',
  'tab-size',
  'direction',
  'unicode-bidi',
  'text-transform',
  'text-rendering',
  'writing-mode',
  'text-orientation',
  'text-align',
  'text-align-last',
  'line-break',
  'hyphens',
  'text-wrap',
  'text-wrap-mode',
  'text-wrap-style',
  'white-space-collapse',
  'text-autospace',
  'text-spacing-trim',
  'text-combine-upright',
  'text-size-adjust',
] as const;
const lineLayout = [
  ...typography,
  'box-sizing',
  'width',
  'padding-left',
  'padding-right',
  'padding-top',
  'padding-bottom',
  'border-left-width',
  'border-right-width',
  'border-top-width',
  'border-bottom-width',
  'text-indent',
  'vertical-align',
] as const;
/** Preserve actual inherited theme variables and all resolved font controls, without copying measured height. */
function presentation(
  style: CSSStyleDeclaration,
  names: readonly string[],
): readonly (readonly [string, string])[] {
  const properties = new Set(names);
  for (let at = 0; at < style.length; at++) {
    const name = style.item(at);
    if (name.startsWith('--') || name.startsWith('font-')) properties.add(name);
  }
  return [...properties].map((name) => [name, style.getPropertyValue(name)] as const);
}
/** A cold native context retains exact logical edge bounds and resolved physical placement, excluding proposal height. */
export interface ReorderBaseContext {
  /** Actual native border-box height is an extent admission operand, never a typography/cache key. */
  readonly height: number;
  readonly key: string;
  /** Resolved paint and layout identity excludes the containing line's viewport position. */
  readonly paintKey: string;
  /** Exact native containing parent permits replacement lines only within the original structural owner. */
  readonly lineParent: HTMLElement | null;
  /** Exact native tree root distinguishes ShadowRoot parents when parentElement is null. */
  readonly lineRoot: Node;
  /** Native DOM owner, ancestry and literal style/class admission can be checked without a layout read. */
  readonly current: () => boolean;
  /** Stable real content/editor ancestry admits retained display paint across replacement-line publications. */
  readonly appearanceCurrent: () => boolean;
  /** Actual scroller clipping is read-issued separately from proposal-independent paint/position keys. */
  readonly viewport: ReorderMarkerRect;
  readonly prefix: SourceRange;
  readonly suffix: SourceRange;
  readonly classes: string;
  readonly lineClasses: string;
  readonly content: readonly (readonly [string, string])[];
  readonly line: readonly (readonly [string, string])[];
  readonly width: number;
  readonly left: number;
  readonly top: number;
  readonly scaleX: number;
  readonly scaleY: number;
}
/** A read-issued native DOM stamp fences literal context mutations and ancestor replacement during deferred publication. */
function contextCurrent(view: EditorView, nodes: readonly HTMLElement[]): () => boolean {
  const classes = view.themeClasses;
  const stamps = [...new Set([...nodes, view.contentDOM])].map((node) => ({
    node,
    parent: node.parentElement,
    root: node.getRootNode(),
    classes: node.getAttribute('class') ?? '',
    style: node.getAttribute('style'),
  }));
  return () =>
    view.themeClasses === classes &&
    stamps.every(
      (stamp) =>
        stamp.node.isConnected &&
        stamp.node.parentElement === stamp.parent &&
        stamp.node.getRootNode() === stamp.root &&
        (stamp.node.getAttribute('class') ?? '') === stamp.classes &&
        stamp.node.getAttribute('style') === stamp.style,
    );
}
/** Supported CM DOM positions and computed containing styles are read only during requestMeasure. */
export function readReorderBaseContext(
  view: EditorView,
  preview: ReorderPreview,
): ReorderBaseContext | null {
  const sourceState = view.state;
  const range = prepareReorderStoragePaint(preview).replacement,
    first = view.state.doc.lineAt(range.from),
    last = view.state.doc.lineAt(range.to),
    position = view.domAtPos(first.from),
    element = (
      position.node.nodeType === 1 ? position.node : position.node.parentNode
    ) as HTMLElement | null,
    line = element?.closest<HTMLElement>('.cm-line'),
    window = view.dom.ownerDocument.defaultView;
  if (!line || !window) return null;
  const nativeAxes = new NativeReorderAxes(window, () => view.state === sourceState);
  const style = window.getComputedStyle(line);
  if (view.state !== sourceState) return null;
  const contentStyle = window.getComputedStyle(view.contentDOM);
  if (view.state !== sourceState) return null;
  const axes = nativeAxes.capture(line, style);
  if (!axes) return null;
  const witnessWidth = readReorderPaintSizeWitness(style, 'width', () =>
      nativeAxes.currentNode(line),
    ),
    witnessHeight = readReorderPaintSizeWitness(style, 'height', () =>
      nativeAxes.currentNode(line),
    );
  if (witnessWidth === null || witnessHeight === null) return null;
  const box = line.getBoundingClientRect();
  if (!nativeAxes.current()) return null;
  const scaleX = axes.x,
    scaleY = axes.y,
    width = box.width / scaleX,
    firstHeight = box.height / scaleY;
  if (
    !nativeAxes.current() ||
    ![width, firstHeight].every((value) => Number.isFinite(value) && value > 0) ||
    ![box.left, box.top].every(Number.isFinite)
  )
    return null;
  if (
    Math.abs(box.width - witnessWidth * scaleX) > 1 / 16 ||
    Math.abs(box.height - witnessHeight * scaleY) > 1 / 16
  )
    return null;
  const lineWidth = prepareReorderPaintWidth(style, width, () => nativeAxes.currentNode(line));
  if (lineWidth === null) return null;
  const nativeLines: HTMLElement[] = [line];
  let bottom = box.bottom,
    height = firstHeight;
  const publication = view.state.field(reorderPreview, false);
  if (
    preview.presentation !== 'collision' &&
    publication?.native &&
    publication.pointer?.mode === 'stage'
  ) {
    if (last.number - first.number >= 16) return null;
    for (let number = first.number + 1; number <= last.number; number++) {
      const nativePosition = view.domAtPos(view.state.doc.line(number).from),
        nativeElement = (
          nativePosition.node.nodeType === 1 ? nativePosition.node : nativePosition.node.parentNode
        ) as HTMLElement | null,
        nativeLine = nativeElement?.closest<HTMLElement>('.cm-line');
      if (!nativeLine || nativeLines.includes(nativeLine)) return null;
      const nativeBox = nativeLine.getBoundingClientRect();
      if (
        ![nativeBox.left, nativeBox.width, nativeBox.top, nativeBox.bottom].every(
          Number.isFinite,
        ) ||
        Math.abs(nativeBox.left - box.left) > 1 / 16 ||
        Math.abs(nativeBox.width - box.width) > 1 / 16 ||
        Math.abs(nativeBox.top - bottom) > 1 / 16 ||
        nativeBox.bottom <= bottom
      )
        return null;
      nativeLines.push(nativeLine);
      bottom = nativeBox.bottom;
    }
    height = (bottom - box.top) / scaleY;
  }
  const content = presentation(contentStyle, typography),
    layout = presentation(style, lineLayout).map(
      ([name, value]) => [name, name === 'width' ? lineWidth : value] as const,
    ),
    classes = view.themeClasses,
    paint = [
      classes,
      line.className,
      content,
      layout,
      width,
      scaleX,
      scaleY,
      requireValue(axes, 'reorder-base-context').context,
    ],
    viewport = [window.innerWidth, window.innerHeight, window.devicePixelRatio],
    clipping = view.scrollDOM.getBoundingClientRect();
  if (
    ![clipping.left, clipping.right, clipping.top, clipping.bottom].every(Number.isFinite) ||
    clipping.right <= clipping.left ||
    clipping.bottom <= clipping.top
  )
    return null;
  if (!nativeAxes.current()) return null;
  const result = Object.freeze({
    key: JSON.stringify([...paint, box.left, box.top, ...viewport]),
    paintKey: JSON.stringify([...paint, ...viewport]),
    lineParent: line.parentElement,
    lineRoot: line.getRootNode(),
    current: contextCurrent(view, [
      ...requireValue(axes, 'reorder-base-context').nodes,
      ...nativeLines,
    ]),
    appearanceCurrent: contextCurrent(
      view,
      requireValue(axes, 'reorder-base-context').nodes.filter((node) => node !== line),
    ),
    viewport: Object.freeze({
      left: clipping.left,
      right: clipping.right,
      top: clipping.top,
      bottom: clipping.bottom,
    }),
    prefix: { from: first.from, to: range.from },
    suffix: { from: range.to, to: last.to },
    classes,
    lineClasses: line.className,
    content,
    line: layout,
    width,
    height,
    left: box.left,
    top: box.top,
    scaleX,
    scaleY,
  });
  return nativeAxes.current() ? result : null;
}
/** A write resets external placement so the following native read observes its actual ancestor coordinate space. */
export function resetReorderBasePlacement(root: HTMLElement, context: ReorderBaseContext): void {
  root.style.left = '0px';
  root.style.top = '0px';
  root.style.setProperty('width', `${context.width}px`, 'important');
  root.style.setProperty('height', '256px', 'important');
  root.style.setProperty('transform', 'none', 'important');
}
/** The finite external scene preserves native context and genuine scaling while contributing no editor flow or paint. */
export function createReorderBaseScene(
  view: EditorView,
  context: ReorderBaseContext,
): {
  readonly root: HTMLElement;
  readonly line: HTMLElement;
} {
  const document = view.dom.ownerDocument,
    root = document.createElement('div');
  try {
    const scroller = document.createElement('div'),
      content = document.createElement('div'),
      line = document.createElement('div');
    root.className = `cm-editor ${context.classes} sp-reorder-base-measurement`;
    root.setAttribute('aria-hidden', 'true');
    root.setAttribute('inert', '');
    root.contentEditable = 'false';
    root.style.setProperty('position', 'fixed', 'important');
    root.style.setProperty('display', 'block', 'important');
    resetReorderBasePlacement(root, context);
    // Fixed calibration dimensions isolate layout while complete canonical target fragments remain measurable outside the box.
    root.style.setProperty('contain', 'layout size', 'important');
    root.style.setProperty('overflow', 'visible', 'important');
    root.style.setProperty('min-height', '0px', 'important');
    root.style.transformOrigin = '0 0';
    root.style.setProperty('zoom', '1', 'important');
    root.style.opacity = '0';
    root.style.pointerEvents = 'none';
    root.style.margin = '0';
    root.style.padding = '0';
    root.style.border = '0';
    scroller.className = 'cm-scroller';
    scroller.style.setProperty('display', 'block', 'important');
    scroller.style.height = 'auto';
    scroller.style.overflow = 'visible';
    content.className = 'cm-content';
    for (const [name, value] of context.content) content.style.setProperty(name, value);
    content.style.padding = '0';
    content.style.margin = '0';
    content.style.minHeight = '0';
    content.style.width = `${context.width}px`;
    line.className = context.lineClasses;
    for (const [name, value] of context.line) line.style.setProperty(name, value);
    line.style.height = 'auto';
    for (const layer of [scroller, content, line]) {
      layer.style.setProperty('transform', 'none', 'important');
      layer.style.setProperty('zoom', '1', 'important');
    }
    root.append(scroller);
    scroller.append(content);
    content.append(line);
    return { root, line };
  } catch (cause) {
    try {
      root.remove();
    } catch (cleanup) {
      throw new AggregateError([cause, cleanup], 'Reorder base scene cleanup failed');
    }
    throw cause;
  }
}
