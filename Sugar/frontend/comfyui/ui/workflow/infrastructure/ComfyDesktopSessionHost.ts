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
/** Install the production desktop checkpoint handshake alongside Comfy's existing workflow workspace. */
import type { ComfyDesktopSessionOptions } from './ComfyDesktopSessionRuntime.js';
import { ComfyDesktopSessionRuntime } from './ComfyDesktopSessionRuntime.js';
import { createNativeDesktopSessionTransport } from './NativeDesktopSessionTransport.js';
import { BrowserDesktopAssets } from './BrowserDesktopAssets.js';
import { ComfyDesktopArtifacts } from './ComfyDesktopArtifacts.js';
import { OutputArtifactClient } from '../../output_saving/OutputArtifactClient.js';
import { DesktopSessionClient } from '../application/DesktopSessionClient.js';
import { ComfyAboutInformation } from '../../settings/ComfyAboutInformation.js';

/** Own only desktop session adapters; ordinary browser workflows keep their existing composition. */
export class ComfyDesktopSessionHost {
  #runtime: ComfyDesktopSessionRuntime | null = null;
  #client: DesktopSessionClient | null = null;

  constructor(
    private readonly window: Window,
    private readonly options: Omit<
      ComfyDesktopSessionOptions,
      'scope' | 'webview' | 'nextId' | 'now' | 'versions' | 'artifacts'
    >,
  ) {}

  /** Register a nonblocking handshake before Comfy's normal initial workflow restoration completes. */
  setup(): void {
    if (this.#client) return;
    const transport = createNativeDesktopSessionTransport(this.window);
    if (!transport) return;
    const request: typeof globalThis.fetch = (input, init) => this.window.fetch(input, init);
    const nextId = (): string => this.window.crypto.randomUUID();
    const runtime = new ComfyDesktopSessionRuntime({
      ...this.options,
      scope: transport.scope,
      webview: transport.webview,
      nextId,
      now: () => Date.now(),
      artifacts: new ComfyDesktopArtifacts(
        new OutputArtifactClient(request),
        new BrowserDesktopAssets(transport, request, this.window),
      ),
      versions: async () => {
        const metadata = await new ComfyAboutInformation().load();
        const frontend: unknown = Reflect.get(this.window, '__COMFYUI_FRONTEND_VERSION__');
        return {
          frontend: typeof frontend === 'string' && frontend.length ? frontend : 'Unavailable',
          sugarcubes:
            metadata.components.find((component) => component.name === 'SugarCubes')?.version ??
            'Unavailable',
        };
      },
    });
    this.#runtime = runtime;
    try {
      runtime.install();
      this.#client = new DesktopSessionClient({
        transport,
        runtime,
        nextId,
        digest: async (text) => {
          const bytes = new TextEncoder().encode(text);
          const hash = await this.window.crypto.subtle.digest('SHA-256', bytes);
          return [...new Uint8Array(hash)]
            .map((value) => value.toString(16).padStart(2, '0'))
            .join('');
        },
        report: this.options.report,
      });
      this.#client.install();
    } catch (error: unknown) {
      this.dispose();
      throw error;
    }
  }

  /** Cancel protocol work before graph and presentation owners retire at page teardown. */
  dispose(): void {
    this.#client?.dispose();
    this.#client = null;
    this.#runtime?.dispose();
    this.#runtime = null;
  }
}
