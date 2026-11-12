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
/** Prepared projection data binds source ranges, permissions and host inputs to one native rendering publication. */
import { RangeSet } from '@codemirror/state';
import { Decoration, type DecorationSet } from '@codemirror/view';
import type { SourceRange } from '../../core/src/offsets.js';
import type { SourceToken } from './projection-widgets.js';
import type { LoraPresentation } from './lora-presentation.js';
import type { LoraMediaSnapshot } from './lora-media-state.js';
import type { WildcardCatalog } from '../../services/src/index.js';
import type { WildcardDiagnosticValue } from './wildcard-diagnostic-value.js';
import type { SceneErrors } from './scene-errors.js';
import type { PromptDocumentKind } from '../../core/src/index.js';
import type { ExactWeightDraft } from './exact-weight-draft.js';
import type { WeightSubject } from './weight-target.js';
import type { RegionTarget } from './region-index.js';

/** One actual canonical region target owns its source-clipped native hover decoration. */
export interface RegionHoverProjection {
  readonly target: RegionTarget;
  readonly decorations: DecorationSet;
}

/** Derived ranges use logical-document UTF-16; identities bind them to current exact source and host descriptors. */
export interface PreparedProjection {
  readonly documentKind: PromptDocumentKind;
  readonly sourceIdentity: symbol | null;
  readonly emphasisEnabled: boolean;
  readonly loraEnabled: boolean;
  readonly wildcardEnabled: boolean;
  /** Prepared source contains rendered LoRA spans, so resource/media publications can affect its decorations. */
  readonly hasLoras: boolean;
  /** Ordinary rich paint hidden by original headings can become visible when Core moves those headings inline. */
  readonly sceneSyntax: DecorationSet;
  /** Prepared odd Scene operands use logical native UTF-16, reusing the existing canonical value mapping. */
  readonly sceneBands: readonly SourceRange[];
  readonly loraPresentation: LoraPresentation;
  readonly loraMedia: LoraMediaSnapshot;
  readonly wildcardCatalog: WildcardCatalog | null;
  readonly sceneErrors: SceneErrors;
  readonly wildcardDiagnostics: RangeSet<WildcardDiagnosticValue>;
  /** At most one caret-active finding supplies bounded visibility retirement; source changes invalidate its prepared bounds. */
  readonly mutedWildcard: WildcardDiagnosticValue | null;
  readonly all: DecorationSet;
  /** Scope-local ordinal hover lookup preserves canonical target identity across multiple values. */
  readonly regionHovers: Readonly<Record<string, readonly RegionHoverProjection[]>>;
  /** The regional palette base the rails were prepared with; null when no rails were prepared. */
  readonly regionPalette: string | null;
  /** Prepared concealed-widget intervals exclude styling marks and editable body text. */
  readonly allAtomic: DecorationSet;
  readonly tokens: RangeSet<SourceToken>;
  readonly expanded: readonly SourceToken[];
  readonly decorations: DecorationSet;
  /** Visible concealed intervals follow source mapping and reveal without traversing on pointer/key input. */
  readonly atomic: DecorationSet;
  /** One explicit source reveal follows native selection/history in logical-document UTF-16. */
  readonly revealed: SourceRange | null;
  /** Only native structural source authoring retains its reveal at the opening endpoint. */
  readonly retainRevealOpening: boolean;
  /** At most two prior kind-qualified feedback subjects permit bounded retirement without accenting nested foreign roles. */
  readonly accented: readonly WeightSubject[];
  /** Derived prior native draft identity supports bounded retirement without retaining editable text or DOM. */
  readonly weightDraft: ExactWeightDraft | null;
}

/** Raw, disabled or not-yet-prepared rendering retains explicit input identities without discovering source semantics. */
export function emptyProjection(
  sourceIdentity: symbol | null,
  emphasisEnabled: boolean,
  loraEnabled: boolean,
  wildcardEnabled: boolean,
  loraPresentation: LoraPresentation,
  loraMedia: LoraMediaSnapshot,
  wildcardCatalog: WildcardCatalog | null,
  sceneErrors: SceneErrors,
  documentKind: PromptDocumentKind,
): PreparedProjection {
  return {
    documentKind,
    sourceIdentity,
    emphasisEnabled,
    loraEnabled,
    wildcardEnabled,
    hasLoras: false,
    sceneSyntax: Decoration.none,
    sceneBands: Object.freeze([]),
    loraPresentation,
    loraMedia,
    wildcardCatalog,
    sceneErrors,
    wildcardDiagnostics: RangeSet.empty,
    mutedWildcard: null,
    all: Decoration.none,
    regionHovers: Object.freeze({}),
    regionPalette: null,
    allAtomic: Decoration.none,
    tokens: RangeSet.empty,
    expanded: [],
    decorations: Decoration.none,
    atomic: Decoration.none,
    revealed: null,
    retainRevealOpening: false,
    accented: [],
    weightDraft: null,
  };
}
