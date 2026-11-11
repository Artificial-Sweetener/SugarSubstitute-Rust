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
/** Reorder provenance adapts the existing authored index through exact Core origin pieces before native issuance. */
import { requireValue } from '../../core/src/required.js';
import type { ChangeSet, EditorState } from '@codemirror/state';
import {
  reorderOriginBoundary,
  type PreparedDocumentPromptScopes,
  type ReorderOutput,
  type SourceRange,
} from '../../core/src/index.js';
import { sourcePositionsToDocument, type EncodedSource } from '../../core/src/line-endings.js';
import { preparedAuthorship, type Authorship, type GeneratedShell } from './authorship-ranges.js';
import type { NativeDocumentValueTarget } from './document-values.js';
import {
  reorderProvenanceSource,
  type DocumentValueReorderProvenance,
  type ReorderAuthorshipReplacement,
} from './reorder-provenance.js';
/** An original boundary must have exactly one associated usable image; many/interior images are explicit refusal. */
function uniqueBoundary(
  output: ReorderOutput,
  position: number,
  association: -1 | 1,
): number | null {
  const result = reorderOriginBoundary(output, position, association);
  if (result.status !== 'observed') return null;
  const image = result.images[0];
  return result.images.length === 1 && image?.kind === 'boundary' ? image.position : null;
}
/** Adjacent authored fragments form one interval only when no newly derived byte separates them. */
function joinedLiteralImages(images: readonly SourceRange[]): readonly SourceRange[] {
  const merged: SourceRange[] = [];
  for (const image of images) {
    const last = merged.at(-1);
    if (last?.to === image.from) merged[merged.length - 1] = { from: last.from, to: image.to };
    else if (image.from < image.to) merged.push(image);
  }
  return merged;
}
/** Adapt only exact original intervals; joins across newly derived bytes never inherit literal intent. */
function literalImages(output: ReorderOutput, range: SourceRange): readonly SourceRange[] | null {
  const images: SourceRange[] = [];
  for (const piece of output.origin.pieces) {
    if (piece.kind === 'derived') continue;
    let position = piece.outputRange.from;
    const parts = piece.kind === 'composed' ? piece.components : [piece];
    for (const part of parts) {
      const length =
        part.kind === 'derived' ? part.text.length : part.sourceRange.to - part.sourceRange.from;
      if (part.kind !== 'derived') {
        const from = Math.max(range.from, part.sourceRange.from),
          to = Math.min(range.to, part.sourceRange.to);
        if (from < to)
          images.push({
            from: position + from - part.sourceRange.from,
            to: position + to - part.sourceRange.from,
          });
      }
      position += length;
    }
  }
  const merged = joinedLiteralImages(images);
  for (const image of merged) {
    for (const endpoint of [image.from, image.to]) {
      const c = output.text.charCodeAt(endpoint),
        previous = output.text.charCodeAt(endpoint - 1);
      if (
        (previous === 13 && c === 10) ||
        (previous >= 0xd800 && previous <= 0xdbff && c >= 0xdc00 && c <= 0xdfff)
      )
        return null;
    }
  }
  return merged;
}
/** Use the issuer's sole post-scope parse and exact container conversion; never issue a fake normalization receipt. */
export function reorderedAuthorship(
  state: EditorState,
  target: NativeDocumentValueTarget,
  provenance: DocumentValueReorderProvenance,
  postScopes: PreparedDocumentPromptScopes,
  changes: ChangeSet,
  encoded: EncodedSource,
  replacement: ReorderAuthorshipReplacement,
):
  | {
      readonly status: 'ready';
      readonly value: Authorship;
    }
  | {
      readonly status: 'rejected';
      readonly reason: 'unsafe-origin';
    } {
  const receipt = reorderProvenanceSource(state, target, provenance, replacement);
  const rejected = { status: 'rejected', reason: 'unsafe-origin' } as const;
  if (!receipt) return rejected;
  const output = receipt.output,
    post = postScopes.byId[target.scope.id],
    container = postScopes.values.containersById[target.scope.container.id];
  const padding = target.scope.container.valueRange.from;
  if (
    !post ||
    !container?.mapping ||
    container.mapping.text !==
      requireValue(target.scope.container.mapping, 'reorder-authorship').text.slice(0, padding) +
        output.text +
        requireValue(target.scope.container.mapping, 'reorder-authorship').text.slice(
          target.scope.container.valueRange.to,
        )
  )
    return rejected;
  const shift = padding - container.valueRange.from;
  const native = requireValue(target.entry.semanticMapping, 'reorder-authorship'),
    positions: number[] = [];
  /** Native bounds enter Core only through the actual semantic inverse map. */
  const decoded = (at: number): number | null =>
    native.logicalToDecoded[at - native.logicalRange.from] ?? null;
  /** Queue exact storage endpoints for one batch native conversion. */
  const queueFull = (at: number): number | null => {
    const exact = requireValue(container.mapping, 'reorder-authorship').decodedToSource[at];
    if (exact == null) return null;
    if (postScopes.values.source[exact - 1] === '\r' && postScopes.values.source[exact] === '\n')
      return null;
    positions.push(exact);
    return positions.length - 1;
  };
  /** Output offsets remain relative to the original semantic padding, including post-trimmed edge carriers. */
  const queue = (at: number): number | null => queueFull(padding + at);
  const generated: GeneratedShell[] = [],
    literals: SourceRange[] = [];
  for (const shell of receipt.generated) {
    const from = decoded(shell.from),
      to = decoded(shell.to),
      contentFrom = decoded(shell.contentFrom),
      contentTo = decoded(shell.contentTo);
    if (from === null && to === null) {
      if (
        !(
          shell.to < target.entry.logicalStorage.from || shell.from > target.entry.logicalStorage.to
        )
      )
        return rejected;
      generated.push({
        ...shell,
        from: -changes.mapPos(shell.from, 1) - 1,
        to: -changes.mapPos(shell.to, -1) - 1,
        contentFrom: -changes.mapPos(shell.contentFrom, -1) - 1,
        contentTo: -changes.mapPos(shell.contentTo, 1) - 1,
      });
      continue;
    }
    if (from === null || to === null || contentFrom === null || contentTo === null) return rejected;
    const original = target.scope.document.emphasisSpans.find(
      (s) =>
        s.outerRange.from === from &&
        s.outerRange.to === to &&
        s.contentRange.from === contentFrom &&
        s.contentRange.to === contentTo,
    );
    if (!original) return rejected;
    let occurrences = output.origin.envelopes.filter((o) => o.span === original);
    if (!occurrences.length) {
      const a = uniqueBoundary(output, from, 1),
        b = uniqueBoundary(output, to, -1),
        c = uniqueBoundary(output, contentFrom, 1),
        d = uniqueBoundary(output, contentTo, -1);
      if (a === null || b === null || c === null || d === null) return rejected;
      occurrences = [
        {
          span: original,
          outerRange: { from: a, to: b },
          contentRange: { from: c, to: d },
        },
      ];
    }
    for (const occurrence of occurrences) {
      const span = post.document.emphasisSpans.find(
        (s) =>
          s.outerRange.from === occurrence.outerRange.from + shift &&
          s.outerRange.to === occurrence.outerRange.to + shift &&
          s.contentRange.from === occurrence.contentRange.from + shift &&
          s.contentRange.to === occurrence.contentRange.to + shift,
      );
      if (!span) return rejected;
      const bounds = [
        queue(occurrence.outerRange.from),
        queue(occurrence.outerRange.to),
        queue(occurrence.contentRange.from),
        queue(occurrence.contentRange.to),
      ];
      if (bounds.some((b) => b === null)) return rejected;
      generated.push({
        from: requireValue(bounds[0], 'reorder-authorship'),
        to: requireValue(bounds[1], 'reorder-authorship'),
        contentFrom: requireValue(bounds[2], 'reorder-authorship'),
        contentTo: requireValue(bounds[3], 'reorder-authorship'),
        nestingDepth: shell.nestingDepth,
      });
    }
  }
  for (const literal of receipt.literals) {
    const full = requireValue(target.entry.fullMapping, 'reorder-authorship'),
      from = full.logicalToDecoded[literal.from - full.logicalRange.from] ?? null,
      to = full.logicalToDecoded[literal.to - full.logicalRange.from] ?? null;
    if (from === null && to === null) {
      if (
        !(
          literal.to < target.entry.logicalStorage.from ||
          literal.from > target.entry.logicalStorage.to
        )
      )
        return rejected;
      literals.push({
        from: -changes.mapPos(literal.from, 1) - 1,
        to: -changes.mapPos(literal.to, -1) - 1,
      });
      continue;
    }
    if (from === null || to === null) return rejected;
    const oldEnd = target.scope.container.valueRange.to,
      images = literalImages(output, {
        from: Math.max(0, from - padding),
        to: Math.min(oldEnd, to) - padding,
      });
    if (!images) return rejected;
    const all: SourceRange[] = [
      ...(from < padding ? [{ from, to: Math.min(to, padding) }] : []),
      ...images.map((r) => ({ from: padding + r.from, to: padding + r.to })),
      ...(to > oldEnd
        ? [
            {
              from: padding + output.text.length + Math.max(from - oldEnd, 0),
              to: padding + output.text.length + to - oldEnd,
            },
          ]
        : []),
    ];
    for (const image of joinedLiteralImages(all)) {
      const a = queueFull(image.from),
        b = queueFull(image.to);
      if (a === null || b === null) return rejected;
      literals.push({ from: a, to: b });
    }
  }
  const logical = sourcePositionsToDocument(encoded.logical, encoded.endings, positions);
  /** Negative slots are untouched actual ChangeSet positions; other slots consume the one encoded post map. */
  const resolve = (at: number): number =>
    at < 0 ? -at - 1 : requireValue(logical[at], 'reorder-authorship');
  return {
    status: 'ready',
    value: preparedAuthorship(
      generated.map((s) => ({
        ...s,
        from: resolve(s.from),
        to: resolve(s.to),
        contentFrom: resolve(s.contentFrom),
        contentTo: resolve(s.contentTo),
      })),
      literals.map((r) => ({ from: resolve(r.from), to: resolve(r.to) })),
    ),
  };
}
