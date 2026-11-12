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
/** Generation-only resolution consumes core spans and pass-local injected candidates without editing authoring source. */
import { requireValue } from '../../../core/src/required.js';
import { parsePrompt, unicodeLowerCase, type WildcardSpan } from '../../../core/src/index.js';
import { strip } from '../../../core/src/prompt/text.js';
import {
  captureGenerationInput,
  generationSourceKey,
  validGenerationSeed,
  type CapturedGenerationInput,
} from './generation-input.js';
import { WildcardSelectionContext } from './generation-selection.js';
import { isWildcardUnicode } from './generation-digest.js';
import type {
  WildcardGenerationIssue,
  WildcardReplacementDetail,
  WildcardGenerationResolution,
  WildcardGenerationPass,
  WildcardGenerationOutcome,
  WildcardGenerationPreparation,
} from './generation-contracts.js';
/** One iteration's actual token occurrences share one selected value and provenance owner. */
interface TokenGroup {
  readonly span: WildcardSpan;
  readonly ranges: WildcardSpan['outerRange'][];
  readonly detail: Omit<WildcardReplacementDetail, 'ranges'>;
}
/** One complete generation owner consumes prepared candidates; it retains no external work or editor state. */
class GenerationPass implements WildcardGenerationPass {
  private disposed = false;
  private requests = 0;
  private readonly context: WildcardSelectionContext;
  private readonly input: CapturedGenerationInput;
  /** One immutable revision defines the entire pass; changing candidates requires preparing a new pass. */
  constructor(input: CapturedGenerationInput) {
    this.input = input;
    this.context = new WildcardSelectionContext(input.entropySeed);
  }
  /** Resolve full prompts with the same seed/source coordination across calls, preserving exact source separately. */
  resolve(source: string, seed?: string | null): WildcardGenerationOutcome {
    if (this.disposed) return Object.freeze({ status: 'rejected', reason: 'disposed' });
    if (typeof source !== 'string' || source.length > 1000000)
      return Object.freeze({ status: 'rejected', reason: 'invalid-source' });
    if (seed !== undefined && seed !== null && !validGenerationSeed(seed))
      return Object.freeze({ status: 'rejected', reason: 'invalid-seed' });
    if (++this.requests > 1024)
      return Object.freeze({ status: 'rejected', reason: 'request-limit' });
    const effectiveSeed = seed ?? this.input.seed;
    let current = source;
    const details: WildcardReplacementDetail[] = [],
      issues: WildcardGenerationIssue[] = [];
    /** Freeze a stable complete host submission; replacement arrays are derived from their sole detail owner. */
    const finish = (status: WildcardGenerationResolution['status']): WildcardGenerationResolution =>
      Object.freeze({
        status,
        revision: this.input.revision,
        sourceText: source,
        resolvedText: current,
        replacements: Object.freeze(
          details.map((item) => Object.freeze([item.outerText, item.value] as const)),
        ),
        replacementDetails: Object.freeze(details),
        issues: Object.freeze(issues),
      });
    if (!this.input.enabled) return finish('disabled');
    for (let iteration = 0; iteration < 25; iteration++) {
      const spans = parsePrompt(current, this.input.syntax).wildcardSpans;
      const groups = new Map<string, TokenGroup>(),
        failed = new Set<string>();
      const replacements: {
        from: number;
        to: number;
        value: string;
      }[] = [];
      /** Record typed expected failures using actual iteration-local UTF-16 ranges. */
      const issue = (
        span: WildcardSpan,
        reason: WildcardGenerationIssue['reason'],
        inputIteration = iteration,
      ): void => {
        issues.push(
          Object.freeze({
            reason,
            iteration: inputIteration,
            range: span.outerRange,
            form: span.form,
            identifier: span.identifier,
            tag: span.tag,
            csvColumn: span.csvColumn,
          }),
        );
      };
      let coveredTo = 0;
      for (const span of spans) {
        if (span.outerRange.from < coveredTo) continue;
        const outerText = current.slice(span.outerRange.from, span.outerRange.to);
        if (failed.has(outerText)) continue;
        let group = groups.get(outerText);
        if (!group) {
          const chosen = this.choose(span, outerText, effectiveSeed, iteration);
          if (typeof chosen === 'string') {
            failed.add(outerText);
            issue(span, chosen);
            continue;
          }
          group = { span, ranges: [], detail: chosen };
          groups.set(outerText, group);
        }
        group.ranges.push(span.outerRange);
        replacements.push({ ...span.outerRange, value: group.detail.value });
        coveredTo = span.outerRange.to;
      }
      const iterationDetails = [...groups.values()].map((group) =>
        Object.freeze({ ...group.detail, ranges: Object.freeze(group.ranges) }),
      );
      if (!replacements.length) break;
      let cursor = 0,
        next = '';
      for (const replacement of replacements) {
        if (replacement.from < cursor) throw new Error('wildcard-replacement-overlap-invariant');
        next += current.slice(cursor, replacement.from) + replacement.value;
        cursor = replacement.to;
        if (next.length > 8000000) break;
      }
      next += current.slice(cursor);
      if (next.length > 8000000) {
        for (const { span } of groups.values()) issue(span, 'output-limit');
        break;
      }
      details.push(...iterationDetails);
      if (next === current) {
        for (const { span } of groups.values()) issue(span, 'iteration-limit');
        break;
      }
      current = next;
      if (iteration === 24)
        for (const span of parsePrompt(current, this.input.syntax).wildcardSpans)
          issue(span, 'iteration-limit', 25);
    }
    return finish(issues.length ? 'partial' : 'resolved');
  }
  /** Resolve one authored token using the sole prepared candidates and selection context. */
  private choose(
    span: WildcardSpan,
    outerText: string,
    effectiveSeed: string | null,
    iteration: number,
  ): Omit<WildcardReplacementDetail, 'ranges'> | WildcardGenerationIssue['reason'] {
    const candidate = this.input.sources.get(generationSourceKey(span.form, span.identifier));
    if (!candidate) {
      return 'source-missing';
    }
    if (candidate.status === 'unavailable') {
      return 'source-unavailable';
    }
    const count = candidate.values.length;
    if (!count) {
      return 'empty-source';
    }
    if (effectiveSeed !== null && span.tag !== null && !isWildcardUnicode(span.tag)) {
      return 'invalid-tag';
    }
    const index = this.context.select(
      effectiveSeed,
      span.form,
      span.identifier,
      candidate.sourceId,
      count,
      span.tag,
    );
    if (index === null) {
      return 'context-limit';
    }
    const selected = requireValue(candidate.values[index], 'generation');
    let value: string;
    if (span.form === 'simple') {
      if (typeof selected !== 'string') throw new Error('wildcard-source-form-invariant');
      value = selected;
    } else {
      if (typeof selected === 'string') throw new Error('wildcard-source-form-invariant');
      const cell = selected.get(
        unicodeLowerCase(strip(requireValue(span.csvColumn, 'generation'))),
      );
      if (cell === undefined) {
        return 'column-missing';
      }
      value = cell;
    }
    return {
      outerText,
      value,
      form: span.form,
      identifier: span.identifier,
      sourceId: candidate.sourceId,
      selectedIndex: index,
      lineNumber: index + (span.form === 'simple' ? 1 : 2),
      itemCount: count,
      tag: span.tag,
      csvColumn: span.csvColumn,
      seed: effectiveSeed,
      iteration,
    };
  }
  /** Retire candidate references and coordination state without retaining user-content results. */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.input.sources.clear();
    this.context.dispose();
  }
}
/** Prepare a usable finite generation pass from unknown host data; preference defaults enabled. */
export function prepareWildcardGeneration(value: unknown): WildcardGenerationPreparation {
  const input = captureGenerationInput(value);
  return input
    ? Object.freeze({ status: 'ready', pass: new GenerationPass(input) })
    : Object.freeze({ status: 'rejected', reason: 'invalid-input' });
}
