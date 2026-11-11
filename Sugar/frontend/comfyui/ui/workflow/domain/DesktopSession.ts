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
/** Define explicit desktop-session delivery and semantic snapshot ports without browser authority. */
import type {
  SessionBinding,
  SessionCheckpoint,
  SessionFence,
  SessionMessage,
  SessionScope,
  SessionFailure,
} from '../../../../../integrations/contracts/SessionProtocol.generated.js';

export interface DesktopSessionTransport {
  readonly scope: SessionScope;
  readonly origin: string;
  readonly webview: string;
  send(message: SessionMessage): Promise<void>;
  subscribe(receive: (message: unknown) => void): () => void;
  /** Resolve only a current retained final through the host's revocable same-origin media capability. */
  assetUrl?(outputId: string, binding: SessionBinding): string | null;
}

/** Keep host-assigned durable revision separate from page-local observation fences. */
export interface DesktopSessionRuntime {
  fence(): SessionFence;
  readiness(): { readonly committed: boolean; readonly workflowPersistence: boolean };
  subscribe(listener: () => void): () => void;
  /** Await admitted Ready delivery before probing retained metadata with freshly proposed runtime bindings. */
  bindObservationDelivery?(publish: () => Promise<void>): () => void;
  capture(
    checkpointId: string,
    revision: number,
    cancellation: DesktopSessionCancellation,
  ): Promise<SessionCheckpoint>;
  materialize(
    checkpoint: SessionCheckpoint,
    cancellation: DesktopSessionCancellation,
  ): Promise<readonly SessionBinding[]>;
  acknowledged(checkpointId: string, revision: number): void;
}

/** Own one protocol request's cancellation without exposing browser, scheduler or native handles. */
export class DesktopSessionCancellation {
  readonly #listeners = new Set<() => void>();
  #cancelled = false;

  /** Read cancellation before every effect and after delayed external completion. */
  cancelled(): boolean {
    return this.#cancelled;
  }

  /** Observe retirement; already-cancelled requests notify synchronously. */
  subscribe(listener: () => void): () => void {
    if (this.#cancelled) listener();
    else this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }

  /** Cancel exactly once and release every observer after notification. */
  cancel(): void {
    if (this.#cancelled) return;
    this.#cancelled = true;
    for (const listener of this.#listeners) listener();
    this.#listeners.clear();
  }
}

/** Preserve actionable portable refusal and its underlying cause across frontend adapter boundaries. */
export class DesktopSessionRefusal extends Error {
  /** Receive a G0 machine-readable code; localized product feedback remains with the host owner. */
  constructor(
    readonly code: SessionFailure['code'],
    message: string,
    options?: ErrorOptions,
  ) {
    super(message, options);
    this.name = 'DesktopSessionRefusal';
  }
}
