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
/** Finite resolved visual properties preserve inert native paint without copying unrelated browser CSS state per proposal. */
const paintProperties = [
  'display',
  'position',
  'z-index',
  'inset',
  'box-sizing',
  'width',
  'height',
  'min-width',
  'max-width',
  'min-height',
  'max-height',
  'margin',
  'padding',
  'border-top',
  'border-right',
  'border-bottom',
  'border-left',
  'border-radius',
  'border-collapse',
  'border-spacing',
  'background-color',
  'background-image',
  'background-position',
  'background-size',
  'background-repeat',
  'background-origin',
  'background-clip',
  'box-shadow',
  'outline',
  'outline-offset',
  'opacity',
  'visibility',
  'color',
  'color-scheme',
  'font',
  'font-family',
  'font-size',
  'font-weight',
  'font-style',
  'font-stretch',
  'font-width',
  'font-feature-settings',
  'font-variation-settings',
  'font-kerning',
  'font-optical-sizing',
  'font-variant-ligatures',
  'font-variant-numeric',
  'font-variant-east-asian',
  'font-variant-caps',
  'font-variant-position',
  'font-variant-alternates',
  'font-synthesis',
  'font-synthesis-weight',
  'font-synthesis-style',
  'font-synthesis-small-caps',
  'font-synthesis-position',
  'font-size-adjust',
  'line-height',
  'letter-spacing',
  'word-spacing',
  'white-space',
  'word-break',
  'overflow-wrap',
  'tab-size',
  'direction',
  'unicode-bidi',
  'text-transform',
  'text-rendering',
  'writing-mode',
  'text-orientation',
  'text-align',
  'text-align-last',
  'text-indent',
  'text-decoration',
  'text-decoration-line',
  'text-decoration-style',
  'text-decoration-color',
  'text-decoration-thickness',
  'text-underline-offset',
  'text-underline-position',
  'text-shadow',
  'text-overflow',
  'vertical-align',
  'line-break',
  'hyphens',
  'text-wrap',
  'text-wrap-mode',
  'text-wrap-style',
  'white-space-collapse',
  'text-autospace',
  'text-spacing-trim',
  'text-combine-upright',
  'text-size-adjust',
  '-webkit-text-fill-color',
  '-webkit-text-stroke',
  'overflow-x',
  'overflow-y',
  'clip-path',
  'object-fit',
  'object-position',
  'transform',
  'transform-origin',
  'zoom',
  'filter',
  'isolation',
  'mix-blend-mode',
  'flex',
  'flex-flow',
  'order',
  'gap',
  'align-items',
  'align-content',
  'align-self',
  'justify-content',
  'justify-items',
  'justify-self',
  'grid-template',
  'grid-auto-flow',
  'grid-auto-columns',
  'grid-auto-rows',
  'grid-column',
  'grid-row',
  'fill',
  'fill-rule',
  'fill-opacity',
  'stroke',
  'stroke-width',
  'stroke-opacity',
  'stroke-linecap',
  'stroke-linejoin',
  'stroke-dasharray',
  'stroke-dashoffset',
  'vector-effect',
  '--sp-lora-accent',
  '--sp-lora-height',
  '--sp-lora-taper',
  '--sp-weight-cell-padding',
  '--sp-region-rule-width',
  '--sp-region-color',
  '--sp-region-hover-color',
  '--sp-region-rail-width',
  '--sp-region-title-width',
  '--sugarprompt-lora-surface',
  '--sugarprompt-error-foreground',
  '--sugarprompt-content-inset',
  '--sugarprompt-line-active',
  '--sugarprompt-line-alternate',
] as const;

/** Frozen resolved declarations retain original opacity without rereading native CSS or parsing serialized values. */
export interface PreparedReorderPaintStyle {
  readonly cssText: string;
  readonly opacity: string;
  readonly display: string;
  readonly transform: string;
  readonly position: string;
  /** Existing resolved physical inset shorthand; exact0px witnesses four unshifted sides. */
  readonly inset: string;
}
/** Read the exact CSSOM named standard attribute; custom/unsupported fields retain native method serialization and raw failures. */
export function readReorderPaintProperty(original: CSSStyleDeclaration, name: string): string {
  if (!name.startsWith('--')) {
    // CSSOM defines dashed named getters too; the pinned DOM type lacks a string index, so validate this exact external field.
    const value: unknown = original[name as keyof CSSStyleDeclaration];
    if (typeof value === 'string') return value;
  }
  return original.getPropertyValue(name);
}

/** Canonical wrappers retain their actual cascade; only genuine inline text fill is transported, including absence and priority. */
export function prepareReorderWrapperPaintStyle(
  original: CSSStyleDeclaration,
  authored: CSSStyleDeclaration,
  baselineOpacity?: string,
): PreparedReorderPaintStyle {
  const value = authored.getPropertyValue('-webkit-text-fill-color'),
    priority = value ? authored.getPropertyPriority('-webkit-text-fill-color') : '';
  return prepareReorderPaintStyle(original, baselineOpacity, {
    value,
    priority,
  });
}

/** One bounded CM READ serializer preserves resolved paint; nativeWidth carries the same request's measured CSS-local width instead of a rounded CSSOM used value. */
export function prepareReorderPaintStyle(
  original: CSSStyleDeclaration,
  baselineOpacity?: string,
  authoredFill?: { readonly value: string; readonly priority: string },
  nativeWidth?: string,
): PreparedReorderPaintStyle {
  let declarations = '',
    opacity = '1',
    display = '',
    transform = '',
    position = '',
    inset = '';
  for (const name of paintProperties) {
    const value =
      name === 'width' && nativeWidth !== undefined
        ? nativeWidth
        : name === '-webkit-text-fill-color' && authoredFill !== undefined
          ? authoredFill.value
          : name === 'opacity' && baselineOpacity !== undefined
            ? baselineOpacity
            : readReorderPaintProperty(original, name);
    if (name === 'opacity') opacity = value || '1';
    if (name === 'display') display = value;
    if (name === 'transform') transform = value;
    if (name === 'position') position = value;
    if (name === 'inset') inset = value;
    if (value) {
      const inlineFill = name === '-webkit-text-fill-color' && authoredFill !== undefined;
      declarations += `${name}:${inlineFill ? value : value.includes('url(') ? 'none' : value}${inlineFill && authoredFill.priority ? ` !${authoredFill.priority}` : ''};`;
    }
  }
  return Object.freeze({
    cssText: declarations,
    opacity,
    display,
    transform,
    position,
    inset,
  });
}

/** Convert a precise native border-box width to its original CSS sizing space without rounding measured geometry. */
export function prepareReorderPaintWidth(
  style: CSSStyleDeclaration,
  borderWidth: number,
  current: () => boolean,
): string | null {
  if (!current() || !Number.isFinite(borderWidth) || borderWidth < 0) return null;
  let width = borderWidth;
  const sizing = readReorderPaintProperty(style, 'box-sizing');
  if (!current()) return null;
  if (sizing !== 'border-box') {
    for (const name of [
      'padding-left',
      'padding-right',
      'border-left-width',
      'border-right-width',
    ]) {
      const value = readReorderPaintProperty(style, name);
      if (!current()) return null;
      const edge = value ? Number.parseFloat(value) : 0;
      if (!Number.isFinite(edge) || edge < 0) return null;
      width -= edge;
    }
  }
  return current() && Number.isFinite(width) && width >= 0 ? `${width}px` : null;
}

/** Rounded CSS dimensions remain an independent admission witness, never the published native geometry. */
export function readReorderPaintSizeWitness(
  style: CSSStyleDeclaration,
  axis: 'width' | 'height',
  current: () => boolean,
): number | null {
  const literal = readReorderPaintProperty(style, axis);
  if (!current()) return null;
  let size = Number.parseFloat(literal);
  const sizing = readReorderPaintProperty(style, 'box-sizing');
  if (!current()) return null;
  if (sizing !== 'border-box') {
    for (const edge of axis === 'width' ? ['left', 'right'] : ['top', 'bottom']) {
      for (const name of [`padding-${edge}`, `border-${edge}-width`]) {
        const value = readReorderPaintProperty(style, name);
        if (!current()) return null;
        size += Number.parseFloat(value) || 0;
      }
    }
  }
  return current() && Number.isFinite(size) && size > 0 ? size : null;
}
