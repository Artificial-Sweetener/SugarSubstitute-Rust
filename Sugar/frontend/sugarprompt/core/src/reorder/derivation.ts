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
/** Chip derivation delegates grammar to actual parsed spans and physical endings to the shared line owner. */
import { requireValue } from '../required.js';
import type { SourceRange } from '../offsets.js';
import type {
  PreparedDocumentPromptScope,
  PreparedDocumentPromptScopes,
} from '../document/prompt-scopes.js';
import type { PromptDocument, PromptSegment } from '../prompt/contracts.js';
import { concatText, sourceText } from './source-pieces.js';
import { freezeStructure, strip } from '../prompt/text.js';
import type { PreparedValueReorder, ReorderChip, ReorderEnvelope } from './contracts.js';
import { ReorderDerivationSpans } from './derivation-spans.js';
/** Trim horizontal padding without parsing syntax or changing exact source coordinates. */
function horizontalRange(source: string, range: SourceRange): SourceRange {
  let { from, to } = range;
  while (from < to && ' \t'.includes(requireValue(source[from], 'derivation'))) from++;
  while (to > from && ' \t'.includes(requireValue(source[to - 1], 'derivation'))) to--;
  return { from, to };
}
/** Split source-owned text around outermost actual LoRA spans, conserving boundary padding. */
function splitLoras(
  documentScope: PreparedDocumentPromptScope,
  document: PromptDocument,
  spans: ReorderDerivationSpans,
  chip: ReorderChip,
): ReorderChip[] {
  const eligible = spans.loras(chip.contentRange);
  const children: ReorderChip[] = [];
  let cursor = chip.contentRange.from;
  /** Keep visible text; surrounding whitespace becomes a separator rather than another item. */
  const textChild = (to: number): void => {
    const range = horizontalRange(document.sourceText, { from: cursor, to });
    if (range.from !== range.to)
      children.push({
        ...chip,
        text: document.sourceText.slice(range.from, range.to),
        contentRange: range,
        leading: sourceText(documentScope, 0, 0),
        trailing: sourceText(documentScope, 0, 0),
      });
  };
  for (const span of eligible) {
    if (span.outerRange.from < cursor) continue;
    textChild(span.outerRange.from);
    children.push({
      ...chip,
      text: document.sourceText.slice(span.outerRange.from, span.outerRange.to),
      contentRange: span.outerRange,
      leading: sourceText(documentScope, 0, 0),
      trailing: sourceText(documentScope, 0, 0),
    });
    cursor = span.outerRange.to;
  }
  if (!children.length) return [chip];
  textChild(chip.contentRange.to);
  return children.map((child, index) => ({
    ...child,
    separatorRange:
      index + 1 < children.length
        ? {
            from: child.contentRange.to,
            to: requireValue(children[index + 1], 'derivation').contentRange.from,
          }
        : chip.separatorRange,
    leading:
      index === 0
        ? concatText(
            chip.leading,
            sourceText(documentScope, chip.contentRange.from, child.contentRange.from),
          )
        : sourceText(documentScope, 0, 0),
    trailing:
      index === children.length - 1
        ? concatText(
            sourceText(documentScope, child.contentRange.to, chip.contentRange.to),
            chip.trailing,
          )
        : sourceText(documentScope, 0, 0),
  }));
}
/** Expand a genuine parsed segment; nested commas come only from its actual emphasis metadata. */
function expandSegment(
  documentScope: PreparedDocumentPromptScope,
  document: PromptDocument,
  spans: ReorderDerivationSpans,
  segment: PromptSegment,
  partition: number,
  envelopes: readonly ReorderEnvelope[],
): ReorderChip[] {
  const source = document.sourceText;
  const range = horizontalRange(source, segment.contentRange);
  const shell = spans.emphasis(range);
  if (
    shell &&
    !strip(source.slice(shell.contentRange.from, shell.contentRange.to), 'right').endsWith(',') &&
    (shell.segments.length > 1 || spans.hasLora(range))
  ) {
    const envelope: ReorderEnvelope = {
      span: shell,
      opening: source.slice(shell.outerRange.from, shell.contentRange.from),
      closing: source.slice(shell.contentRange.to, shell.outerRange.to),
    };
    const children = shell.segments.flatMap((child) =>
      expandSegment(documentScope, document, spans, child, partition, [...envelopes, envelope]),
    );
    return children.map((child, index) => ({
      ...child,
      leading:
        index === 0
          ? concatText(
              sourceText(documentScope, segment.contentRange.from, range.from),
              child.leading,
            )
          : child.leading,
      trailing:
        index === children.length - 1
          ? concatText(child.trailing, sourceText(documentScope, range.to, segment.contentRange.to))
          : child.trailing,
      separatorRange: index === children.length - 1 ? segment.separatorRange : child.separatorRange,
    }));
  }
  const base: ReorderChip = {
    index: -1,
    partition,
    text: segment.text,
    contentRange: segment.contentRange,
    separatorRange: segment.separatorRange,
    envelopes,
    leading: sourceText(documentScope, 0, 0),
    trailing: sourceText(documentScope, 0, 0),
  };
  if (!segment.hardBreaks.length) return splitLoras(documentScope, document, spans, base);
  const lines: ReorderChip[] = [];
  let cursor = segment.contentRange.from;
  const endings = [
    ...segment.hardBreaks,
    { from: segment.contentRange.to, to: segment.contentRange.to },
  ];
  for (const ending of endings) {
    const from = cursor,
      to = ending.from;
    if (horizontalRange(source, { from, to }).from !== to) {
      const separatorRange = ending.to > to ? ending : segment.separatorRange;
      lines.push({
        ...base,
        text: source.slice(from, to),
        contentRange: { from, to },
        separatorRange,
      });
    } else if (lines.length && ending.to > to) {
      const previous = requireValue(lines.at(-1), 'derivation');
      lines[lines.length - 1] = {
        ...previous,
        separatorRange: {
          from: previous.separatorRange?.from ?? previous.contentRange.to,
          to: ending.to,
        },
      };
    }
    cursor = ending.to;
  }
  const lastLine = lines.at(-1);
  if (
    lastLine &&
    lastLine.contentRange.to < segment.contentRange.to &&
    horizontalRange(source, {
      from: lastLine.separatorRange?.to ?? lastLine.contentRange.to,
      to: segment.contentRange.to,
    }).from === segment.contentRange.to
  )
    lines[lines.length - 1] = {
      ...lastLine,
      separatorRange: {
        from: lastLine.contentRange.to,
        to: segment.separatorRange?.to ?? segment.contentRange.to,
      },
    };
  return lines.flatMap((line) => splitLoras(documentScope, document, spans, line));
}
/** Prepare one actual canonical value; headers, padding and foreign scope objects cannot contribute chips. */
export function prepareValueReorder(
  scopes: PreparedDocumentPromptScopes,
  scope: PreparedDocumentPromptScope,
  previous?: PreparedValueReorder,
): PreparedValueReorder | null {
  if (scopes.byId[scope.id] !== scope || !scopes.scopes.includes(scope)) return null;
  if (previous?.scope === scope) return previous;
  return deriveValueReorder(scope);
}
/** Prepare every exact scope/index member once in canonical scope order. Inconsistent or duplicate membership rejects the whole batch; all chip/carrier ranges remain decoded scope-local UTF-16. */
export function prepareDocumentValueReorders(
  scopes: PreparedDocumentPromptScopes,
): readonly PreparedValueReorder[] | null {
  if (Object.keys(scopes.byId).length !== scopes.scopes.length) return null;
  const result: PreparedValueReorder[] = [],
    seen = new Set<string>();
  for (const scope of scopes.scopes) {
    if (
      !scope ||
      !Object.hasOwn(scopes.byId, scope.id) ||
      scopes.byId[scope.id] !== scope ||
      seen.has(scope.id)
    )
      return null;
    seen.add(scope.id);
    result.push(deriveValueReorder(scope));
  }
  return Object.freeze(result);
}
/** One derivation consumes an already admitted actual parsed scope without repeating roster membership scans or grammar. */
function deriveValueReorder(scope: PreparedDocumentPromptScope): PreparedValueReorder {
  const document = scope.document;
  const spans = new ReorderDerivationSpans(document);
  const raw: ReorderChip[] = [];
  for (const partition of document.regions.partitions)
    for (const segment of document.segments) {
      const from = Math.max(partition.sourceRange.from, segment.contentRange.from);
      const to = Math.min(partition.sourceRange.to, segment.contentRange.to);
      if (from >= to) continue;
      raw.push(
        ...expandSegment(
          scope,
          document,
          spans,
          {
            ...segment,
            text: document.sourceText.slice(from, to),
            contentRange: { from, to },
            separatorRange: to === segment.contentRange.to ? segment.separatorRange : null,
            hardBreaks: segment.hardBreaks.filter(
              (ending) => ending.from >= from && ending.to <= to,
            ),
          },
          partition.index,
          [],
        ),
      );
    }
  const chips = raw.map((chip, index) => ({ ...chip, index }));
  const first = chips[0],
    last = chips.at(-1);
  const start = first
    ? first.contentRange.from -
      first.leading.text.length -
      first.envelopes.reduce((n, envelope) => n + envelope.opening.length, 0)
    : document.sourceText.length;
  const end = last
    ? last.contentRange.to +
      last.trailing.text.length +
      last.envelopes.reduce((n, envelope) => n + envelope.closing.length, 0)
    : start;
  const slots = chips.slice(0, -1).map((chip, index) => {
    const next = requireValue(chips[index + 1], 'derivation');
    if (chip.partition !== next.partition)
      return sourceText(
        scope,
        chip.contentRange.to +
          chip.trailing.text.length +
          chip.envelopes.reduce((n, envelope) => n + envelope.closing.length, 0),
        next.contentRange.from -
          next.leading.text.length -
          next.envelopes.reduce((n, envelope) => n + envelope.opening.length, 0),
      );
    return chip.separatorRange
      ? sourceText(scope, chip.separatorRange.from, chip.separatorRange.to)
      : sourceText(scope, 0, 0);
  });
  return freezeStructure({
    scope,
    chips,
    initial: {
      order: chips.map((chip) => chip.index),
      partitions: chips.map((chip) => chip.partition),
      slots,
      prefix: sourceText(scope, 0, start),
      suffix: sourceText(scope, end, document.sourceText.length),
    },
  });
}
