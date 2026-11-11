import { ComfyDesktopSessionRuntime } from './ComfyDesktopSessionRuntime.js';
import { createNativeDesktopSessionTransport } from './NativeDesktopSessionTransport.js';
import { BrowserDesktopAssets } from './BrowserDesktopAssets.js';
import { ComfyDesktopArtifacts } from './ComfyDesktopArtifacts.js';
import { OutputArtifactClient } from '../../output_saving/OutputArtifactClient.js';
import { DesktopSessionClient } from '../application/DesktopSessionClient.js';
import { ComfyAboutInformation } from '../../settings/ComfyAboutInformation.js';
/** Own only desktop session adapters; ordinary browser workflows keep their existing composition. */
export class ComfyDesktopSessionHost {
    window;
    options;
    #runtime = null;
    #client = null;
    constructor(window, options) {
        this.window = window;
        this.options = options;
    }
    /** Register a nonblocking handshake before Comfy's normal initial workflow restoration completes. */
    setup() {
        if (this.#client)
            return;
        const transport = createNativeDesktopSessionTransport(this.window);
        if (!transport)
            return;
        const request = (input, init) => this.window.fetch(input, init);
        const nextId = () => this.window.crypto.randomUUID();
        const runtime = new ComfyDesktopSessionRuntime({
            ...this.options,
            scope: transport.scope,
            webview: transport.webview,
            nextId,
            now: () => Date.now(),
            artifacts: new ComfyDesktopArtifacts(new OutputArtifactClient(request), new BrowserDesktopAssets(transport, request, this.window)),
            versions: async () => {
                const metadata = await new ComfyAboutInformation().load();
                const frontend = Reflect.get(this.window, '__COMFYUI_FRONTEND_VERSION__');
                return {
                    frontend: typeof frontend === 'string' && frontend.length ? frontend : 'Unavailable',
                    sugarcubes: metadata.components.find((component) => component.name === 'SugarCubes')?.version ??
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
        }
        catch (error) {
            this.dispose();
            throw error;
        }
    }
    /** Cancel protocol work before graph and presentation owners retire at page teardown. */
    dispose() {
        this.#client?.dispose();
        this.#client = null;
        this.#runtime?.dispose();
        this.#runtime = null;
    }
}
