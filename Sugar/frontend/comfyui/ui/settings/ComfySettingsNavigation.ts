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
/** Read and activate native settings categories without duplicating Comfy routing state. */
export interface SettingsCategory {
  readonly id: string;
  readonly element: HTMLElement;
}

/** Normalize the current host selection annotation at the DOM adapter boundary. */
export function readSettingsCategory(root: HTMLElement): SettingsCategory | null {
  const element = root.querySelector<HTMLElement>(
    '[data-nav-id][class*="surface-selected"], [data-nav-id][aria-current="page"]',
  );
  const id = element?.dataset.navId;
  return element && id ? { id, element } : null;
}

/** Send one native navigation intent and let Comfy mount the requested page. */
export function activateSettingsCategory(root: HTMLElement, id: string): boolean {
  const entry = [...root.querySelectorAll<HTMLElement>('[data-nav-id]')].find(
    (element) => element.dataset.navId === id,
  );
  if (!entry) return false;
  entry.click();
  return true;
}
