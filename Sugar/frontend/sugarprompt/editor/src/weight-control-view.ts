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
/** A finite native portal paints prepared arrows; CodeMirror continues to own content DOM, selection and history. */
import { requireValue } from '../../core/src/required.js';
import type { WeightControlGeometry } from './weight-control-geometry.js';
/** Native arrow intents are independent of prepared geometry and source mutation. */
export type WeightDirection = 'increase' | 'decrease';
/** Own two native buttons and their DOM listeners outside CodeMirror-managed content. */
export class WeightControlView {
  readonly root: HTMLDivElement;
  readonly increase: HTMLButtonElement;
  readonly decrease: HTMLButtonElement;
  private hovered: WeightDirection | null = null;
  private pressed: WeightDirection | null = null;
  /** Bind finite arrow input to the mounted pointer owner. */
  constructor(
    press: (direction: WeightDirection, event: PointerEvent) => void,
    activate: (direction: WeightDirection) => void,
  ) {
    this.root = document.createElement('div');
    this.root.className = 'sp-weight-controls';
    this.root.setAttribute('role', 'group');
    this.root.setAttribute('aria-label', 'Emphasis weight controls');
    Object.assign(this.root.style, {
      position: 'fixed',
      left: '0',
      top: '0',
      zIndex: '40',
      pointerEvents: 'none',
    });
    this.root.hidden = true;
    /** Construct a literal triangle button with a native accessible name and finite owning DOM lifetime. */
    const button = (direction: WeightDirection): HTMLButtonElement => {
      const element = document.createElement('button');
      element.type = 'button';
      element.setAttribute(
        'aria-label',
        `${direction === 'increase' ? 'Increase' : 'Decrease'} emphasis weight`,
      );
      Object.assign(element.style, {
        position: 'fixed',
        padding: '0',
        border: '0',
        background: 'transparent',
        color: 'inherit',
        cursor: 'pointer',
        pointerEvents: 'auto',
        display: 'grid',
        placeItems: 'center',
      });
      const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
      svg.setAttribute('viewBox', '0 0 13 10');
      svg.setAttribute('width', '100%');
      svg.setAttribute('height', '100%');
      svg.setAttribute('aria-hidden', 'true');
      const triangle = document.createElementNS('http://www.w3.org/2000/svg', 'polygon');
      triangle.setAttribute(
        'points',
        direction === 'increase' ? '6.5,3 10,7 3,7' : '3,3 10,3 6.5,7',
      );
      triangle.setAttribute('fill', 'currentColor');
      svg.append(triangle);
      element.append(svg);
      element.addEventListener('pointerdown', (event) => press(direction, event));
      element.addEventListener('click', (event) => {
        if (event.detail === 0) activate(direction);
      });
      this.root.append(element);
      return element;
    };
    this.increase = button('increase');
    this.decrease = button('decrease');
    document.body.append(this.root);
  }
  /** Consume a measured geometry snapshot without discovering source or layout during painting. */
  show(value: WeightControlGeometry): void {
    const subject =
      value.kind === 'lora'
        ? 'LoRA weight'
        : value.kind === 'wildcard'
          ? 'wildcard group'
          : 'emphasis weight';
    this.root.setAttribute(
      'aria-label',
      `${requireValue(subject[0], 'weight-control-view').toUpperCase()}${subject.slice(1)} controls`,
    );
    for (const direction of ['increase', 'decrease'] as const) {
      const rect = value[direction],
        element = this[direction];
      element.setAttribute(
        'aria-label',
        `${direction === 'increase' ? 'Increase' : 'Decrease'} ${subject}`,
      );
      Object.assign(element.style, {
        left: `${rect.left}px`,
        top: `${rect.top}px`,
        width: `${rect.width}px`,
        height: `${rect.height}px`,
      });
    }
    this.root.style.color = value.color;
    this.root.hidden = false;
  }
  /** Hidden native controls become inert and inaccessible without changing the source surface. */
  hide(): void {
    this.root.hidden = true;
    this.hovered = null;
    this.setPressed(null);
  }
  /** Native feedback reflects the currently pressed arrow. */
  setPressed(direction: WeightDirection | null): void {
    this.pressed = direction;
    this.paintFeedback();
  }
  /** Prepared pointer hit classification identifies the hovered arrow without affecting glyph geometry. */
  setHovered(direction: WeightDirection | null): void {
    if (direction === this.hovered) return;
    this.hovered = direction;
    this.paintFeedback();
  }
  /** Hover/press fills use the inherited surface text color, retaining the native arrow dimensions. */
  private paintFeedback(): void {
    for (const direction of ['increase', 'decrease'] as const) {
      const pressed = direction === this.pressed,
        hovered = direction === this.hovered;
      this[direction].style.opacity = pressed ? '0.6' : '1';
      this[direction].style.borderRadius = '4px';
      this[direction].style.background = pressed
        ? 'color-mix(in srgb, currentColor 11%, transparent)'
        : hovered
          ? 'color-mix(in srgb, currentColor 7%, transparent)'
          : 'transparent';
    }
  }
  /** Remove the owned DOM subtree and every listener retained by its buttons. */
  destroy(): void {
    this.root.remove();
  }
}
