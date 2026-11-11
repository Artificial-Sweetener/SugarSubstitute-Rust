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
/** Retain the actual ComfyUI 1.47.11 NodeFooter DOM, whose sibling underlap is part of native geometry. */
export interface NativeFooterElement {
  readonly tag: string;
  readonly attributes: Readonly<Record<string, string>>;
  readonly children: readonly (NativeFooterElement | string)[];
}

/** Capture the installed GraphView-C32XwOxo.js NodeFooter rendered by its actual Vue component. */
export const nativeAdvancedFooterDom: NativeFooterElement = {
  tag: 'div',
  attributes: {
    class: 'isolate -z-1 -mt-5 box-border flex w-full',
  },
  children: [
    {
      tag: 'button',
      attributes: {
        class:
          'relative inline-flex items-center justify-center gap-2 cursor-pointer touch-manipulation whitespace-nowrap appearance-none border-none font-inter transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg:not([width]):not([height])]:size-4 [&_svg]:shrink-0 hover:bg-secondary-background-hover p-2 pointer-events-auto h-11 text-xs font-normal box-border w-full rounded-none bg-node-component-header-surface text-node-component-slot-text pt-8 pb-3 rounded-b-xl',
        'data-testid': 'advanced-inputs-button',
      },
      children: [
        {
          tag: 'div',
          attributes: {
            class: 'flex size-full items-center justify-center gap-2',
          },
          children: [
            {
              tag: 'span',
              attributes: {
                class: 'truncate',
              },
              children: ['Show advanced inputs'],
            },
            {
              tag: 'i',
              attributes: {
                class: 'icon-[lucide--settings-2] size-4 shrink-0',
              },
              children: [],
            },
          ],
        },
      ],
    },
  ],
};
