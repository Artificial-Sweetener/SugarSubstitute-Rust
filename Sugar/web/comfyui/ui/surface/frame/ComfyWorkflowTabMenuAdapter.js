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
/** Project Comfy's lazy tab action model into the shared Fluent menu without copying host policy. */
import { isRecord } from '../../types/common.js';
import { findVueComponent, findVueComponents, resolveVueElement } from '../ComfyVueTree.js';
/** Serialize short-lived native portal reads so rapid context clicks cannot share capture state. */
export class ComfyWorkflowTabMenuAdapter {
    document;
    #pending = Promise.resolve();
    /** Retain only the browser boundary that owns the mounted native tabs. */
    constructor(document) {
        this.document = document;
    }
    /** Read the exact tab's translated commands, availability and conditional actions. */
    read(tab, path, report) {
        const result = this.#pending.then(() => this.#capture(tab, path, report));
        // The caller receives the original rejection; the sequencing tail must still accept later reads.
        this.#pending = result.then(() => undefined, () => undefined);
        return result;
    }
    /** Mount Comfy's lazy action list invisibly and close it before presenting the shared menu. */
    async #capture(tab, path, report) {
        if (!tab.isConnected)
            throw new Error('Comfy workflow tab was replaced; reopen its menu.');
        if (typeof path !== 'string' || !path)
            throw new Error('Comfy workflow tab identity is unavailable.');
        const app = this.document.getElementById('vue-app');
        const root = app ? Reflect.get(app, '_vnode') : null;
        const workflow = findVueComponents(root, 'WorkflowTab').find((node) => {
            const component = isRecord(node.component) ? node.component : {};
            const props = isRecord(component.props) ? component.props : {};
            const option = isRecord(props.workflowOption) ? props.workflowOption : {};
            return isRecord(option.workflow) && option.workflow.path === path;
        });
        const menu = findVueComponent(workflow, 'ContextMenuRoot');
        const component = isRecord(menu?.component) ? menu.component : {};
        const provides = isRecord(component.provides) ? component.provides : {};
        const key = Reflect.ownKeys(provides).find((candidate) => String(candidate) === 'Symbol(ContextMenuRootContext)');
        const context = key === undefined ? null : Reflect.get(provides, key);
        if (!isRecord(context) || typeof context.onOpenChange !== 'function')
            throw new Error('Comfy workflow tab menu controls are unavailable.');
        const setOpen = context.onOpenChange;
        const frame = this.document.documentElement;
        frame.setAttribute('data-sc-tab-menu-capture', '');
        try {
            setOpen.call(context, true);
            // Comfy publishes this computed model only while its retained tab portal is mounted.
            for (let attempt = 0; attempt < 4; attempt++) {
                await Promise.resolve();
                const list = findVueComponent(menu, 'WorkflowActionsList');
                const props = isRecord(list?.props) ? list.props : {};
                const content = findVueComponent(menu, 'ContextMenuContent');
                const element = content ? resolveVueElement(content) : null;
                const nativeSurface = element?.getAttribute('role') === 'menu'
                    ? element
                    : (element?.querySelector('[role="menu"]') ?? null);
                if (Array.isArray(props.items) && nativeSurface) {
                    if (!tab.isConnected)
                        throw new Error('Comfy workflow tab was replaced; reopen its menu.');
                    return projectEntries(props.items, report, () => tab.isConnected);
                }
                const window = this.document.defaultView;
                if (!window)
                    break;
                await new Promise((resolve) => window.requestAnimationFrame(() => resolve()));
            }
            throw new Error('Comfy workflow tab actions did not become available.');
        }
        finally {
            try {
                setOpen.call(context, false);
                await Promise.resolve();
                const window = this.document.defaultView;
                if (window) {
                    await new Promise((resolve) => window.requestAnimationFrame(() => resolve()));
                    // Let Reka's queued focus-scope cleanup finish before the shared menu receives its actions.
                    await new Promise((resolve) => window.setTimeout(() => resolve(), 0));
                }
            }
            finally {
                frame.removeAttribute('data-sc-tab-menu-capture');
            }
        }
    }
}
/** Validate native rows and retain their callbacks instead of reconstructing workflow operations. */
function projectEntries(values, report, connected) {
    const entries = [];
    for (const value of values) {
        if (!isRecord(value))
            throw new Error('Comfy workflow tab menu contains an invalid action.');
        if ((value.visible !== undefined && typeof value.visible !== 'boolean') ||
            (value.disabled !== undefined && typeof value.disabled !== 'boolean'))
            throw new Error('Comfy workflow tab menu availability is malformed.');
        if (value.visible === false)
            continue;
        if (value.separator === true) {
            if (entries.length && !entries.at(-1)?.separator)
                entries.push({ label: '', separator: true });
            continue;
        }
        if (typeof value.label !== 'string' || typeof value.command !== 'function')
            throw new Error('Comfy workflow tab menu action is missing its label or command.');
        const command = value.command;
        const disabled = value.disabled === true;
        entries.push({
            label: value.label,
            disabled,
            ...(typeof value.id === 'string'
                ? { attributes: { 'data-sc-native-tab-action': value.id } }
                : {}),
            ...(typeof value.icon === 'string' || isRecord(value.overlayIcon)
                ? { icon: (document) => nativeIcon(document, value) }
                : {}),
            select: () => {
                if (disabled)
                    return;
                try {
                    if (!connected())
                        throw new Error('Comfy workflow tab was replaced; reopen its menu.');
                    void Promise.resolve(command.call(value)).catch(report);
                }
                catch (error) {
                    report(error);
                }
            },
        });
    }
    if (entries.at(-1)?.separator)
        entries.pop();
    return entries;
}
/** Preserve host icon classes and compound close-direction glyphs without parsing markup. */
function nativeIcon(document, action) {
    const icon = document.createElement('span');
    icon.setAttribute('aria-hidden', 'true');
    const overlay = isRecord(action.overlayIcon) ? action.overlayIcon : null;
    if (!overlay) {
        if (typeof action.icon === 'string')
            icon.className = action.icon;
        return icon;
    }
    icon.style.position = 'relative';
    icon.style.width = '1em';
    icon.style.height = '1em';
    const main = document.createElement('i');
    const sub = document.createElement('i');
    if (typeof overlay.mainIcon === 'string')
        main.className = overlay.mainIcon;
    if (typeof overlay.subIcon === 'string')
        sub.className = overlay.subIcon;
    sub.style.position = 'absolute';
    sub.style[overlay.positionX === 'left' ? 'left' : 'right'] = '0';
    sub.style[overlay.positionY === 'top' ? 'top' : 'bottom'] = '0';
    const scale = overlay.subIconScale;
    sub.style.fontSize = `${String(typeof scale === 'number' && scale > 0 && scale <= 1 ? scale * 100 : 50)}%`;
    icon.append(main, sub);
    return icon;
}
