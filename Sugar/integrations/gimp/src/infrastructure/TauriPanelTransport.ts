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
/** Carry only existing public editor envelopes between the local shell and its ComfyUI guest. */
import type { PanelMessage } from '../../../contracts/PanelProtocol.generated.js';
import type { CanvasMessage } from '../../../contracts/CanvasProtocol.generated.js';
import type { CanvasMaskMessage } from '../../../contracts/CanvasMasksProtocol.generated.js';
import { MAX_CANVAS_ENCODED_CHARACTERS } from '../../../shared/CanvasImageEncoding.js';
import { invokeTauri } from './TauriInvocation.js';

/** Keep the viewer's public message port distinct from its private GIMP SDK port. */
export type PublicPanelMessage = PanelMessage | CanvasMessage | CanvasMaskMessage;

/** Describe Tauri's typed channel boundary without importing its declarations into other runtimes. */
interface TauriChannel {
  onmessage: (value: unknown) => void;
}

/** Register a callback on this WebView only; Rust validates the role, origin and navigation identity. */
function channel(host: object, receive: (value: unknown) => void): TauriChannel {
  const bridge: unknown = Reflect.get(host, '__TAURI__');
  if (!bridge || typeof bridge !== 'object') throw new Error('Tauri panel bridge unavailable.');
  const core: unknown = Reflect.get(bridge, 'core');
  const constructor: unknown =
    core && typeof core === 'object' ? Reflect.get(core, 'Channel') : null;
  if (typeof constructor !== 'function') throw new Error('Tauri panel channels unavailable.');
  const result: unknown = Reflect.construct(constructor, []);
  if (!result || typeof result !== 'object') throw new Error('Invalid Tauri panel channel.');
  const owned = result as TauriChannel;
  owned.onmessage = receive;
  return owned;
}

/** Bound asynchronous admission while each message retains the existing protocol's authoritative owner. */
export class TauriPanelTransport {
  readonly #listeners = new Set<(value: unknown) => void>();
  readonly #ready: Promise<unknown>;
  #closed = false;
  #pending = 0;
  #characters = 0;

  /** Subscribe before navigation so the guest's first readiness envelope cannot be lost. */
  constructor(
    private readonly host: object,
    readonly session: string,
    role: 'host' | 'guest',
    private readonly report: (error: unknown) => void,
  ) {
    this.#ready = invokeTauri(host, role === 'host' ? 'panel_open' : 'panel_attach', {
      session,
      channel: channel(host, (value) => {
        if (!this.#closed) for (const listener of this.#listeners) listener(value);
      }),
    });
    void this.#ready.catch(report);
  }

  /** Wait until the native relay owns this renderer's callback. */
  async ready(): Promise<void> {
    await this.#ready;
    if (this.#closed) throw new Error('The panel transport has closed.');
  }

  /** Admit bounded public messages and preserve send order across bridge initialization. */
  send(message: PublicPanelMessage): void {
    if (this.#closed) return;
    const size = JSON.stringify(message).length;
    const maximum = MAX_CANVAS_ENCODED_CHARACTERS + 4 * 1024 * 1024;
    if (
      message.session !== this.session ||
      this.#pending >= 16 ||
      this.#characters + size > maximum
    )
      throw new Error('The public panel transport is stale or busy.');
    this.#pending++;
    this.#characters += size;
    void this.#ready
      .then(async () => {
        if (!this.#closed)
          await invokeTauri(this.host, 'panel_send', { session: this.session, message });
      })
      .catch(this.report)
      .finally(() => {
        this.#pending--;
        this.#characters -= size;
      });
  }

  /** Retain only explicitly subscribed application consumers. */
  subscribe(receive: (value: unknown) => void): () => void {
    this.#listeners.add(receive);
    return () => {
      this.#listeners.delete(receive);
    };
  }

  /** Detach renderer-owned consumers and suppress late native deliveries. */
  dispose(): void {
    if (this.#closed) return;
    this.#closed = true;
    this.#listeners.clear();
  }
}
