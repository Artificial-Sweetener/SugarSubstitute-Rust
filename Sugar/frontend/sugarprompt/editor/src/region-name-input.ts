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
/** A visibly editable native input owns one temporary regional name and its local input/clipboard/history. */
import { validRegionName } from '../../core/src/index.js';
import type { InputTypography } from './native-typography.js';

/** Prepared viewport geometry follows native transforms while draft width stays in unscaled CSS pixels. */
export interface RegionNameGeometry {
  readonly center: number;
  readonly top: number;
  readonly height: number;
  readonly maximumWidth: number;
  readonly width: number;
  readonly scaleX: number;
  readonly scaleY: number;
  readonly typography: InputTypography;
  readonly color: string;
}

/** Native draft text is authoritative; its inert meter derives glyph width and is never another editor. */
export class RegionNameInput {
  readonly element: HTMLInputElement;
  private readonly root: HTMLDivElement;
  private readonly meter: HTMLSpanElement;
  private disposed = false;
  private readonly resolveFocus: () => void;
  private geometry: RegionNameGeometry | null = null;
  /** Bind one finite literal input and its semantic completion callbacks. */
  constructor(
    value: string,
    finish: (cancel: boolean, outside: boolean) => void,
    measure: () => void,
  ) {
    this.root = document.createElement('div');
    this.root.setAttribute('role', 'region');
    this.root.setAttribute('aria-label', 'Inline region name editing');
    const input = (this.element = document.createElement('input'));
    input.type = 'text';
    input.className = 'sp-region-name-input';
    input.setAttribute('aria-label', 'Region name');
    input.autocomplete = 'off';
    input.spellcheck = false;
    input.value = value;
    Object.assign(input.style, {
      position: 'fixed',
      zIndex: '50',
      margin: '0',
      padding: '0 6px',
      boxSizing: 'border-box',
      border: '0',
      borderRadius: '0',
      background: '#ffffff',
      outline: '1px solid #4f6df5',
      textAlign: 'center',
      transformOrigin: 'top left',
    });
    this.meter = document.createElement('span');
    this.meter.setAttribute('aria-hidden', 'true');
    Object.assign(this.meter.style, {
      position: 'fixed',
      visibility: 'hidden',
      whiteSpace: 'pre',
      pointerEvents: 'none',
    });
    this.meter.textContent = value;
    this.root.append(input, this.meter);
    /** Proposed insertion is validated against the actual native selection, including paste. */
    const permits = (text: string): boolean =>
      validRegionName(
        input.value.slice(0, input.selectionStart ?? 0) +
          text +
          input.value.slice(input.selectionEnd ?? 0),
      );
    input.addEventListener('beforeinput', (event) => {
      if (
        event instanceof InputEvent &&
        !event.isComposing &&
        event.data !== null &&
        !permits(event.data)
      )
        event.preventDefault();
    });
    input.addEventListener('paste', (event) => {
      if (!permits(event.clipboardData?.getData('text/plain') ?? '')) event.preventDefault();
    });
    input.addEventListener('input', () => {
      this.meter.textContent = input.value;
      input.setAttribute('aria-invalid', String(!validRegionName(input.value)));
      measure();
    });
    input.addEventListener('keydown', (event) => {
      if (!event.isComposing && (event.key === 'Enter' || event.key === 'Escape')) {
        event.preventDefault();
        finish(event.key === 'Escape', false);
      }
    });
    this.resolveFocus = () => {
      queueMicrotask(() => {
        if (!this.disposed && document.activeElement !== input && document.hasFocus())
          finish(false, true);
      });
    };
    input.addEventListener('blur', this.resolveFocus);
    window.addEventListener('focus', this.resolveFocus, true);
    document.body.append(this.root);
  }
  /** Glyph width is read only by the native measurement owner. */
  textWidth(): number {
    return this.meter.getBoundingClientRect().width;
  }
  /** Apply measured geometry and typography without touching CodeMirror-managed content. */
  place(value: RegionNameGeometry): void {
    const previous = this.geometry;
    if (
      previous &&
      previous.center === value.center &&
      previous.top === value.top &&
      previous.height === value.height &&
      previous.maximumWidth === value.maximumWidth &&
      previous.width === value.width &&
      previous.scaleX === value.scaleX &&
      previous.scaleY === value.scaleY &&
      previous.color === value.color &&
      Object.entries(value.typography).every(
        ([key, text]) => previous.typography[key as keyof InputTypography] === text,
      )
    )
      return;
    this.geometry = value;
    Object.assign(this.meter.style, value.typography);
    Object.assign(this.element.style, value.typography, {
      left: `${value.center - (value.width * value.scaleX) / 2}px`,
      top: `${value.top}px`,
      width: `${value.width}px`,
      height: `${value.height / value.scaleY}px`,
      lineHeight: 'normal',
      color: value.color,
      transform: `scale(${value.scaleX}, ${value.scaleY})`,
    });
  }
  /** Native selection begins once; geometry changes never reselect or replace the draft. */
  focus(): void {
    this.element.focus({ preventScroll: true });
    this.element.select();
  }
  /** Dispose before subtree removal delivers blur or queued completion. */
  destroy(): void {
    this.disposed = true;
    this.geometry = null;
    window.removeEventListener('focus', this.resolveFocus, true);
    this.root.remove();
  }
}
