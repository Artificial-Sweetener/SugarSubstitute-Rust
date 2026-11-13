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
/** Reorder paint consumes the canonical rich compilation and Core origins, never another syntax compiler. */
import { requireValue } from '../../core/src/required.js';
import type { CanonicalRowPaint } from './canonical-row-paint.js';
import { documentKind } from './document-kind.js';
import { mountCanonicalRowEdges, releaseCanonicalRowEdges } from './canonical-row-edges.js';
import type { ReadonlyWidgetMedia } from './readonly-widget-media.js';
import { RangeSet, type EditorState, type Range } from '@codemirror/state';
import { Decoration, type DecorationSet, type EditorView } from '@codemirror/view';
import type {
  ReorderOutput,
  ReorderText,
  SourceRange,
  ReorderOriginPiece,
} from '../../core/src/index.js';
import type { NativeDocumentValueTarget } from './document-values.js';
import {
  currentCanonicalProjection,
  currentSourceDisplayProjection,
  richProjection,
  projectsRichSyntax,
} from './rich-projection.js';
import type { PreparedProjection } from './projection-contracts.js';
import { SourceWidget } from './projection-widgets.js';
/** Canonical decoration instances use the captured value's decoded UTF-16 coordinates. */
export interface ReorderProjection {
  /** Full logical-document UTF-16 compilation identity, independent of transient exposure. */
  readonly canonical: DecorationSet;
  /** Full logical-document UTF-16 readonly source paint after native selection/reveal/held exposure. */
  readonly visible: DecorationSet;
  readonly decoded: DecorationSet;
  /** Scene-free ordinary syntax includes the canonical rich operands originally hidden by heading ownership. */
  readonly ordinary: DecorationSet;
  /** Captured host feedback is already covered by the projection freshness identity. */
  readonly sceneErrors: import('./scene-errors.js').SceneErrors;
}
/** Every independently mounted rich widget has one finite display-only DOM lifetime. */
interface MountedWidget {
  readonly widget: SourceWidget;
  readonly dom: HTMLElement;
}
/** Actual projection-owner identity fences presentation changes independently of compilation reuse. */
const presentations = new WeakMap<ReorderProjection, PreparedProjection>();
const mounts = new WeakMap<HTMLElement, readonly MountedWidget[]>();
/** Clip canonical logical paint while promoting containing-line attributes and retaining complete widget leases. */
export function canonicalReorderFragment(
  state: EditorState,
  canonical: DecorationSet,
  range: SourceRange,
): DecorationSet {
  const ranges: Range<Decoration>[] = [];
  canonical.between(state.doc.lineAt(range.from).from, range.to, (from, to, value) => {
    const attributes = from === to ? markAttributes(value) : null;
    // Prompt rails consume whole-row witnesses; bounded structured marks and separator presentation stay inline.
    if (
      state.facet(documentKind) === 'prompt' &&
      attributes?.class?.split(/\s+/u).includes('sp-region-content')
    )
      return;
    const decoration =
      from === to
        ? Decoration.mark({
            attributes: markAttributes(value),
            inclusive: true,
          })
        : value;
    const endOfSource = from === to ? state.doc.lineAt(from).to : to;
    const start = Math.max(from, range.from),
      end = Math.min(endOfSource, range.to);
    if (start >= end) return;
    if (value.spec.widget instanceof SourceWidget && (start !== from || end !== to))
      throw new Error('reorder-projection-clipped-widget');
    ranges.push(decoration.range(start, end));
  });
  return Decoration.set(ranges, true);
}
/** Adapt already prepared widget/mark endpoints once; structured storage escapes never become displayed syntax. */
export function prepareReorderProjection(
  state: EditorState,
  target: NativeDocumentValueTarget,
): ReorderProjection | null {
  if (!state.field(richProjection, false)) return null;
  const prepared = currentSourceDisplayProjection(state);
  if (!prepared) return null;
  const mapping = requireValue(target.entry.semanticMapping, 'reorder-projection'),
    ranges: Range<Decoration>[] = [];
  canonicalReorderFragment(state, prepared.decorations, mapping.logicalRange).between(
    mapping.logicalRange.from,
    mapping.logicalRange.to,
    (from, to, value) => {
      const decodedFrom = mapping.logicalToDecoded[from - mapping.logicalRange.from],
        decodedTo = mapping.logicalToDecoded[to - mapping.logicalRange.from];
      if (decodedFrom == null || decodedTo == null)
        throw new Error('reorder-projection-unmapped-endpoint');
      ranges.push(value.range(decodedFrom, decodedTo));
    },
  );
  const latent: Range<Decoration>[] = [];
  prepared.sceneSyntax.between(
    mapping.logicalRange.from,
    mapping.logicalRange.to,
    (from, to, value) => {
      latent.push(value.range(from, to));
    },
  );
  const ordinary = prepared.decorations.update({
    filter: (_from, _to, value) => value.spec.scene !== true,
    add: latent,
    sort: true,
  });
  const decodedOrdinary: Range<Decoration>[] = [];
  canonicalReorderFragment(state, ordinary, mapping.logicalRange).between(
    mapping.logicalRange.from,
    mapping.logicalRange.to,
    (from, to, value) => {
      const start = mapping.logicalToDecoded[from - mapping.logicalRange.from],
        end = mapping.logicalToDecoded[to - mapping.logicalRange.from];
      if (start == null || end == null) throw new Error('reorder-projection-unmapped-endpoint');
      decodedOrdinary.push(value.range(start, end));
    },
  );
  const projection = Object.freeze({
    canonical: prepared.all,
    visible: prepared.decorations,
    decoded: Decoration.set(ranges, true),
    ordinary: Decoration.set(decodedOrdinary, true),
    sceneErrors: prepared.sceneErrors,
  });
  presentations.set(projection, state.field(richProjection));
  return projection;
}
/** Resource/profile changes retire captured paint through the canonical projection's own freshness rules. */
export function currentReorderProjection(
  state: EditorState,
  projection: ReorderProjection | null,
): boolean {
  return projection
    ? presentations.has(projection) &&
        presentations.get(projection) === state.field(richProjection, false) &&
        currentCanonicalProjection(state)?.all === projection.canonical
    : !state.field(richProjection, false) || !projectsRichSyntax(state);
}
/** Remap existing decorations over one exact original source fragment; replacement widgets cannot cross origin cuts. */
function copyDecorations(
  decorations: DecorationSet,
  source: SourceRange,
  outputFrom: number,
  ranges: Range<Decoration>[],
): void {
  decorations.between(source.from, source.to, (from, to, value) => {
    const start = Math.max(from, source.from),
      end = Math.min(to, source.to);
    if (start >= end) return;
    if (value.spec.widget instanceof SourceWidget && (start !== from || end !== to))
      throw new Error('reorder-projection-split-widget');
    ranges.push(value.range(outputFrom + start - source.from, outputFrom + end - source.from));
  });
}
/** Preview source origins retain copied/reopened shells and composed scalar pieces without parsing output. */
export function projectReorderOutput(
  projection: ReorderProjection | null,
  output: ReorderOutput,
): DecorationSet {
  return projectReorderPieces(projection, output.origin.pieces);
}
/** An affected projected hard-line recipe remaps only its actual canonical source intervals. */
export function projectReorderPieces(
  projection: ReorderProjection | null,
  pieces: readonly ReorderOriginPiece[],
): DecorationSet {
  if (!projection) return Decoration.none;
  const ranges: Range<Decoration>[] = [];
  for (const piece of pieces) {
    if (piece.kind === 'derived') continue;
    if (piece.kind !== 'composed') {
      copyDecorations(projection.decoded, piece.sourceRange, piece.outputRange.from, ranges);
      continue;
    }
    let cursor = piece.outputRange.from;
    for (const part of piece.components) {
      if (part.kind === 'derived') cursor += part.text.length;
      else {
        copyDecorations(projection.decoded, part.sourceRange, cursor, ranges);
        cursor += part.sourceRange.to - part.sourceRange.from;
      }
    }
  }
  return Decoration.set(ranges, true);
}
/** Proposed Scene context overlays the same source-origin adapter using its captured ordinary syntax. */
export function projectOrdinaryReorderOutput(
  projection: ReorderProjection | null,
  output: ReorderOutput,
): DecorationSet {
  return projection
    ? projectReorderPieces({ ...projection, decoded: projection.ordinary }, output.origin.pieces)
    : Decoration.none;
}
/** An isolated proxy uses Core's original chip serializer and the same canonical widget/mark instances. */
export function projectReorderText(
  projection: ReorderProjection | null,
  text: ReorderText,
): DecorationSet {
  if (!projection) return Decoration.none;
  const ranges: Range<Decoration>[] = [];
  let cursor = 0;
  for (const piece of text.pieces) {
    if (piece.kind === 'derived') cursor += piece.text.length;
    else {
      copyDecorations(projection.decoded, piece.sourceRange, cursor, ranges);
      cursor += piece.sourceRange.to - piece.sourceRange.from;
    }
  }
  return Decoration.set(ranges, true);
}
/** Validate literal prepared mark attributes at the CodeMirror spec boundary without recomputing their visual policy. */
function markAttributes(value: Decoration): Record<string, string> {
  const attributes: unknown = value.spec.attributes;
  const result: Record<string, string> = {};
  if (typeof attributes === 'object' && attributes !== null)
    for (const [name, attribute] of Object.entries(attributes)) {
      if (typeof attribute !== 'string') throw new Error('reorder-projection-invalid-mark');
      result[name] = attribute;
    }
  return result;
}
/** Apply literal public mark attributes, preserving safe dynamic labels and the canonical visual classes. */
function markElement(document: Document, value: Decoration): HTMLElement {
  const element = document.createElement('span');
  const className: unknown = value.spec.class;
  if (typeof className === 'string') element.className = className;
  for (const [name, attribute] of Object.entries(markAttributes(value))) {
    element.setAttribute(name, attribute);
  }
  return element;
}
/** Paint readonly inline replacements with literal word-break opportunities through supported ranges and shared widget renderers. */
export function renderReorderFragment(
  view: EditorView,
  root: HTMLElement,
  literal: (range: SourceRange) => string,
  range: SourceRange,
  decorations: DecorationSet,
  rowPaint: CanonicalRowPaint | null,
  media?: ReadonlyWidgetMedia,
): void {
  if (mounts.has(root)) throw new Error('reorder-fragment-already-mounted');
  if (!rowPaint && (range.from === range.to || decorations === Decoration.none)) {
    root.textContent = range.from === range.to ? '' : literal(range);
    return;
  }
  const mounted: MountedWidget[] = [];
  mounts.set(root, mounted);
  /** Retired fragments never acquire late edge resources; the caller retains publication currentness. */
  const finish = (): void => {
    if (rowPaint && mounts.get(root) === mounted)
      mountCanonicalRowEdges(view, root, rowPaint, () => mounts.get(root) === mounted);
  };
  if (range.from === range.to || decorations === Decoration.none) {
    try {
      root.textContent = range.from === range.to ? '' : literal(range);
      finish();
    } catch (primary) {
      try {
        retireReorderFragment(root, mounted);
      } catch (cleanup) {
        throw new AggregateError([primary, cleanup], 'Reorder rich paint cleanup failed');
      }
      throw primary;
    }
    return;
  }
  const document = view.dom.ownerDocument,
    replacements = new Map<
      number,
      {
        readonly to: number;
        readonly value: Decoration;
      }[]
    >();
  decorations.between(range.from, range.to, (from, to, value) => {
    if (value.spec.widget instanceof SourceWidget && from >= range.from && to <= range.to) {
      const at = replacements.get(from) ?? [];
      at.push({ to, value });
      replacements.set(from, at);
    }
  });
  const open: Decoration[] = [],
    parents: HTMLElement[] = [root];
  /** Public range precedence and continued-range counts preserve one canonical ancestor around each complete literal or inline replacement group. */
  const append = (
    nodes: readonly Node[],
    active: readonly Decoration[],
    openStart: number,
  ): void => {
    let continued = 0;
    while (
      continued < openStart &&
      continued < open.length &&
      continued < active.length &&
      requireValue(open[continued], 'reorder-projection').eq(
        requireValue(active[active.length - 1 - continued], 'reorder-projection'),
      )
    )
      continued++;
    open.length = continued;
    parents.length = continued + 1;
    for (let at = active.length - 1 - continued; at >= 0; at--) {
      const mark = requireValue(active[at], 'reorder-projection'),
        wrapper = markElement(document, mark);
      requireValue(parents[parents.length - 1], 'reorder-projection').append(wrapper);
      open.push(mark);
      parents.push(wrapper);
    }
    requireValue(parents[parents.length - 1], 'reorder-projection').append(...nodes);
  };
  try {
    RangeSet.spans([decorations], range.from, range.to, {
      /** Preserve literal source between canonical replacement widgets. */
      span(from, to, active, openStart) {
        append([document.createTextNode(literal({ from, to }))], active, openStart);
      },
      /** Mount one complete readonly inline replacement with zero-text break opportunities on both boundaries, retaining canonical mark precedence. */
      point(from, to, value, active, openStart) {
        if (from === to) return;
        if (!replacements.get(from)?.some((item) => item.to === to && item.value === value))
          throw new Error('reorder-projection-fragment-split-widget');
        const widget: unknown = value.spec.widget;
        if (!(widget instanceof SourceWidget)) throw new Error('reorder-projection-invalid-widget');
        const dom = media ? widget.toReadonlyDOM(view, media) : widget.toDOM(view);
        mounted.push({ widget, dom });
        dom.contentEditable = 'false';
        append(
          [document.createElement('wbr'), dom, document.createElement('wbr')],
          active,
          openStart,
        );
      },
    });
    finish();
  } catch (cause) {
    try {
      retireReorderFragment(root, mounted);
    } catch (cleanup) {
      throw new AggregateError([cause, cleanup], 'Reorder rich paint cleanup failed');
    }
    throw cause;
  }
}
/** Retire every actual mount once, preserving all unexpected cleanup causes. */
export function releaseReorderFragment(root: HTMLElement): void {
  const mounted = mounts.get(root);
  if (mounted) retireReorderFragment(root, mounted);
}
/** An obsolete renderer cannot retire a replacement registered reentrantly on the same native root. */
function retireReorderFragment(root: HTMLElement, mounted: readonly MountedWidget[]): void {
  if (mounts.get(root) !== mounted) return;
  mounts.delete(root);
  const failures: unknown[] = [];
  try {
    releaseCanonicalRowEdges(root);
  } catch (cause) {
    failures.push(cause);
  }
  for (const { widget, dom } of mounted) {
    try {
      widget.destroy(dom);
    } catch (cause) {
      failures.push(cause);
    }
  }
  if (failures.length === 1) throw failures[0];
  if (failures.length) throw new AggregateError(failures, 'Reorder rich widget cleanup failed');
}
