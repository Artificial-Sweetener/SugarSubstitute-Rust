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
type ElementChild = Node | string | number | null | undefined;
type ElementChildren = ElementChild | ElementChild[];
type ElementProps = Record<string, unknown>;

export function $el(
  tag: string,
  props: ElementProps | ElementChildren = {},
  children: ElementChildren = [],
): HTMLElement {
  let resolvedProps = props;
  let resolvedChildren = children;
  if (
    Array.isArray(props) ||
    props instanceof Node ||
    typeof props === 'string' ||
    typeof props === 'number'
  ) {
    resolvedChildren = props;
    resolvedProps = {};
  }
  const parts = String(tag || 'div').split('.');
  const tagName = parts[0] || 'div';
  const element = document.createElement(tagName);
  if (parts.length > 1) {
    element.className = parts.slice(1).join(' ');
  }
  if (resolvedProps && typeof resolvedProps === 'object') {
    for (const [key, value] of Object.entries(resolvedProps)) {
      if (key === 'class' || key === 'className') {
        element.className = String(value);
      } else if (key === 'style' && value && typeof value === 'object') {
        Object.assign(element.style, value);
      } else if (key === 'textContent') {
        element.textContent = String(value ?? '');
      } else if (key === 'value' && 'value' in element) {
        (element as HTMLElement & { value: string }).value = String(value ?? '');
      } else if (key.startsWith('on') && typeof value === 'function') {
        element.addEventListener(key.slice(2).toLowerCase(), value as EventListener);
      } else if (value !== false && value != null) {
        element.setAttribute(key, String(value));
      }
    }
  }
  const list = Array.isArray(resolvedChildren) ? resolvedChildren : [resolvedChildren];
  for (const child of list) {
    if (child == null) {
      continue;
    }
    if (typeof child === 'string') {
      element.appendChild(document.createTextNode(child));
    } else {
      if (child instanceof Node) element.appendChild(child);
    }
  }
  return element;
}
