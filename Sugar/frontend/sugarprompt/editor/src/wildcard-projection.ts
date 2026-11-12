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
/** Prepare native wildcard wrappers and canonical CSV glyphs while retaining authored body editing and selection. */
import { requireValue } from '../../core/src/required.js';
import type { EditorState, Range } from '@codemirror/state';
import { Decoration } from '@codemirror/view';
import {
  wildcardBodyRange,
  prepareWildcardColumnAliases,
  isDiagnosticCommitBoundary,
  decodedRangeToSource,
  type WildcardGroup,
} from '../../core/src/index.js';
import type { NativeValueMapping } from './document-values.js';
import { SourceToken } from './projection-widgets.js';
import { WildcardWidget } from './wildcard-widget.js';
import { wildcardCatalog } from './wildcard-catalog.js';
import { WildcardColumnWidget } from './wildcard-column-widget.js';
import { prepareWildcardDiagnostic } from '../../services/src/index.js';
import { WildcardDiagnosticValue } from './wildcard-diagnostic-value.js';
/** Prepared intervals use logical-document UTF-16; the parent projection owns publication and reuse. */
export interface WildcardProjection {
  readonly decorations: readonly Range<Decoration>[];
  readonly tokens: readonly Range<SourceToken>[];
  readonly diagnostics: readonly Range<WildcardDiagnosticValue>[];
}
/** Compile each group's chrome once outside native input and paint, retaining exact source underneath. */
export function prepareWildcardProjection(
  state: EditorState,
  groups: readonly WildcardGroup[],
  mapping: NativeValueMapping,
  source: string,
): WildcardProjection {
  const catalog = state.facet(wildcardCatalog),
    rows = groups.map((group) => {
      const resolution = catalog?.resolve(group.span) ?? {
        status: 'unavailable' as const,
      };
      const diagnostic = prepareWildcardDiagnostic(group.span, resolution);
      const diagnosticRange = diagnostic
        ? decodedRangeToSource(mapping.mapping, diagnostic.range)
        : null;
      if (diagnostic && !diagnosticRange) throw new Error('wildcard-diagnostic-endpoint-unmapped');
      return {
        group,
        resolution,
        diagnostic: diagnostic
          ? Object.freeze({
              ...diagnostic,
              range: requireValue(diagnosticRange, 'wildcard-projection'),
            })
          : null,
        body: wildcardBodyRange(group.span),
        aliases: prepareWildcardColumnAliases(
          group.span,
          resolution.status === 'found' ? resolution.matchedCsvColumn : null,
        ),
      };
    }),
    positions = rows
      .flatMap(({ group, body, aliases }) => [
        group.span.outerRange.from,
        body.from,
        body.to,
        group.span.outerRange.to,
        ...aliases.flatMap((alias) => [alias.range.from, alias.range.to]),
      ])
      .map((decoded) => {
        const logical = mapping.decodedToLogical[decoded];
        if (logical == null) throw new Error('wildcard-projection-endpoint-unmapped');
        return logical;
      });
  const decorations: Range<Decoration>[] = [],
    tokens: Range<SourceToken>[] = [],
    diagnostics: Range<WildcardDiagnosticValue>[] = [];
  let position = 0;
  for (const { group, resolution, aliases, diagnostic } of rows) {
    const token = new SourceToken(),
      from = requireValue(positions[position++], 'wildcard-projection'),
      bodyFrom = requireValue(positions[position++], 'wildcard-projection'),
      bodyTo = requireValue(positions[position++], 'wildcard-projection'),
      to = requireValue(positions[position++], 'wildcard-projection');
    const span = group.span;
    const title = diagnostic
      ? diagnostic.message
      : resolution.status === 'pending'
        ? 'Wildcard catalog pending'
        : resolution.status === 'unavailable'
          ? 'Wildcard catalog unavailable'
          : resolution.status !== 'found' || resolution.matchedCsvColumn === null
            ? 'Wildcard found'
            : `CSV wildcard column: ${resolution.matchedCsvColumn}`;
    const finding = diagnostic
      ? new WildcardDiagnosticValue(
          diagnostic,
          bodyFrom,
          bodyTo,
          isDiagnosticCommitBoundary(
            Array.from(source.slice(span.outerRange.to, span.outerRange.to + 2))[0] ?? '',
          ),
        )
      : null;
    if (finding) diagnostics.push(finding.range(from, to));
    decorations.push(
      Decoration.mark({
        inclusive: true,
        wildcardDiagnostic: finding,
        sourceToken: token,
        class: `sp-wildcard-body sp-wildcard-${resolution.status}`,
        attributes: {
          'data-resolution': resolution.status,
          'data-diagnostic-active': 'false',
          title,
        },
      }).range(bodyFrom, bodyTo),
      Decoration.replace({
        widget: new WildcardWidget(token, 'prefix', group, resolution.status),
      }).range(from, bodyFrom),
      Decoration.replace({
        widget: new WildcardWidget(token, 'suffix', group, resolution.status),
      }).range(bodyTo, to),
    );
    tokens.push(token.range(from, to));
    for (const alias of aliases)
      decorations.push(
        Decoration.replace({
          widget: new WildcardColumnWidget(token, alias.text),
        }).range(
          requireValue(positions[position++], 'wildcard-projection'),
          requireValue(positions[position++], 'wildcard-projection'),
        ),
      );
  }
  return { decorations, tokens, diagnostics };
}
