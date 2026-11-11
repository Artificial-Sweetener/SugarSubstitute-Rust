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
/** Native draft projection metadata contains no editable name or competing source/caret state. */
import { requireValue } from '../../core/src/required.js';
import { StateEffect, StateField } from '@codemirror/state';
import { Decoration, EditorView } from '@codemirror/view';
import { editorFreshness, type EditorFreshness } from './freshness.js';
import { richRendering } from './presentation.js';
import { semanticStructures } from './structure.js';
/** One source-current row's measured unscaled title width frames its finite native draft. */
export interface RegionDraft {
  readonly freshness: EditorFreshness;
  readonly scopeId: string;
  readonly ordinal: number;
  readonly width: number;
}
/** Publish source-free draft geometry; source edits and context replacement retire it immediately. */
export const setRegionDraft = StateEffect.define<RegionDraft | null>();
/** Prompt rows and scope-local structured marks reserve only the measured target title width. */
export const regionDraft = StateField.define<RegionDraft | null>({
  /** A baseline contains no transient native name session. */
  create: () => null,
  /** Source/context, permissions and raw rendering retire source-free geometry before publication. */
  update(current, transaction) {
    if (
      current?.freshness !== transaction.state.field(editorFreshness) ||
      !transaction.state.facet(richRendering) ||
      transaction.state.readOnly
    )
      current = null;
    for (const effect of transaction.effects)
      if (
        effect.is(setRegionDraft) &&
        (!effect.value ||
          (effect.value.freshness === transaction.state.field(editorFreshness) &&
            transaction.state.facet(richRendering) &&
            !transaction.state.readOnly))
      )
        current = effect.value;
    return current;
  },
  provide: (field) =>
    EditorView.outerDecorations.compute([field, semanticStructures], (state) => {
      const draft = state.field(field),
        target =
          draft &&
          state
            .field(semanticStructures)
            .native?.regions.find(
              (member) => member.scopeId === draft.scopeId && member.ordinal === draft.ordinal,
            );
      if (!target || !draft) return Decoration.none;
      const attributes = {
        class: 'sp-region-editing',
        style: '--sp-region-title-width:' + String(draft.width) + 'px',
      };
      const native = requireValue(state.field(semanticStructures).native, 'region-draft');
      return Decoration.set([
        native.kind === 'prompt'
          ? Decoration.line({ attributes }).range(state.doc.lineAt(target.from).from)
          : Decoration.mark({ attributes, inclusive: true }).range(target.from, target.to),
      ]);
    }),
});
