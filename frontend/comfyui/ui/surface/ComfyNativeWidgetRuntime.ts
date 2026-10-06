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
/** Load Comfy's exact exported widget components independently of the active graph renderer. */
import {
  discoverComfyFrontendModules,
  discoverReferencedModuleUrls,
} from './ComfyFrontendModuleGraph.js';
import {
  loadComfyVueRenderRuntime,
  NativeRendererCompatibilityError,
} from './ComfyRuntimeModuleLoader.js';
import type { ComfyVueRenderRuntime } from './ComfyRuntimeModuleLoader.js';
import type { UnknownRecord } from '../types/common.js';
import { isRecord } from '../types/common.js';

/** Retain installed component identities alongside Comfy's own Vue render primitives. */
export interface ComfyNativeWidgetRuntime extends ComfyVueRenderRuntime {
  components: ReadonlyMap<string, UnknownRecord>;
}

/** Resolve hashed asset names from the installed module graph rather than copying widget code. */
export async function loadComfyNativeWidgetRuntime(
  document: Document,
): Promise<ComfyNativeWidgetRuntime> {
  const names = ['WidgetInputNumber', 'WidgetSelect', 'WidgetInputText', 'WidgetToggleSwitch'];
  const [runtime, discovery] = await Promise.all([
    loadComfyVueRenderRuntime(document),
    discoverComfyFrontendModules(
      document,
      [
        {
          key: 'widgets',
          matches: (module) => names.every((name) => module.source.includes(`./${name}-`)),
        },
      ],
      async (url) => {
        const response = await fetch(url, { signal: AbortSignal.timeout(5_000) });
        if (!response.ok) throw new Error(`Comfy widget discovery failed (${response.status}).`);
        return response.text();
      },
    ),
  ]);
  const components = new Map<string, UnknownRecord>();
  const registry = discovery.modules.get('widgets');
  if (!registry)
    throw new NativeRendererCompatibilityError('Comfy native widget registry is unavailable.');
  const urls = discoverReferencedModuleUrls(registry.source, registry.url);
  for (const name of names) {
    const url = urls.find((url) => new URL(url).pathname.split('/').pop()?.startsWith(`${name}-`));
    if (!url) throw new NativeRendererCompatibilityError(`Comfy no longer exposes ${name}.`);
    const loaded: unknown = await import(url);
    const component = isRecord(loaded) ? loaded.default : null;
    if (!isRecord(component) || (component.__name !== name && component.name !== name))
      throw new NativeRendererCompatibilityError(
        `Comfy ${name} has an incompatible component export.`,
      );
    components.set(name, component);
  }
  return { ...runtime, components };
}
