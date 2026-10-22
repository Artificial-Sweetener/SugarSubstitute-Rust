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
/** Prepared scene ownership and materialized prompt text share the parsed exact source. */
import { requireValue } from '../required.js';
import type { SourceRange } from '../offsets.js';
import type { SceneBlock, SceneDocument } from '../prompt/contracts.js';
import { strip } from '../prompt/text.js';
/** One immutable context covers exact-source UTF-16 bounds; the final context includes its ending caret. */
export interface ScenePositionContext {
  readonly sourceRange: SourceRange;
  readonly scene: SceneBlock | null;
  readonly effectivePromptText: string;
}
/** Source-bound derived text is prepared once; caret lookup reuses an actual context in logarithmic time. */
export interface PreparedSceneContexts {
  readonly document: SceneDocument;
  readonly universal: ScenePositionContext;
  readonly scenes: readonly ScenePositionContext[];
  /** Distinct effective texts in source order, including universal content only when it owns a source position. */
  readonly effectivePromptTexts: readonly string[];
  /** Look up a safe integer exact-source UTF-16 position, clamping it to the source as the reference does. */
  at(position: number): ScenePositionContext;
}
/** Trim Python-compatible boundary whitespace and join universal/local source with one blank LF line. */
export function materializeScenePrompt(universalText: string, sceneText: string): string {
  const universal = strip(universalText),
    local = strip(sceneText);
  return joinTrimmedPrompt(universal, local);
}
/** Prepared callers trim shared universal source once; joining retains the same materialization owner. */
function joinTrimmedPrompt(universal: string, local: string): string {
  return universal && local ? `${universal}\n\n${local}` : universal || local;
}
/** Prepare every parsed scene, preserving authored title identity and internal newline spellings. */
export function prepareSceneContexts(document: SceneDocument): PreparedSceneContexts {
  const universal: ScenePositionContext = Object.freeze({
    sourceRange: document.universalRange,
    scene: null,
    effectivePromptText: document.universalText,
  });
  const trimmedUniversal = strip(document.universalText);
  const includesUniversal = document.universalRange.to > 0 || document.scenes.length === 0;
  const effective = includesUniversal ? [universal.effectivePromptText] : [];
  const localTexts = new Set<string>();
  const scenes = Object.freeze(
    document.scenes.map((scene): ScenePositionContext => {
      const local = strip(scene.text);
      const context = Object.freeze({
        sourceRange: Object.freeze({
          from: scene.marker.lineRange.from,
          to: scene.contentRange.to,
        }),
        scene,
        effectivePromptText: joinTrimmedPrompt(trimmedUniversal, local),
      });
      // The common universal prefix is identical; compare local source rather than hash each materialized copy.
      if (
        !localTexts.has(local) &&
        !(includesUniversal && !local && trimmedUniversal === document.universalText)
      )
        effective.push(context.effectivePromptText);
      localTexts.add(local);
      return context;
    }),
  );
  const effectivePromptTexts = Object.freeze(effective);
  return Object.freeze({
    document,
    universal,
    scenes,
    effectivePromptTexts,
    /** Heading indentation belongs to the following scene; only the final scene owns EOF. */
    at(position: number): ScenePositionContext {
      if (!Number.isSafeInteger(position)) throw new RangeError('Invalid scene source position');
      const clamped = Math.max(0, Math.min(position, document.sourceText.length));
      let from = 0,
        to = scenes.length;
      while (from < to) {
        const middle = (from + to) >>> 1;
        if (requireValue(scenes[middle], 'context').sourceRange.from <= clamped) from = middle + 1;
        else to = middle;
      }
      return scenes[from - 1] ?? universal;
    },
  });
}
