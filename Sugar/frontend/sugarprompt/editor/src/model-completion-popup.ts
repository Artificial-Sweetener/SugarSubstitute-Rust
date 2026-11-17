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
/** A host presents model completion candidates in its own picker surface; the native session keeps selection and acceptance. */
import { Facet } from '@codemirror/state';
import type { LoraCatalogItem } from '../../services/src/index.js';
import { captureRecord } from '../../services/src/saved-segments/capture.js';

/** Arrow keys that move the current candidate across the host's gallery. */
export type ModelCompletionArrow = 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight';

/** One mounted host gallery shows the session's ranked models and never owns the caret or the selection. */
export interface ModelCompletionPopup {
  /** The host's whole popup surface; the editor only anchors and bounds it. */
  readonly element: HTMLElement;
  /** Show these models in ranked order with one current candidate. */
  update(models: readonly LoraCatalogItem[], selected: number): void;
  /** The candidate one arrow key away from `selected`, following the gallery's own visual rows. */
  neighbor(selected: number, key: ModelCompletionArrow): number;
  /** Release the surface and everything it mounted. */
  dispose(): void;
}

/** Events a mounted gallery reports back to the native completion session. */
export interface ModelCompletionPopupEvents {
  /** The person chose the candidate at this ranked position with the pointer. */
  activate(index: number): void;
}

/** The host capability that mounts a picker gallery for one completion lifetime. */
export interface ModelCompletionPopupHost {
  mount(events: ModelCompletionPopupEvents): ModelCompletionPopup;
}

/** One host presents model candidates; without it model names are not completed. */
export const modelCompletionPopupHost = Facet.define<
  ModelCompletionPopupHost | null,
  ModelCompletionPopupHost | null
>({ combine: (values) => values.at(-1) ?? null });

/** Validate the injected capability's shape without mounting anything. */
export function validateModelCompletionPopupHost(
  value: ModelCompletionPopupHost | null | undefined,
): void {
  if (value == null) return;
  if (typeof captureRecord(value, ['mount']).mount !== 'function')
    throw new TypeError('invalid-model-completion-popup-host');
}

/** Capture the admitted callable and its receiver; later property replacement is never consulted. */
export function captureModelCompletionPopupHost(
  value: ModelCompletionPopupHost | null,
): ModelCompletionPopupHost | null {
  if (value === null) return null;
  const mount = captureRecord(value, ['mount']).mount;
  if (typeof mount !== 'function') throw new TypeError('invalid-model-completion-popup-host');
  return Object.freeze({
    mount: (events: ModelCompletionPopupEvents) =>
      Reflect.apply(mount, value, [events]) as ModelCompletionPopup,
  });
}
