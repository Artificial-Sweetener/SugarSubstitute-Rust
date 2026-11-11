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
/** Present authoritative Cube target models in native picker result titles. */

import { resolveCubeModelTitle } from '../cube/CubeModelTitlePresentation.js';
import { presentCubeModelTitle } from '../surface/CubeModelPillDomRenderer.js';
import type { ComfyCubeNodeDefinition } from './ComfyCubeNodeDefProjector.js';

/** Mark the native title leaf already claimed for one projected Cube definition. */
export const PRESENTED_MODEL_TYPE_ATTRIBUTE = 'data-sugarcubes-picker-model-title';

/** Replace one uniquely located visible title leaf with safe model-pill DOM. */
export class ComfyCubePickerModelTitlePresenter {
  /** Present one already-identified Cube result and preserve its full accessible name. */
  present(result: HTMLElement, definition: ComfyCubeNodeDefinition): void {
    const presentation = resolveCubeModelTitle({
      targetModel: definition.sugarcubes_target_model,
      title: definition.display_name,
    });
    if (!presentation.usesModelPill) return;
    const markedTitle = result.querySelector<HTMLElement>(`[${PRESENTED_MODEL_TYPE_ATTRIBUTE}]`);
    const title = markedTitle ?? findUniqueTitleLeaf(result, definition.display_name);
    if (!title) return;
    title.setAttribute(PRESENTED_MODEL_TYPE_ATTRIBUTE, definition.name);
    presentCubeModelTitle(title, presentation);
  }
}

/** Locate exactly one literal title leaf without depending on Comfy's internal CSS classes. */
function findUniqueTitleLeaf(result: HTMLElement, displayName: string): HTMLElement | null {
  const matches = [...result.querySelectorAll<HTMLElement>('span, div')].filter(
    (candidate) => candidate.children.length === 0 && candidate.textContent?.trim() === displayName,
  );
  return matches.length === 1 ? (matches[0] ?? null) : null;
}
