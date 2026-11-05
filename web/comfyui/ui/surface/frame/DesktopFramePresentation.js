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
/** Negotiate document-local desktop frame presentation without granting native privileges. */
import { isRecord } from '../../types/common.js';
import { DESKTOP_BODY_WASH_PROPERTY, DESKTOP_CHROME_WASH_PROPERTY, desktopMaterialWash, } from './DesktopMaterialWash.js';
/** Reject unknown protocol fields and unsupported geometry before touching browser state. */
export function desktopFrameConfiguration(value) {
    if (!isRecord(value) ||
        Array.isArray(value) ||
        Object.keys(value).sort().join(',') !== 'captionWidth,darkTheme,schemaVersion,titlebarHeight' ||
        value.schemaVersion !== 1 ||
        value.captionWidth !== 138 ||
        value.titlebarHeight !== 34 ||
        typeof value.darkTheme !== 'boolean')
        throw new TypeError('SugarSubstitute desktop frame protocol 1 requires supported caption geometry.');
    return Object.freeze({
        schemaVersion: 1,
        captionWidth: 138,
        titlebarHeight: 34,
        darkTheme: value.darkTheme,
    });
}
/** Preserve exact host attributes and styles while the existing frame overlays native Mica. */
export class DesktopFramePresentation {
    #attributes = [];
    #styles = [];
    #configuration = null;
    #mount = null;
    /** Retain validated presentation for remounts without changing persisted browser preferences. */
    configure(value) {
        const configuration = desktopFrameConfiguration(value);
        const mount = this.#mount;
        this.detach();
        this.#configuration = configuration;
        if (mount)
            this.attach(mount);
    }
    /** Borrow the current accepted configuration and genuine connected mount state. */
    read() {
        return this.#configuration
            ? Object.freeze({ ...this.#configuration, mounted: this.#mount?.isConnected === true })
            : null;
    }
    /** Mark only the real frame's ancestor chain and publish the Mica washes on the document root. */
    attach(mount) {
        this.detach();
        this.#mount = mount;
        const configuration = this.#configuration;
        if (!configuration)
            return;
        for (let surface = mount; surface; surface = surface.parentElement)
            this.#attribute(surface, 'data-sc-desktop-frame-surface');
        this.#attribute(mount, 'data-sc-desktop-frame-host');
        this.#style(mount, '--sc-native-frame-background', 'transparent');
        this.#style(mount, '--sc-native-frame-foreground', configuration.darkTheme ? '#fff' : '#000');
        this.#style(mount, '--sc-native-caption-width', `${String(configuration.captionWidth)}px`);
        const wash = desktopMaterialWash(configuration.darkTheme);
        const root = mount.ownerDocument.documentElement;
        this.#style(root, DESKTOP_BODY_WASH_PROPERTY, wash.body);
        this.#style(root, DESKTOP_CHROME_WASH_PROPERTY, wash.chrome);
    }
    /** Restore only values still owned by this presentation, preserving later host mutations. */
    detach() {
        for (const change of this.#styles.splice(0).reverse()) {
            if (change.element.style.getPropertyValue(change.name) !== change.applied)
                continue;
            if (change.previous)
                change.element.style.setProperty(change.name, change.previous, change.priority);
            else
                change.element.style.removeProperty(change.name);
        }
        for (const change of this.#attributes.splice(0).reverse()) {
            if (change.element.getAttribute(change.name) !== '')
                continue;
            if (change.previous === null)
                change.element.removeAttribute(change.name);
            else
                change.element.setAttribute(change.name, change.previous);
        }
        this.#mount = null;
    }
    /** End configuration along with the owning frontend lifetime. */
    dispose() {
        this.detach();
        this.#configuration = null;
    }
    /** Retain an exact host attribute before assigning this lifetime's marker. */
    #attribute(element, name) {
        this.#attributes.push({ element, name, previous: element.getAttribute(name) });
        element.setAttribute(name, '');
    }
    /** Preserve custom-property priority and previous value without replacing unrelated inline styles. */
    #style(element, name, applied) {
        this.#styles.push({
            element,
            name,
            previous: element.style.getPropertyValue(name),
            priority: element.style.getPropertyPriority(name),
            applied,
        });
        element.style.setProperty(name, applied);
    }
}
