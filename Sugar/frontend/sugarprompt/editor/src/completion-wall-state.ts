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
/** Prepared wall geometry and viewport media follow the sole native suggestion session. */
import { StateEffect, StateField } from '@codemirror/state';
import { completionSession } from './completion-session.js';
import type { PreparedCompletionResult } from './prompt-completion-model.js';
import type { LoraWallLayout } from './lora-wall-layout.js';
import type { LoraMediaReference } from './lora-media-contracts.js';

/** Background preparation binds immutable geometry and the visible/overscan interval to candidate identity. */
export interface CompletionWall {
  readonly result: PreparedCompletionResult;
  readonly layout: LoraWallLayout;
  readonly from: number;
  readonly to: number;
  readonly references: readonly LoraMediaReference[];
}

/** Publish measured, prepared viewport input; stale tooltip lifetimes cannot revive retired candidates. */
export const installCompletionWall = StateEffect.define<CompletionWall>();

/** Geometry and media membership are derived state, with no editable document or selected-index authority. */
export const completionWall = StateField.define<CompletionWall | null>({
  /** New native sessions have no measured wall. */
  create() {
    return null;
  },
  /** Selection-only transactions reuse actual geometry; candidate replacement retires all derived membership. */
  update(wall, transaction) {
    const session = transaction.state.field(completionSession);
    if (wall && wall.result !== session?.result) {
      const previous = transaction.startState.field(completionSession);
      wall =
        session && previous?.presentation === session.presentation
          ? { ...wall, result: session.result }
          : null;
    }
    for (const effect of transaction.effects)
      if (effect.is(installCompletionWall) && effect.value.result === session?.result)
        wall = effect.value;
    return wall;
  },
});
