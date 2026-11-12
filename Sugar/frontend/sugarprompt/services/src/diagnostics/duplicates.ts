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
/** Immutable duplicate preparation selects effective ownership and adapts canonical value coordinates. */
import type { SourceRange } from '../../../core/src/offsets.js';
import type {
  PreparedDocumentPromptScope,
  PreparedDocumentPromptScopes,
} from '../../../core/src/document/prompt-scopes.js';
import { decodedRangeToSource } from '../../../core/src/document/value-mapping.js';
import { ConditioningTopology } from '../../../core/src/prompt/conditioning.js';
import {
  prepareDuplicateSegments,
  type DuplicateFinding,
  type PreparedDuplicateSegments,
} from '../../../core/src/diagnostics/duplicate-segments.js';
import type { ConditioningContext } from '../regions/context.js';

/** One actual decoded value retains Core's complete ownership and source-bound findings. */
export interface PreparedDuplicateGroup {
  readonly scope: PreparedDocumentPromptScope;
  readonly prepared: PreparedDuplicateSegments;
}

/** Display ranges use exact storage UTF-16; the retained Core finding uses decoded scope-local UTF-16. */
export interface DuplicateDiagnostic {
  readonly kind: 'duplicate-segment';
  readonly severity: 'error';
  readonly group: PreparedDuplicateGroup;
  readonly finding: DuplicateFinding;
  readonly range: SourceRange;
  readonly firstRange: SourceRange;
  readonly message: string;
}

/** A pure snapshot grants no editor freshness, permissions, ignore lifetime or native edit authority. */
export interface PreparedDuplicateDiagnostics {
  readonly scopes: PreparedDocumentPromptScopes;
  readonly context: ConditioningContext;
  readonly groups: readonly PreparedDuplicateGroup[];
  readonly findings: readonly DuplicateDiagnostic[];
}

/** Map only actual prepared decoded boundaries; a missing canonical boundary is an observable invariant. */
function sourceRange(scope: PreparedDocumentPromptScope, range: SourceRange): SourceRange {
  const mapped = decodedRangeToSource(scope.mapping, range);
  if (!mapped) throw new Error('unmapped-duplicate-diagnostic');
  return mapped;
}

/** Delegate inheritance to the actual scene/topology owner; candidate storage never contributes grammar. */
export function prepareDuplicateDiagnostics(
  scopes: PreparedDocumentPromptScopes,
  context: ConditioningContext,
  previous?: PreparedDuplicateDiagnostics,
): PreparedDuplicateDiagnostics {
  if (
    previous?.scopes === scopes &&
    previous.context.key === context.key &&
    previous.context.mode === context.mode
  )
    return previous;
  const groups: PreparedDuplicateGroup[] = [];
  const findings: DuplicateDiagnostic[] = [];
  for (const scope of scopes.scopes) {
    const document = scope.document;
    const scenes = scope.sceneContexts;
    const prepared = prepareDuplicateSegments(
      document,
      scopes.values.kind !== 'prompt'
        ? { kind: 'value' }
        : scenes?.scenes.length && !document.regions.separators.length
          ? { kind: 'scenes', contexts: scenes }
          : {
              kind: 'conditioning',
              topology: new ConditioningTopology(context.mode, document.regions),
            },
    );
    const group = Object.freeze({ scope, prepared });
    groups.push(group);
    for (const finding of prepared.findings) {
      findings.push(
        Object.freeze({
          kind: 'duplicate-segment' as const,
          severity: 'error' as const,
          group,
          finding,
          range: sourceRange(scope, finding.duplicate.range),
          firstRange: sourceRange(scope, finding.first.range),
          message: `Duplicate prompt segment: ${finding.duplicate.key}`,
        }),
      );
    }
  }
  return Object.freeze({
    scopes,
    context: Object.freeze({ key: context.key, mode: context.mode }),
    groups: Object.freeze(groups),
    findings: Object.freeze(findings),
  });
}
