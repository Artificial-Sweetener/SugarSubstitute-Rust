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
/** Own transport-independent panel negotiation and bounded connection monitoring. */
import type { PanelMessage, PanelPresentation } from '../contracts/PanelProtocol.generated.js';
import { isPanelMessage } from './PanelValidation.generated.js';
import { freezePanelPresentation, samePanelPresentation } from './PanelPresentation.js';

/** Describe the visible connection lifetime independently of an application's UI. */
export type PanelState = 'loading' | 'connected' | 'disconnected' | 'failed';

/** Supply native transport, presentation policy, and diagnostics explicitly. */
export interface PanelHostPort {
  send(message: PanelMessage): void;
  present(state: PanelState): void;
}

/** Keep one native panel session alive across backend disconnects without reloading its workflow. */
export class PanelHostSession {
  #configured = false;
  #applied = false;
  #lastReply: number;
  #lastPing = -Infinity;
  #state: PanelState = 'loading';

  /** Begin a bounded handshake after the native adapter chooses a new session identity. */
  constructor(
    readonly session: string,
    private presentation: PanelPresentation,
    private readonly port: PanelHostPort,
    now: number,
  ) {
    this.presentation = freezePanelPresentation(presentation);
    this.#lastReply = now;
    port.present('loading');
  }

  /** Apply live host appearance without replacing the workflow or its connection lifetime. */
  updatePresentation(presentation: PanelPresentation): void {
    if (this.#state === 'failed' || samePanelPresentation(this.presentation, presentation)) return;
    this.presentation = freezePanelPresentation(presentation);
    if (this.#configured) this.#send({ type: 'configure', presentation: this.presentation });
  }

  /** Accept only validated messages addressed to this navigation's session. */
  receive(value: unknown, now: number): boolean {
    if (!isPanelMessage(value) || value.session !== this.session || this.#state === 'failed')
      return false;
    if (value.type === 'ready') {
      this.#configured = true;
      this.#send({ type: 'configure', presentation: this.presentation });
    } else if (value.type === 'applied' && this.#configured) {
      this.#applied = true;
      this.#lastReply = now;
      this.#ping(now);
    } else if (value.type === 'pong' && this.#applied) {
      this.#lastReply = now;
      this.#present(value.connected ? 'connected' : 'disconnected');
    } else if (value.type === 'rejected' && this.#configured) this.fail();
    else return false;
    return true;
  }

  /** Bound startup and detect stalled pages while allowing the same backend session to recover. */
  tick(now: number): void {
    if (this.#state === 'failed') return;
    if (!this.#applied && now - this.#lastReply >= 30000) {
      this.fail();
      return;
    }
    if (this.#applied) {
      if (now - this.#lastReply >= 15000) this.#present('disconnected');
      if (now - this.#lastPing >= 5000) this.#ping(now);
    }
  }

  /** Expose a terminal load failure until the panel explicitly retries with a fresh session. */
  fail(): void {
    this.#present('failed');
  }

  /** Send only host-side protocol messages with authoritative session metadata. */
  #send(message: { type: 'configure'; presentation: PanelPresentation } | { type: 'ping' }): void {
    this.port.send({ protocol: 'sugarcubes.panel', version: 3, session: this.session, ...message });
  }

  /** Update the heartbeat deadline before dispatch to support synchronous in-memory hosts. */
  #ping(now: number): void {
    this.#lastPing = now;
    this.#send({ type: 'ping' });
  }

  /** Publish each state transition once. */
  #present(state: PanelState): void {
    if (state === this.#state) return;
    this.#state = state;
    this.port.present(state);
  }
}

/** Supply the portable frontend client with presentation and connectivity capabilities. */
export interface PanelClientPort {
  send(message: PanelMessage): void;
  apply(presentation: PanelPresentation): boolean;
  connected(): boolean;
  report(error: unknown): void;
}

/** Negotiate presentation without granting native document or filesystem capabilities. */
export class PanelClientSession {
  #applied = false;

  /** Bind one application-neutral client to an externally verified transport session. */
  constructor(
    readonly session: string,
    private readonly port: PanelClientPort,
  ) {}

  /** Announce readiness until the native panel confirms presentation configuration. */
  ready(): void {
    if (!this.#applied) this.#send({ type: 'ready' });
  }

  /** Apply validated configuration or answer a heartbeat after presentation is accepted. */
  receive(value: unknown): boolean {
    if (!isPanelMessage(value) || value.session !== this.session) return false;
    if (value.type === 'configure') {
      try {
        if (!this.port.apply(value.presentation)) {
          this.#send({ type: 'rejected', reason: 'unsupported-presentation' });
          return true;
        }
        this.#applied = true;
        this.#send({ type: 'applied' });
      } catch (error: unknown) {
        this.port.report(error);
        this.#send({ type: 'rejected', reason: 'presentation-failed' });
      }
    } else if (value.type === 'ping' && this.#applied)
      this.#send({ type: 'pong', connected: this.port.connected() });
    else return false;
    return true;
  }

  /** Attach authoritative client metadata to schema-checked response kinds. */
  #send(
    message:
      | { type: 'ready' | 'applied' }
      | { type: 'pong'; connected: boolean }
      | { type: 'rejected'; reason: 'unsupported-presentation' | 'presentation-failed' },
  ): void {
    this.port.send({ protocol: 'sugarcubes.panel', version: 3, session: this.session, ...message });
  }
}
