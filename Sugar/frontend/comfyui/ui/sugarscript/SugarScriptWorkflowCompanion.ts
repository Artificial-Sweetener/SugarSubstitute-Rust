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
/** Persist optional SugarScript as a non-executable workflow companion. */

import { isRecord } from '../types/common.js';
import type { UnknownRecord } from '../types/common.js';
import type { SugarScriptDiagnostic } from './SugarScriptAuthoringModels.js';

/** Name the root workflow metadata field that retains non-executable SugarScript. */
export const SUGARSCRIPT_COMPANION_KEY = 'sugarcubes_sugarscript';

/** Describe the minimal root graph metadata boundary owned by Comfy. */
export interface WorkflowExtraOwner extends UnknownRecord {
  /** Retain opaque root workflow metadata across SugarScript materialization. */
  extra?: unknown;
}

/** Preserve exact metadata ownership for an atomic rollback. */
export interface WorkflowExtraSnapshot {
  /** Report whether the graph originally owned an explicit extra field. */
  present: boolean;
  /** Retain the original value without interpreting unrelated host metadata. */
  value: unknown;
}

/** Describe source retained beside an independently authoritative workflow. */
export interface ImportedSugarScriptCompanion {
  /** Distinguish parsed stale source, located invalid source, and unavailable validation. */
  state: 'detached' | 'invalid' | 'unverified';
  /** Explain why the source cannot be claimed as synchronized. */
  reason: 'synchronization_unproven' | 'source_diagnostics' | 'compilation_unavailable';
  /** Retain a source semantic hash only after successful compilation. */
  semanticHash?: string;
  /** Preserve located language diagnostics for future authoring presentation. */
  diagnostics: readonly SugarScriptDiagnostic[];
}

/** Own source-companion persistence without participating in execution. */
export class SugarScriptWorkflowCompanion {
  readonly #graph: WorkflowExtraOwner;

  /** Bind the authoritative root graph serialized by Comfy. */
  constructor(graph: WorkflowExtraOwner) {
    this.#graph = graph;
  }

  /** Capture exact prior state for atomic authoring rollback. */
  capture(): WorkflowExtraSnapshot {
    return {
      present: Object.prototype.hasOwnProperty.call(this.#graph, 'extra'),
      value: this.#graph.extra,
    };
  }

  /** Attach source materialized atomically with the authoritative native workflow. */
  attach(source: string, semanticHash: string): void {
    const extra = isRecord(this.#graph.extra) ? this.#graph.extra : {};
    this.#graph.extra = {
      ...extra,
      [SUGARSCRIPT_COMPANION_KEY]: {
        schema: 1,
        state: 'synchronized',
        synchronization: {
          method: 'compiled_native_plan',
          version: 'sugarscript-native-plan-v1',
          source_semantic_hash: semanticHash,
        },
        source,
      },
    };
  }

  /** Retain imported source without claiming it controls or matches the workflow. */
  attachImported(source: string, companion: ImportedSugarScriptCompanion): void {
    const extra = isRecord(this.#graph.extra) ? this.#graph.extra : {};
    const sourceSemantics = companion.semanticHash
      ? {
          version: 'sugarscript-native-plan-v1',
          semantic_hash: companion.semanticHash,
        }
      : null;
    this.#graph.extra = {
      ...extra,
      [SUGARSCRIPT_COMPANION_KEY]: {
        schema: 1,
        state: companion.state,
        reason: companion.reason,
        source,
        ...(sourceSemantics ? { source_semantics: sourceSemantics } : {}),
        diagnostics: companion.diagnostics.map((diagnostic) => ({
          ...diagnostic,
          span: {
            start: { ...diagnostic.span.start },
            end: { ...diagnostic.span.end },
          },
        })),
      },
    };
  }

  /** Restore exact prior root metadata after any batch failure. */
  restore(snapshot: WorkflowExtraSnapshot): void {
    if (snapshot.present) this.#graph.extra = snapshot.value;
    else delete this.#graph.extra;
  }
}
