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
/** Compile cached source semantics and prepared host descriptors outside native input, measurement and paint. */
import { requireValue } from '../../core/src/required.js';
import { prepareSceneBands } from './scene-line-bands.js';
import { RangeSet, type EditorState, type Range } from '@codemirror/state';
import { Decoration } from '@codemirror/view';
import { parenthesisEscapes } from '../../core/src/index.js';
import { featureEnabled } from '../../services/src/index.js';
import { editorFreshness } from './freshness.js';
import { featureProfile } from './features.js';
import { richRendering } from './presentation.js';
import { emptyProjection, type PreparedProjection } from './projection-contracts.js';
import {
  SourceToken,
  EmphasisWidget,
  ParenthesisWidget,
  SourceWidget,
  sourceDecorationToken,
} from './projection-widgets.js';
import { LoraWidget } from './lora-widget.js';
import { loraPresentation, projectedLora } from './lora-presentation.js';
import { loraMedia } from './lora-media-state.js';
import { prepareWildcardProjection } from './wildcard-projection.js';
import { wildcardCatalog } from './wildcard-catalog.js';
import { sceneErrors } from './scene-errors.js';
import { prepareSceneProjection } from './scene-projection.js';
import { documentKind } from './document-kind.js';
import { prepareRegionProjection } from './region-projection.js';
import { regionPalette } from './region-paint.js';
import type { PreparedStructure } from './structure.js';
import { lineEndings } from './exact-source.js';
/** Derive one current decoration/range set; source, native caret and history remain untouched. */
export function prepareProjection(
  state: EditorState,
  structure: PreparedStructure,
): PreparedProjection {
  const emphasisEnabled = featureEnabled(state.facet(featureProfile), 'emphasis'),
    loraEnabled = featureEnabled(state.facet(featureProfile), 'lora_syntax'),
    wildcardEnabled = featureEnabled(state.facet(featureProfile), 'wildcard_syntax'),
    resources = state.field(loraPresentation),
    media = state.field(loraMedia);
  if (!state.facet(richRendering))
    return emptyProjection(
      state.field(editorFreshness).source,
      emphasisEnabled,
      loraEnabled,
      wildcardEnabled,
      resources,
      media,
      state.facet(wildcardCatalog),
      state.field(sceneErrors),
      state.facet(documentKind),
    );
  if (
    structure.nativeValues.document !== state.doc ||
    structure.nativeValues.endings !== state.field(lineEndings) ||
    structure.sourceIdentity !== state.field(editorFreshness).source ||
    structure.kind !== state.facet(documentKind)
  )
    throw new Error('projection-source-mismatch');
  const scene =
    structure.kind === 'prompt' ? prepareSceneProjection(state, structure.structure.scenes) : null;
  const sceneBands =
    structure.kind === 'prompt'
      ? prepareSceneBands(structure.structure.scenes).map(({ from, to }) => {
          const mapping = structure.nativeValues.entries[0]?.semanticMapping;
          const start = mapping?.decodedToLogical[from],
            end = mapping?.decodedToLogical[to];
          if (start == null || end == null) throw new Error('scene-band-endpoint-unmapped');
          return Object.freeze({ from: start, to: end });
        })
      : [];
  const region = prepareRegionProjection(state, structure);
  const ranges: Range<Decoration>[] = [],
    tokenRanges: Range<SourceToken>[] = [];
  let hasLoras = false;
  const wildcardDiagnostics: ReturnType<typeof prepareWildcardProjection>['diagnostics'][number][] =
    [];
  for (const entry of structure.nativeValues.entries) {
    const scope = entry.scope,
      mapping = entry.semanticMapping;
    if (!scope || !mapping) continue;
    const source = scope.mapping.text;
    /** Canonical scalar, CRLF and CSV escape gaps remain invalid decoration endpoints. */
    const position = (decoded: number): number => {
      const logical = mapping.decodedToLogical[decoded];
      if (logical == null) throw new Error('projection-endpoint-unmapped');
      return logical;
    };
    /** Regions keep their existing literal owner; Scene suppression partitions this same compiled syntax after all ordinary roles are prepared. */
    const ownsStructure = (range: { readonly from: number; readonly to: number }): boolean =>
      region.ownsDecodedRange(scope.id, range);
    const spans = emphasisEnabled
        ? scope.document.emphasisSpans.filter((span) => !ownsStructure(span.outerRange))
        : [],
      loras = loraEnabled
        ? scope.document.loraSpans.filter((span) => !ownsStructure(span.outerRange))
        : [],
      wildcards = wildcardEnabled
        ? scope.document.wildcardSpans.filter((span) => !ownsStructure(span.outerRange))
        : [];
    let loraIndex = 0,
      wildcardIndex = 0;
    const escapes = parenthesisEscapes(source).filter((range) => {
      if (ownsStructure(range)) return false;
      while (
        loras[loraIndex] &&
        requireValue(loras[loraIndex], 'projection-preparation').outerRange.to <= range.from
      )
        ++loraIndex;
      const lora = loras[loraIndex];
      while (
        wildcards[wildcardIndex] &&
        requireValue(wildcards[wildcardIndex], 'projection-preparation').outerRange.to <= range.from
      )
        ++wildcardIndex;
      const wildcard = wildcards[wildcardIndex];
      return (
        (!lora || range.from < lora.outerRange.from || range.to > lora.outerRange.to) &&
        (!wildcard || range.from < wildcard.outerRange.from || range.to > wildcard.outerRange.to)
      );
    });
    const positions = [
      ...spans.flatMap((span) => [
        span.outerRange.from,
        span.contentRange.from,
        span.contentRange.to,
        span.outerRange.to,
      ]),
      ...escapes.flatMap((range) => [range.from, range.to]),
      ...loras.flatMap((span) => [span.outerRange.from, span.outerRange.to]),
    ].map(position);
    const tokens = spans.map(() => new SourceToken());
    ranges.push(
      ...spans.flatMap((span, index) => [
        Decoration.replace({
          widget: new EmphasisWidget(
            requireValue(tokens[index], 'projection-preparation'),
            'prefix',
          ),
        }).range(
          requireValue(positions[index * 4], 'projection-preparation'),
          requireValue(positions[index * 4 + 1], 'projection-preparation'),
        ),
        Decoration.replace({
          widget: new EmphasisWidget(
            requireValue(tokens[index], 'projection-preparation'),
            'suffix',
            source.slice(span.weightRange.from, span.weightRange.to),
          ),
        }).range(
          requireValue(positions[index * 4 + 2], 'projection-preparation'),
          requireValue(positions[index * 4 + 3], 'projection-preparation'),
        ),
      ]),
    );
    tokenRanges.push(
      ...tokens.map((token, index) =>
        token.range(
          requireValue(positions[index * 4], 'projection-preparation'),
          requireValue(positions[index * 4 + 3], 'projection-preparation'),
        ),
      ),
    );
    for (const [index, escape] of escapes.entries()) {
      const offset = spans.length * 4 + index * 2,
        from = requireValue(positions[offset], 'projection-preparation'),
        to = requireValue(positions[offset + 1], 'projection-preparation'),
        token = new SourceToken();
      const parenthesis = source[escape.from + 1];
      if (parenthesis !== '(' && parenthesis !== ')')
        throw new Error('Invalid prepared parenthesis escape');
      ranges.push(
        Decoration.replace({
          widget: new ParenthesisWidget(token, parenthesis),
        }).range(from, to),
      );
      tokenRanges.push(token.range(from, to));
    }
    for (const [index, span] of loras.entries()) {
      const offset = spans.length * 4 + escapes.length * 2 + index * 2,
        from = requireValue(positions[offset], 'projection-preparation'),
        to = requireValue(positions[offset + 1], 'projection-preparation'),
        token = new SourceToken(),
        name = source.slice(span.nameRange.from, span.nameRange.to),
        resource = projectedLora(state, name);
      ranges.push(
        Decoration.replace({
          widget: new LoraWidget(
            token,
            resource,
            source.slice(span.firstWeightRange.from, span.firstWeightRange.to),
            'none',
            resource.resolution === 'found' && resource.thumbnail.status === 'ready'
              ? media.get(resource.thumbnail)
              : undefined,
          ),
        }).range(from, to),
      );
      tokenRanges.push(token.range(from, to));
    }
    const wildcard = prepareWildcardProjection(
      state,
      wildcardEnabled
        ? scope.wildcardGroups.filter((group) => !ownsStructure(group.span.outerRange))
        : [],
      mapping,
      source,
    );
    ranges.push(...wildcard.decorations);
    tokenRanges.push(...wildcard.tokens);
    wildcardDiagnostics.push(...wildcard.diagnostics);
    hasLoras ||= loras.length > 0;
  }
  /** Prepared heading intervals classify ordinary native paint once; readonly output can later reuse that exact compiled syntax. */
  const sceneRanges = scene?.tokens ?? [];
  const ownsScene = (from: number, to: number): boolean => {
    let low = 0,
      high = sceneRanges.length;
    while (low < high) {
      const middle = (low + high) >>> 1;
      if (requireValue(sceneRanges[middle], 'projection-preparation').to <= from) low = middle + 1;
      else high = middle;
    }
    return !!sceneRanges[low] && requireValue(sceneRanges[low], 'projection-preparation').from < to;
  };
  const hiddenTokens = new Set(
    tokenRanges.filter(({ from, to }) => ownsScene(from, to)).map(({ value }) => value),
  );
  /** Entire prepared roles retain their original outer-span classification, including wrappers on opposite sides of a heading. */
  const hidden = ({ from, to, value }: Range<Decoration>): boolean => {
    const token = sourceDecorationToken(value);
    return token ? hiddenTokens.has(token) : ownsScene(from, to);
  };
  const sceneSyntax = Decoration.set(ranges.filter(hidden), true);
  const nativeRanges = ranges.filter((range) => !hidden(range));
  const nativeTokens = tokenRanges.filter(({ value }) => !hiddenTokens.has(value));
  nativeRanges.push(...(scene?.decorations ?? []));
  nativeTokens.push(...(scene?.tokens ?? []));
  nativeRanges.push(...region.decorations);
  nativeTokens.push(...region.tokens);
  const all = Decoration.set(nativeRanges, true);
  const atomic = Decoration.set(
    nativeRanges.filter(({ value }) => value.spec.widget instanceof SourceWidget),
    true,
  );
  return {
    documentKind: state.facet(documentKind),
    sourceIdentity: state.field(editorFreshness).source,
    emphasisEnabled,
    loraEnabled,
    wildcardEnabled,
    loraPresentation: resources,
    hasLoras,
    sceneSyntax,
    sceneBands: Object.freeze(sceneBands),
    loraMedia: media,
    wildcardCatalog: state.facet(wildcardCatalog),
    sceneErrors: state.field(sceneErrors),
    wildcardDiagnostics: RangeSet.of(
      wildcardDiagnostics.filter(({ from, to }) => !ownsScene(from, to)),
      true,
    ),
    mutedWildcard: null,
    all,
    regionHovers: region.hovers,
    regionPalette: state.facet(regionPalette),
    allAtomic: atomic,
    tokens: RangeSet.of(nativeTokens, true),
    expanded: [],
    decorations: all,
    atomic,
    revealed: null,
    retainRevealOpening: false,
    accented: [],
    weightDraft: null,
  };
}
