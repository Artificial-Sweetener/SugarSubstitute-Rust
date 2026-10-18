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

/** Version-one geometry uses logical CSS pixels and the reference native caption metrics. */
export interface DesktopFrameConfiguration {
  readonly schemaVersion: 1;
  readonly captionWidth: 138;
  readonly titlebarHeight: 34;
  readonly darkTheme: boolean;
}

/** Report accepted presentation independently of Comfy workflow or execution readiness. */
export interface DesktopFrameReceipt extends DesktopFrameConfiguration {
  readonly mounted: boolean;
}

/** Expose one presentation owner to the facade without sharing workflow or native authority. */
export interface DesktopFramePort {
  configureDesktopFrame(configuration: unknown): DesktopFrameReceipt;
  readDesktopFrame(): DesktopFrameReceipt | null;
}

interface AttributeChange {
  readonly element: HTMLElement;
  readonly name: string;
  readonly previous: string | null;
}
interface StyleChange {
  readonly element: HTMLElement;
  readonly name: string;
  readonly previous: string;
  readonly priority: string;
  readonly applied: string;
}

/** Reject unknown protocol fields and unsupported geometry before touching browser state. */
export function desktopFrameConfiguration(value: unknown): DesktopFrameConfiguration {
  if (
    !isRecord(value) ||
    Array.isArray(value) ||
    Object.keys(value).sort().join(',') !== 'captionWidth,darkTheme,schemaVersion,titlebarHeight' ||
    value.schemaVersion !== 1 ||
    value.captionWidth !== 138 ||
    value.titlebarHeight !== 34 ||
    typeof value.darkTheme !== 'boolean'
  )
    throw new TypeError(
      'SugarSubstitute desktop frame protocol 1 requires supported caption geometry.',
    );
  return Object.freeze({
    schemaVersion: 1,
    captionWidth: 138,
    titlebarHeight: 34,
    darkTheme: value.darkTheme,
  });
}

/** Preserve exact host attributes and styles while the existing frame overlays native Mica. */
export class DesktopFramePresentation {
  readonly #attributes: AttributeChange[] = [];
  readonly #styles: StyleChange[] = [];
  #configuration: DesktopFrameConfiguration | null = null;
  #mount: HTMLElement | null = null;

  /** Retain validated presentation for remounts without changing persisted browser preferences. */
  configure(value: unknown): void {
    const configuration = desktopFrameConfiguration(value);
    const mount = this.#mount;
    this.detach();
    this.#configuration = configuration;
    if (mount) this.attach(mount);
  }

  /** Borrow the current accepted configuration and genuine connected mount state. */
  read(): DesktopFrameReceipt | null {
    return this.#configuration
      ? Object.freeze({ ...this.#configuration, mounted: this.#mount?.isConnected === true })
      : null;
  }

  /** Mark only the real frame's ancestor chain; selected tabs and graph surfaces keep their fills. */
  attach(mount: HTMLElement): void {
    this.detach();
    this.#mount = mount;
    const configuration = this.#configuration;
    if (!configuration) return;
    for (let surface: HTMLElement | null = mount; surface; surface = surface.parentElement)
      this.#attribute(surface, 'data-sc-desktop-frame-surface');
    this.#attribute(mount, 'data-sc-desktop-frame-host');
    this.#style(mount, '--sc-native-frame-background', 'transparent');
    this.#style(mount, '--sc-native-frame-foreground', configuration.darkTheme ? '#fff' : '#000');
    this.#style(mount, '--sc-native-caption-width', `${String(configuration.captionWidth)}px`);
  }

  /** Restore only values still owned by this presentation, preserving later host mutations. */
  detach(): void {
    for (const change of this.#styles.splice(0).reverse()) {
      if (change.element.style.getPropertyValue(change.name) !== change.applied) continue;
      if (change.previous)
        change.element.style.setProperty(change.name, change.previous, change.priority);
      else change.element.style.removeProperty(change.name);
    }
    for (const change of this.#attributes.splice(0).reverse()) {
      if (change.element.getAttribute(change.name) !== '') continue;
      if (change.previous === null) change.element.removeAttribute(change.name);
      else change.element.setAttribute(change.name, change.previous);
    }
    this.#mount = null;
  }

  /** End configuration along with the owning frontend lifetime. */
  dispose(): void {
    this.detach();
    this.#configuration = null;
  }

  /** Retain an exact host attribute before assigning this lifetime's marker. */
  #attribute(element: HTMLElement, name: string): void {
    this.#attributes.push({ element, name, previous: element.getAttribute(name) });
    element.setAttribute(name, '');
  }

  /** Preserve custom-property priority and previous value without replacing unrelated inline styles. */
  #style(element: HTMLElement, name: string, applied: string): void {
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
