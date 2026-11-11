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
/** Route focused discrete viewer keys through the shared attachment command authority. */
import type {
  CanvasCommand,
  CanvasCommandRequest,
  CanvasViewPort,
} from '../domain/CanvasViewPort.js';

/** Bind only discrete keyboard intent; native pointer gestures remain with their host. */
export class CanvasViewportKeyboard {
  #disposed = false;
  /** Retain the existing port identity, capability checks and asynchronous cancellation owner. */
  constructor(
    private readonly element: HTMLElement,
    private readonly port: CanvasViewPort,
    private readonly report: (error: unknown, request: CanvasCommandRequest | null) => void,
    private readonly cancel?: () => void,
  ) {
    element.tabIndex = 0;
    element.setAttribute('role', 'region');
    element.setAttribute('aria-label', 'Output image viewer');
    element.addEventListener('keydown', this.#key);
  }

  /** Remove only this keyboard binding; the attachment owns in-flight commands. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.element.removeEventListener('keydown', this.#key);
  }

  /** Preserve editable descendants and host/browser modifier shortcuts. */
  readonly #key = (event: KeyboardEvent): void => {
    if (
      this.#disposed ||
      event.target !== this.element ||
      event.ctrlKey ||
      event.metaKey ||
      event.altKey
    )
      return;
    if (event.key === 'Escape' && this.cancel) {
      this.cancel();
    } else {
      const command = this.#command(event);
      if (!command || !this.port.capabilities.commands.includes(command.type)) return;
      if (this.port.read().preview) {
        event.preventDefault();
        event.stopPropagation();
        return;
      }
      let request: CanvasCommandRequest | null = null;
      try {
        request = this.port.createRequest(command, this.port.read().revision);
        const admitted = request;
        void this.port.dispatch(admitted).catch((error: unknown) => this.report(error, admitted));
      } catch (error: unknown) {
        this.report(error, request);
      }
    }
    event.preventDefault();
    event.stopPropagation();
  };

  /** Express arrows as viewport fractions while preserving the existing forty-CSS-pixel step. */
  #command(event: KeyboardEvent): CanvasCommand | null {
    if (event.key === '+' || event.key === '=') return { type: 'zoom', factor: 1.25 };
    if (event.key === '-') return { type: 'zoom', factor: 0.8 };
    if (event.shiftKey) return null;
    if (event.key === '0') return { type: 'fit' };
    if (event.key === '1') return { type: 'native' };
    const bounds = this.element.getBoundingClientRect();
    if (!(bounds.width > 0 && bounds.height > 0)) return null;
    const x = Math.min(10, 40 / bounds.width);
    const y = Math.min(10, 40 / bounds.height);
    switch (event.key) {
      case 'ArrowLeft':
        return { type: 'pan', x, y: 0, units: 'viewport' };
      case 'ArrowRight':
        return { type: 'pan', x: -x, y: 0, units: 'viewport' };
      case 'ArrowUp':
        return { type: 'pan', x: 0, y, units: 'viewport' };
      case 'ArrowDown':
        return { type: 'pan', x: 0, y: -y, units: 'viewport' };
      default:
        return null;
    }
  }
}
