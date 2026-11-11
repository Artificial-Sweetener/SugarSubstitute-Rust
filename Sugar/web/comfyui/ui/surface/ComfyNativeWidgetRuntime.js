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
import { fetchComfyRuntimeText } from './ComfyRuntimeAssetRequest.js';
import { discoverComfyFrontendModules, discoverReferencedModuleUrls, } from './ComfyFrontendModuleGraph.js';
import { loadComfyVueRenderRuntime, NativeRendererCompatibilityError, } from './ComfyRuntimeModuleLoader.js';
import { isRecord } from '../types/common.js';
/** Resolve hashed asset names from the installed module graph rather than copying widget code. */
export async function loadComfyNativeWidgetRuntime(document) {
    const names = ['WidgetInputNumber', 'WidgetSelect', 'WidgetInputText', 'WidgetToggleSwitch'];
    const [runtime, discovery] = await Promise.all([
        loadComfyVueRenderRuntime(document),
        discoverComfyFrontendModules(document, [
            {
                key: 'widgets',
                matches: (module) => names.every((name) => module.source.includes(`./${name}-`)),
            },
        ], fetchComfyRuntimeText),
    ]);
    const components = new Map();
    const registry = discovery.modules.get('widgets');
    if (!registry)
        throw new NativeRendererCompatibilityError('Comfy native widget registry is unavailable.');
    const urls = discoverReferencedModuleUrls(registry.source, registry.url);
    for (const name of names) {
        const url = urls.find((url) => new URL(url).pathname.split('/').pop()?.startsWith(`${name}-`));
        if (!url)
            throw new NativeRendererCompatibilityError(`Comfy no longer exposes ${name}.`);
        const loaded = await import(url);
        const component = isRecord(loaded) ? loaded.default : null;
        if (!isRecord(component) || (component.__name !== name && component.name !== name))
            throw new NativeRendererCompatibilityError(`Comfy ${name} has an incompatible component export.`);
        components.set(name, component);
    }
    return { ...runtime, components };
}
