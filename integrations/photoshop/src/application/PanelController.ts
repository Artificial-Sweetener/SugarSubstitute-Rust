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
/** Coordinate one Photoshop panel lifetime through portable session and native transport ports. */
import { PanelHostSession, type PanelState } from '../../../shared/PanelSession.js';
import type {
  PanelMessage,
  PanelPresentation,
} from '../../../contracts/PanelProtocol.generated.js';

/** Provide the selected native host without leaking its APIs into panel sequencing. */
export interface NativePanelPort {
  newSession(): string;
  now(): number;
  open(session: string): void;
  send(message: PanelMessage): void;
  present(state: PanelState): void;
  schedule(tick: () => void): () => void;
  report(error: unknown): void;
}

/** Keep retry and lifecycle transitions independent of native WebView implementation. */
export class PanelController {
  #session: PanelHostSession | null = null;
  #stop: (() => void) | null = null;
  #terminal = false;

  /** Bind explicit host capabilities and this application's presentation choice. */
  constructor(
    private readonly port: NativePanelPort,
    private presentation: PanelPresentation,
  ) {}

  /** Retain the current host appearance across live updates, hide/show and retry. */
  updatePresentation(presentation: PanelPresentation): void {
    this.presentation = presentation;
    this.#guard(() => this.#session?.updatePresentation(presentation));
  }

  /** Start once when Photoshop creates or first shows the panel. */
  start(): void {
    if (!this.#session) this.retry();
  }

  /** Retain a responsive guest and replace only failed, stalled or uninitialized navigations. */
  retry(): void {
    try {
      if (this.#session?.retryConnection(this.port.now())) {
        this.show();
        return;
      }
    } catch (error: unknown) {
      this.#failed(error);
      return;
    }
    this.dispose();
    this.#terminal = false;
    try {
      this.#session = new PanelHostSession(
        this.port.newSession(),
        this.presentation,
        {
          send: (message) => {
            this.port.send(message);
          },
          present: (state) => {
            this.port.present(state);
            if (state === 'failed') {
              this.#terminal = true;
              this.hide();
            }
          },
        },
        this.port.now(),
      );
      this.#stop = this.port.schedule(() => {
        this.#guard(() => this.#session?.tick(this.port.now()));
      });
      this.port.open(this.#session.session);
    } catch (error: unknown) {
      this.#failed(error);
    }
  }

  /** Dispatch only messages whose source and origin the native boundary has verified. */
  receive(value: unknown): void {
    this.#guard(() => this.#session?.receive(value, this.port.now()));
  }

  /** Report a native load failure with structured context supplied by its boundary. */
  fail(error: unknown): void {
    this.#failed(error);
  }

  /** Stop heartbeat work while retaining a hidden panel's document and workflow. */
  hide(): void {
    this.#stop?.();
    this.#stop = null;
  }

  /** Resume monitoring without creating another WebView or losing the current workflow. */
  show(): void {
    if (this.#terminal) return;
    if (!this.#session) this.start();
    else if (!this.#stop) {
      this.#stop = this.port.schedule(() => {
        this.#guard(() => this.#session?.tick(this.port.now()));
      });
      this.#guard(() => this.#session?.tick(this.port.now()));
    }
  }

  /** Retire callbacks and session identity when the native panel or its navigation is replaced. */
  dispose(): void {
    this.hide();
    this.#session = null;
  }

  /** Preserve unexpected transport errors at the owned panel failure boundary. */
  #guard(action: () => unknown): void {
    try {
      action();
    } catch (error: unknown) {
      this.#failed(error);
    }
  }

  /** Present actionable failure without repeatedly logging failed heartbeat callbacks. */
  #failed(error: unknown): void {
    this.#terminal = true;
    this.hide();
    this.port.report(error);
    this.#session?.fail();
    if (!this.#session) this.port.present('failed');
  }
}
