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
/** Compose existing panel negotiation and canvas dispatch over the replaceable companion transport. */
import { PanelHostSession, type PanelState } from '../../../shared/PanelSession.js';
import { CanvasMaskServer } from '../../../shared/CanvasMaskServer.js';
import type { ConnectionState } from '../application/CompanionConnection.js';
import type { GimpNativePort } from '../application/NativeGimpSession.js';
import { GimpCanvasBridge } from './GimpCanvasBridge.js';
import { GimpCanvasMasks } from './GimpCanvasMasks.js';
import { TauriPanelTransport } from './TauriPanelTransport.js';
import { connectCompanion } from './TauriConnection.js';
import { invokeTauri } from './TauriInvocation.js';

/** Retain one workflow and its shared protocol session across backend disconnects. */
export class GimpPanelConnection {
  #transport: TauriPanelTransport | null = null;
  #panel: PanelHostSession | null = null;
  #canvas: GimpCanvasBridge | null = null;
  #masks: CanvasMaskServer | null = null;
  #maskHost: GimpCanvasMasks | null = null;
  #stop: (() => void) | null = null;
  #timer: number | null = null;
  #state: PanelState = 'loading';
  #address = '';
  #publish: (state: ConnectionState) => void = () => {};
  #closed = false;

  /** Borrow the GIMP connection; viewer disposal remains with the owning local renderer. */
  constructor(
    private readonly window: Window,
    private readonly native: GimpNativePort,
    private readonly report: (error: unknown) => void,
  ) {}

  /** Probe readiness without replacing a disconnected backend's loaded workflow. */
  async connect(publish: (state: ConnectionState) => void): Promise<void> {
    if (this.#closed) return;
    this.#publish = publish;
    if (!this.#transport || this.#state === 'failed') this.#start();
    const transport = this.#transport;
    if (!transport) throw new Error('The companion panel did not start.');
    await transport.ready();
    const result = await connectCompanion(this.window, transport.session);
    if (this.#transport !== transport) return;
    this.#address = result.address;
    if (result.phase === 'disconnected') {
      if (this.#state === 'loading') this.#retire();
      else
        void invokeTauri(this.window, 'panel_visible', {
          session: transport.session,
          visible: false,
        }).catch(this.report);
      publish(result);
    } else this.#present(this.#state);
  }

  /** Retire a failed navigation and prepare all shared dispatchers before opening another one. */
  #start(): void {
    this.#retire();
    const transport = new TauriPanelTransport(
      this.window,
      this.window.crypto.randomUUID(),
      'host',
      this.report,
    );
    this.#transport = transport;
    const send = transport.send.bind(transport);
    this.#canvas = new GimpCanvasBridge(transport.session, this.native, send, this.report);
    const masks = new GimpCanvasMasks(this.native, this.report);
    this.#maskHost = masks;
    this.#masks = new CanvasMaskServer(transport.session, {
      changes: null,
      execute: (command) => masks.execute(command),
      send,
      report: this.report,
    });
    this.#panel = new PanelHostSession(
      transport.session,
      {
        pluginMode: true,
        visualStyle: 'gimp',
        photoProPalette: 'dark-gray',
        accentColor: null,
        resizeCornerSize: 0,
      },
      {
        send,
        present: (state) => {
          this.#present(state);
        },
      },
      this.window.performance.now(),
    );
    this.#stop = transport.subscribe((value) => {
      try {
        if (this.#panel?.receive(value, this.window.performance.now())) return;
        if (this.#canvas?.receive(value)) return;
        this.#masks?.receive(value);
      } catch (error: unknown) {
        this.report(error);
        this.#panel?.fail();
      }
    });
    this.#timer = this.window.setInterval(() => {
      try {
        this.#panel?.tick(this.window.performance.now());
      } catch (error: unknown) {
        this.report(error);
        this.#panel?.fail();
      }
    }, 1000);
  }

  /** Reveal the workflow only after shared negotiation confirms application readiness. */
  #present(state: PanelState): void {
    this.#state = state;
    this.#publish({
      phase:
        state === 'connected' ? 'connected' : state === 'loading' ? 'connecting' : 'disconnected',
      address: this.#address,
      message:
        state === 'connected'
          ? 'Connected to ComfyUI.'
          : state === 'loading'
            ? 'Waiting for SugarCubes…'
            : state === 'failed'
              ? 'SugarCubes did not initialize. Check ComfyUI, then retry.'
              : 'ComfyUI is not connected. Start the server, then retry.',
    });
    if (this.#transport && state !== 'loading')
      void invokeTauri(this.window, 'panel_visible', {
        session: this.#transport.session,
        visible: state === 'connected',
      }).catch(this.report);
  }

  /** Release public requests and heartbeat work before the shared native connection closes. */
  dispose(): void {
    this.#closed = true;
    this.#retire();
  }

  /** Remove session-owned collaborators while retaining the owning GIMP viewer lifetime. */
  #retire(): void {
    this.#stop?.();
    this.#stop = null;
    if (this.#timer !== null) this.window.clearInterval(this.#timer);
    this.#timer = null;
    this.#canvas?.dispose();
    this.#masks?.dispose();
    this.#maskHost?.dispose();
    this.#transport?.dispose();
    this.#canvas = null;
    this.#masks = null;
    this.#maskHost = null;
    this.#transport = null;
    this.#panel = null;
    this.#state = 'loading';
  }
}
