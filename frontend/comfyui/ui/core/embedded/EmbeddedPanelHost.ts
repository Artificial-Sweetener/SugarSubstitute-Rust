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
/** Bind one browser document to negotiated host presentation and existing Comfy connectivity. */
import { PanelClientSession } from './PanelRuntime.js';
import type { EmbeddedPresentation } from './EmbeddedPresentation.js';
import { createUxpPanelTransport } from './UxpPanelTransport.js';
import { createNativePanelTransport } from './NativePanelTransport.js';
import type { BrowserPanelTransport } from './BrowserPanelTransport.js';
import { EmbeddedPanelLayout } from './EmbeddedPanelLayout.js';
import type { EmbeddedCanvasConnection } from './EmbeddedCanvasConnection.js';
import type { MemoryBudgetRegistry } from '../memory/MemoryBudgetRegistry.js';
import { MemoryHostLink, type MemoryPreferenceSource } from '../memory/MemoryHostLink.js';

/** Supply the browser lifetime without introducing another connection or preference owner. */
export interface EmbeddedPanelOptions {
  readonly window: Window;
  readonly presentation: EmbeddedPresentation;
  readonly canvas?: EmbeddedCanvasConnection;
  /** The page's cache registry and the stored memory limit it reports to a desktop host. */
  readonly memory?: {
    readonly registry: MemoryBudgetRegistry;
    preferences(): MemoryPreferenceSource;
  };
  connected(): boolean;
  subscribeConnection?(listener: (connected: boolean) => void): () => void;
  report(error: unknown): void;
}

/** Keep messaging active only while the owning SugarCubes document is alive. */
export class EmbeddedPanelHost {
  readonly #layout: EmbeddedPanelLayout;
  #stop: (() => void) | null = null;
  #stopConnection: (() => void) | null = null;
  #memory: MemoryHostLink | null = null;
  #timer: number | null = null;

  /** Receive an identified adapter transport after Comfy and presentation setup have succeeded. */
  constructor(
    private readonly options: EmbeddedPanelOptions,
    private readonly transport: BrowserPanelTransport,
  ) {
    this.#layout = new EmbeddedPanelLayout(options.window);
  }

  /** Listen before announcing readiness so synchronous host responses cannot be lost. */
  setup(): void {
    if (this.#stop) return;
    const client = new PanelClientSession(this.transport.session, {
      send: (message) => this.transport.send(message),
      apply: (presentation) => {
        this.#layout.apply(presentation.resizeCornerSize);
        if (this.transport.preferencePolicy !== 'saved')
          this.options.presentation.set(presentation);
        this.options.canvas?.connect(this.transport, this.options.window, this.options.report);
        return true;
      },
      connected: this.options.connected,
      report: this.options.report,
    });
    this.#stop = this.transport.subscribe((value) => {
      try {
        client.receive(value);
      } catch (error: unknown) {
        this.options.report(error);
      }
    });
    this.#stopConnection =
      this.options.subscribeConnection?.((connected) => {
        if (connected) this.options.canvas?.refresh();
      }) ?? null;
    const ready = (): void => {
      try {
        client.ready();
      } catch (error: unknown) {
        this.options.report(error);
      }
    };
    if (this.transport.memoryBudget && this.options.memory) {
      try {
        this.#memory = new MemoryHostLink(
          this.transport,
          this.options.memory.registry,
          this.options.memory.preferences(),
          this.options.report,
        );
        this.#memory.start();
      } catch (error: unknown) {
        this.options.report(error);
      }
    }
    this.#timer = this.options.window.setInterval(ready, 1000);
    this.options.window.addEventListener('pagehide', this.dispose);
    ready();
  }

  /** Retire subscriptions and restore saved preferences when the embedded document leaves. */
  readonly dispose = (): void => {
    this.#stop?.();
    this.#stop = null;
    this.#stopConnection?.();
    this.#stopConnection = null;
    this.#memory?.dispose();
    this.#memory = null;
    if (this.#timer !== null) this.options.window.clearInterval(this.#timer);
    this.#timer = null;
    this.options.window.removeEventListener('pagehide', this.dispose);
    this.options.presentation.set(null);
    this.options.canvas?.dispose();
    this.#layout.dispose();
  };
}

/** Activate embedded presentation only for an identified application WebView. */
export function startEmbeddedPanel(options: EmbeddedPanelOptions): EmbeddedPanelHost | null {
  const query = new URLSearchParams(options.window.location.search);
  const kind = query.get('sc-panel-transport');
  if (query.getAll('sc-panel-transport').length > 1 || (kind !== null && kind !== 'native'))
    return null;
  const native = createNativePanelTransport(options.window);
  const transport =
    kind === 'native' ? native : (native ?? createUxpPanelTransport(options.window));
  if (!transport) return null;
  const host = new EmbeddedPanelHost(options, transport);
  host.setup();
  return host;
}
