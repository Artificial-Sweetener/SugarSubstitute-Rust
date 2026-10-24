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
/** Own UXP lifecycle, trusted WebView transport, native status surfaces, and timer cleanup. */
import { entrypoints } from 'uxp';
import { core } from 'photoshop';
import { readPhotoshopPresentation, watchPhotoshopTheme } from './PhotoshopTheme.js';
import manifest from '../../manifest.json' with { type: 'json' };
import { PanelController } from '../application/PanelController.js';
import { panelFeedback } from '../presentation/PanelFeedback.js';
import type { PanelMessage } from '../../../contracts/PanelProtocol.generated.js';
import { PhotoshopCanvasBridge } from './PhotoshopCanvasBridge.js';
import { PhotoshopMaskSession } from './PhotoshopMaskSession.js';

/** Supply the documented UXP globals omitted by the pinned declarations, scoped to this adapter. */
declare const window: {
  addEventListener(name: string, receive: (event: unknown) => void): void;
  removeEventListener(name: string, receive: (event: unknown) => void): void;
};
declare const console: {
  info(message: string, context: unknown): void;
  error(message: string, context: unknown): void;
};
declare function setInterval(tick: () => void, milliseconds: number): number;
declare function clearInterval(timer: number): void;
declare const URL: {
  new (input: string): { readonly origin: string };
};

const ORIGIN = manifest.requiredPermissions.webview.domains[0] ?? '';
if (!ORIGIN) throw new Error('The SugarCubes manifest must permit a ComfyUI origin.');
const PLUGIN_ID = manifest.id;

/** Normalize UXP's full content URL so Comfy workflow routing preserves trusted messages. */
function isComfyOrigin(value: unknown): boolean {
  if (typeof value !== 'string') return false;
  try {
    return new URL(value).origin === ORIGIN;
  } catch {
    return false;
  }
}

/** Describe the native panel lifecycle missing from the pinned upstream entrypoint declaration. */
interface PanelLifecycle {
  create(): void;
  show(): void;
  hide(): void;
  destroy(): void;
}

/** Keep UXP's dynamic element declarations inside the native boundary. */
interface NativeSurface {
  textContent: string | null;
  setAttribute(name: string, value: string): void;
  addEventListener(name: string, listener: (event: unknown) => void): void;
  removeEventListener(name: string, listener: (event: unknown) => void): void;
}

/** Fail early when Photoshop has not mounted an expected static package surface. */
function element(id: string): NativeSurface {
  const value: unknown = document.getElementById(id);
  if (
    !value ||
    typeof value !== 'object' ||
    !('setAttribute' in value) ||
    typeof value.setAttribute !== 'function' ||
    !('addEventListener' in value) ||
    typeof value.addEventListener !== 'function' ||
    !('removeEventListener' in value) ||
    typeof value.removeEventListener !== 'function' ||
    !('textContent' in value)
  )
    throw new Error(`Missing SugarCubes panel element: ${id}`);
  // Static package elements expose textContent as a writable native accessor, not a JavaScript field.
  return value as NativeSurface;
}

/** Register one native dockable panel and keep every borrowed host resource scoped to it. */
export function registerPhotoshopPanel(): void {
  let dispose: (() => void) | null = null;
  let controller: PanelController | null = null;
  const create = (): void => {
    if (controller) return;
    const webview = element('comfyui');
    const status = element('connection');
    const title = element('connection-title');
    const message = element('connection-message');
    const retry = element('retry');
    if (!('postMessage' in webview) || typeof webview.postMessage !== 'function')
      throw new Error('The Photoshop host does not expose the SugarCubes WebView transport.');
    // UXP declares WebView separately from the common native element interface.
    const nativeWebView = webview as unknown as HTMLWebViewElement;
    const canvas = new PhotoshopCanvasBridge(
      (value) => {
        nativeWebView.postMessage(value, ORIGIN, undefined);
      },
      (error) => {
        console.error('SugarCubes Photoshop canvas failed', { error });
      },
      new PhotoshopMaskSession((error) => {
        console.error('SugarCubes Photoshop mask association failed', { error });
      }),
    );
    let expectedUrl = '';
    const active = new PanelController(
      {
        newSession: () => crypto.randomUUID(),
        now: () => Date.now(),
        open: (session) => {
          canvas.start(session);
          expectedUrl = `${ORIGIN}/?sc-panel-host=${encodeURIComponent(PLUGIN_ID)}&sc-panel-session=${encodeURIComponent(session)}`;
          nativeWebView.src = expectedUrl;
        },
        send: (value: PanelMessage) => {
          nativeWebView.postMessage(value, ORIGIN, undefined);
        },
        present: (state) => {
          const feedback = panelFeedback(state);
          title.textContent = feedback.title;
          message.textContent = feedback.message;
          retry.setAttribute('style', `display:${feedback.retry ? 'block' : 'none'}`);
          status.setAttribute('style', `display:${feedback.visible ? 'block' : 'none'}`);
          console.info('SugarCubes panel state', { state });
        },
        schedule: (tick) => {
          const timer = setInterval(tick, 1000);
          return () => {
            clearInterval(timer);
          };
        },
        report: (error) => {
          console.error('SugarCubes Photoshop panel failed', { error });
        },
      },
      readPhotoshopPresentation(),
    );
    const receive = (event: unknown): void => {
      if (
        !event ||
        typeof event !== 'object' ||
        !('origin' in event) ||
        !('source' in event) ||
        !('data' in event)
      )
        return;
      if (isComfyOrigin(event.origin) && event.source === webview) {
        active.receive(event.data);
        canvas.receive(event.data);
      }
    };
    const failed = (): void => {
      canvas.dispose();
      active.fail(new Error('ComfyUI WebView navigation failed.'));
    };
    const navigating = (event: unknown): void => {
      if (
        !event ||
        typeof event !== 'object' ||
        !('url' in event) ||
        typeof event.url !== 'string' ||
        (event.url !== 'about:blank' && event.url.split('#')[0] !== expectedUrl)
      ) {
        active.fail(new Error('The panel left its configured ComfyUI navigation.'));
        canvas.dispose();
      }
    };
    const reload = (): void => {
      active.retry();
    };
    window.addEventListener('message', receive);
    webview.addEventListener('loaderror', failed);
    webview.addEventListener('loadstart', navigating);
    retry.addEventListener('click', reload);
    controller = active;
    const stopTheme = watchPhotoshopTheme(
      (value) => {
        active.updatePresentation(value);
      },
      (error) => {
        console.error('SugarCubes Photoshop theme failed', { error });
      },
    );
    void core
      .suppressResizeGripper({ type: 'panel', target: 'sugarcubes', value: true })
      .catch((error: unknown) => {
        console.error('SugarCubes could not hide the Photoshop panel resize gripper', { error });
      });
    dispose = () => {
      stopTheme();
      canvas.dispose();
      active.dispose();
      window.removeEventListener('message', receive);
      webview.removeEventListener('loaderror', failed);
      webview.removeEventListener('loadstart', navigating);
      retry.removeEventListener('click', reload);
      nativeWebView.src = 'about:blank';
      controller = null;
    };
    active.start();
  };
  const destroy = (): void => {
    dispose?.();
    dispose = null;
  };
  const lifecycle: PanelLifecycle = {
    create,
    show: () => {
      create();
      controller?.show();
    },
    hide: () => {
      controller?.hide();
    },
    destroy,
  };
  // The pinned UXP types model panels as an array; the host API requires an ID-keyed object.
  const setup = entrypoints.setup.bind(entrypoints) as unknown as (value: {
    plugin: { create(): void; destroy(): void };
    panels: Record<string, PanelLifecycle>;
  }) => void;
  setup.call(entrypoints, { plugin: { create, destroy }, panels: { sugarcubes: lifecycle } });
}
