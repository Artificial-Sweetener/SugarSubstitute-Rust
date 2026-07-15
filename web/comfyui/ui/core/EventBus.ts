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
/**
 * Own the SugarCubes core UI service layer in `web/comfyui/ui/core/EventBus.js`.
 */

/**
 * Coordinate event bus behavior for the SugarCubes UI.
 */
export class EventBus {
  private readonly target: EventTarget;

  constructor() {
    this.target = new EventTarget();
  }

  on<T>(name: string, handler: (detail: T) => void): () => void {
    if (typeof handler !== 'function') return () => {};
    const listener: EventListener = (event) => {
      if (event instanceof CustomEvent) {
        handler(event.detail as T);
      }
    };
    this.target.addEventListener(name, listener);
    return () => this.target.removeEventListener(name, listener);
  }

  emit<T>(name: string, detail: T): void {
    this.target.dispatchEvent(new CustomEvent(name, { detail }));
  }
}
