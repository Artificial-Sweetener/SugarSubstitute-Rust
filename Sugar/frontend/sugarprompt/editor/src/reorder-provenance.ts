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
/** Genuine canonical reorder plans own finite authored provenance; they grant no gesture or edit issuance authority. */
import { requireValue } from '../../core/src/required.js';
import { nativeReorderRecipe } from './reorder-native-recipe.js';
import type { EditorState } from '@codemirror/state';
import {
  serializeValueReorder,
  type PreparedValueReorder,
  type ReorderOutput,
  type ReorderState,
  type SourceRange,
} from '../../core/src/index.js';
import {
  currentNativeDocumentValue,
  type NativeDocumentValueContext,
  type NativeDocumentValueTarget,
} from './document-values.js';
import { authorship } from './authorship.js';
import {
  generatedShells,
  literalIntents,
  type Authorship,
  type GeneratedShell,
} from './authorship-ranges.js';
/** Only a genuinely prepared plan key can select its internally derived canonical transform. */
export interface PreparedNativeReorderPlan {
  readonly prepared: PreparedValueReorder;
}
/** Original canonical target/context and authored index remain source-bound without another editable owner. */
interface Plan {
  readonly context: NativeDocumentValueContext;
  readonly target: NativeDocumentValueTarget;
  readonly authored: Authorship | undefined;
}
const plans = new WeakMap<PreparedNativeReorderPlan, Plan>();
/** A private receipt identity confers only one exact reorder output's authored adaptation. */
export interface DocumentValueReorderProvenance {
  readonly identity: symbol;
}
/** Validated source observations feed the post-map adapter and confer no publication capability. */
export interface ReorderAuthorshipSource {
  readonly target: NativeDocumentValueTarget;
  readonly prepared: PreparedValueReorder;
  readonly output: ReorderOutput;
  readonly authored: Authorship;
  readonly generated: readonly GeneratedShell[];
  readonly literals: readonly SourceRange[];
}
/** The exact immutable preparation state bounds a receipt before the genuine issuer constructs its transaction. */
interface Receipt extends ReorderAuthorshipSource {
  readonly state: EditorState;
}
const receipts = new WeakMap<DocumentValueReorderProvenance, Receipt>();
/** Derive Core's plan from a genuinely issued, current canonical target; callers cannot register assembled plans. */
export function prepareNativeReorderPlan(
  state: EditorState,
  context: NativeDocumentValueContext,
  target: NativeDocumentValueTarget,
): PreparedNativeReorderPlan | null {
  if (currentNativeDocumentValue(state, context, target).status !== 'ready') return null;
  const prepared = nativeReorderRecipe(context.values, target.entry)?.prepared;
  if (!prepared?.chips.length) return null;
  const plan = Object.freeze({ prepared });
  plans.set(plan, {
    context: Object.freeze({ ...context }),
    target,
    authored: state.field(authorship, false),
  });
  return plan;
}
/** Serialize only a retained genuine plan against its current target and original authored index. */
export function prepareReorderProvenance(
  state: EditorState,
  plan: PreparedNativeReorderPlan,
  order: ReorderState,
): {
  readonly output: ReorderOutput;
  readonly provenance: DocumentValueReorderProvenance;
} | null {
  const saved = plans.get(plan);
  if (
    !saved ||
    currentNativeDocumentValue(state, saved.context, saved.target).status !== 'ready' ||
    state.field(authorship, false) !== saved.authored ||
    !saved.authored
  )
    return null;
  const output = serializeValueReorder(plan.prepared, order);
  if (!output) return null;
  const provenance = Object.freeze({ identity: Symbol() });
  receipts.set(
    provenance,
    Object.freeze({
      state,
      target: saved.target,
      prepared: plan.prepared,
      output,
      authored: saved.authored,
      generated: generatedShells(saved.authored),
      literals: literalIntents(saved.authored),
    }),
  );
  return Object.freeze({ output, provenance });
}
/** The issuer supplies its actual decoded UTF-16 replacement rather than a derived prefix observation. */
export interface ReorderAuthorshipReplacement {
  readonly range: SourceRange;
  readonly text: string;
}
/** Validate the actual preparation state, target, full replacement and authored contents before any post-map adaptation. */
export function reorderProvenanceSource(
  state: EditorState,
  target: NativeDocumentValueTarget,
  provenance: DocumentValueReorderProvenance,
  replacement: ReorderAuthorshipReplacement,
): ReorderAuthorshipSource | null {
  const receipt = receipts.get(provenance);
  if (
    !receipt ||
    receipt.state !== state ||
    receipt.target !== target ||
    state.field(authorship) !== receipt.authored ||
    receipt.prepared.scope !== target.scope ||
    replacement.range.from !== 0 ||
    replacement.range.to !== target.scope.document.sourceText.length ||
    replacement.text !== receipt.output.text
  ) {
    if (receipt) receipts.delete(provenance);
    return null;
  }
  const generatedNow = generatedShells(receipt.authored),
    literalsNow = literalIntents(receipt.authored);
  if (
    generatedNow.length !== receipt.generated.length ||
    literalsNow.length !== receipt.literals.length ||
    generatedNow.some((s, i) => {
      const old = requireValue(receipt.generated[i], 'reorder-provenance');
      return (
        s.from !== old.from ||
        s.to !== old.to ||
        s.contentFrom !== old.contentFrom ||
        s.contentTo !== old.contentTo ||
        s.nestingDepth !== old.nestingDepth
      );
    }) ||
    literalsNow.some(
      (r, i) =>
        r.from !== requireValue(receipt.literals[i], 'reorder-provenance').from ||
        r.to !== requireValue(receipt.literals[i], 'reorder-provenance').to,
    )
  ) {
    receipts.delete(provenance);
    return null;
  }
  return receipt;
}
