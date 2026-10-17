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
/** Theme only native dropdown portals linked to identified standard settings controls. */
import { applyOwnedTheme } from '../surface/appearance/ThemeService.js';

/** Preserve native popup identity and isolate bespoke plugin menus from Fluent control styling. */
export class SettingsPopupTheme {
  readonly #owned = new Set<HTMLElement>();
  readonly #linked = new Set<HTMLElement>();

  /** Associate one portal with its native aria-controls anchor instead of claiming all listboxes. */
  reconcile(
    overlay: HTMLElement,
    roots: Iterable<HTMLElement>,
    fluent: boolean,
    dark: boolean,
  ): void {
    const list = overlay.querySelector<HTMLElement>('[role="listbox"]');
    const linked =
      !!list?.id &&
      [...roots].some((root) =>
        [...root.querySelectorAll('[data-sc-setting-kind="standard"] [aria-controls]')].some(
          (control) => control.getAttribute('aria-controls') === list.id,
        ),
      );
    if (!overlay.isConnected || !linked) {
      this.#linked.delete(overlay);
      this.#release(overlay);
      return;
    }
    this.#linked.add(overlay);
    if (!fluent) {
      this.#release(overlay);
      return;
    }
    this.#owned.add(overlay);
    overlay.setAttribute('data-sc-settings-popup', '');
    applyOwnedTheme(overlay, dark);
  }

  /** Reconcile retained portals when settings mode, palette or dialog lifetime changes. */
  refresh(roots: Iterable<HTMLElement>, fluent: boolean, dark: boolean): void {
    for (const overlay of this.#linked) this.reconcile(overlay, roots, fluent, dark);
  }

  /** Restore host inheritance when the settings presentation retires. */
  dispose(): void {
    for (const overlay of this.#owned) this.#release(overlay);
    this.#linked.clear();
  }

  /** Remove only tokens installed by this positively identified popup owner. */
  #release(overlay: HTMLElement): void {
    if (!this.#owned.delete(overlay)) return;
    overlay.removeAttribute('data-sc-settings-popup');
    applyOwnedTheme(overlay, null);
  }
}
