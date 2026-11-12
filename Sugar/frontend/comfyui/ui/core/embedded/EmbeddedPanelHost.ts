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
import {
  DeclaredHost,
  HOST_CAPABILITIES,
  HOST_PROTOCOLS,
  PanelClientSession,
  isHostDescriptionMessage,
} from './PanelRuntime.js';
import type { HostDescriptionState } from './HostDescriptionState.js';
import type { EmbeddedPresentation } from './EmbeddedPresentation.js';
import type { HostAccent } from './HostAccent.js';
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
  /** Settles on what the host declares; every host-dependent choice below reads it. */
  readonly host: HostDescriptionState;
  readonly presentation: EmbeddedPresentation;
  /** Receives the host's offered default accent under every preference policy. */
  readonly hostAccent?: HostAccent;
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
  #stopHost: (() => void) | null = null;
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
        const host = this.#declared();
        this.#layout.apply(presentation.resizeCornerSize);
        this.options.hostAccent?.set(presentation.accentColor ?? null);
        if (host.declares(HOST_CAPABILITIES.hostStyles))
          this.options.presentation.set(presentation);
        if (host.serves(HOST_PROTOCOLS.canvas))
          this.options.canvas?.connect(this.transport, this.options.window, this.options.report, {
            masks: host.serves(HOST_PROTOCOLS.canvasMasks),
          });
        return true;
      },
      connected: this.options.connected,
      report: this.options.report,
    });
    this.#stop = this.transport.subscribe((value) => {
      try {
        if (this.#describe(value)) return;
        client.receive(value);
      } catch (error: unknown) {
        this.options.report(error);
      }
    });
    let connected = this.options.connected();
    this.#stopConnection =
      this.options.subscribeConnection?.((value) => {
        if (connected === value) return;
        connected = value;
        if (value) this.options.canvas?.refresh();
        else this.options.canvas?.disconnected();
      }) ?? null;
    const ready = (): void => {
      try {
        client.ready();
      } catch (error: unknown) {
        this.options.report(error);
      }
    };
    this.#stopHost = this.options.host.subscribe(() => this.#joinMemoryBudget());
    this.#joinMemoryBudget();
    this.#timer = this.options.window.setInterval(ready, 1000);
    this.options.window.addEventListener('pagehide', this.dispose);
    ready();
  }

  /**
   * Settle this session's host description; report one that is ambiguous or contradicts the first.
   *
   * @returns Whether the value was a host description for this session.
   */
  #describe(value: unknown): boolean {
    if (!isHostDescriptionMessage(value) || value.session !== this.transport.session) return false;
    const declared = DeclaredHost.read(value);
    if (!declared) {
      this.options.report(
        new Error('The host described itself with a repeated protocol or capability.'),
      );
      this.options.host.settle(DeclaredHost.undeclared);
    } else if (!this.options.host.settle(declared))
      this.options.report(new Error('The host changed its description during the panel session.'));
    return true;
  }

  /** A configure without a prior description settles on a host that serves only the panel. */
  #declared(): DeclaredHost {
    const settled = this.options.host.read();
    if (settled) return settled;
    this.options.host.settle(DeclaredHost.undeclared);
    return DeclaredHost.undeclared;
  }

  /** Join the desktop memory budget once the host declares that it serves it. */
  #joinMemoryBudget(): void {
    const host = this.options.host.read();
    if (this.#memory || !this.options.memory || !host?.serves(HOST_PROTOCOLS.memory)) return;
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

  /** Retire subscriptions and restore saved preferences when the embedded document leaves. */
  readonly dispose = (): void => {
    this.#stop?.();
    this.#stop = null;
    this.#stopConnection?.();
    this.#stopConnection = null;
    this.#stopHost?.();
    this.#stopHost = null;
    this.#memory?.dispose();
    this.#memory = null;
    if (this.#timer !== null) this.options.window.clearInterval(this.#timer);
    this.#timer = null;
    this.options.window.removeEventListener('pagehide', this.dispose);
    this.options.presentation.set(null);
    this.options.hostAccent?.set(null);
    this.options.canvas?.dispose();
    this.#layout.dispose();
  };
}

/**
 * Activate embedded presentation only for an identified application WebView.
 *
 * A page with no identified host settles on the standalone declaration at once.
 */
export function startEmbeddedPanel(options: EmbeddedPanelOptions): EmbeddedPanelHost | null {
  const transport = identifiedTransport(options.window);
  if (!transport) {
    options.host.settle(DeclaredHost.standalone);
    return null;
  }
  const host = new EmbeddedPanelHost(options, transport);
  host.setup();
  return host;
}

/** Find the one transport this navigation identifies, refusing an ambiguous transport choice. */
function identifiedTransport(windowRef: Window): BrowserPanelTransport | null {
  const query = new URLSearchParams(windowRef.location.search);
  const kind = query.get('sc-panel-transport');
  if (query.getAll('sc-panel-transport').length > 1 || (kind !== null && kind !== 'native'))
    return null;
  const native = createNativePanelTransport(windowRef);
  return kind === 'native' ? native : (native ?? createUxpPanelTransport(windowRef));
}
