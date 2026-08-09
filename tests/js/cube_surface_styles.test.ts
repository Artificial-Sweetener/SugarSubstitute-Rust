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
/** Characterize the structural Cube-face mode applied around native cards. */

import { ensureCubeSurfaceStyles } from '../../frontend/comfyui/ui/surface/CubeSurfaceStyles.js';

describe('ensureCubeSurfaceStyles', () => {
  test('removes SugarCubes sidebar chrome from the active Cube editor viewport', () => {
    ensureCubeSurfaceStyles(document);

    const css = document.getElementById('sugarcubes-cube-surface-styles')?.textContent ?? '';
    expect(css).toMatch(
      /body\.sugarcubes-cube-editor-workspace[\s\S]*?\.p-splitter:has\(\.sugarcubes-sidebar-panel\)[\s\S]*?> \.side-bar-panel,[\s\S]*?> \.p-splitter-gutter\s*\{[^}]*display:\s*none\s*!important;/s,
    );
  });

  test('suppresses native ports and subgraph footers while preserving advanced inputs', () => {
    ensureCubeSurfaceStyles(document);

    const css = document.getElementById('sugarcubes-cube-surface-styles')?.textContent;
    expect(css).toContain('.sugarcubes-native-node-card .lg-node-slots');
    expect(css).toContain('.sugarcubes-native-node-card .lg-slot');
    expect(css).not.toContain(
      '.sugarcubes-native-node-card [data-testid="advanced-inputs-button"]',
    );
    expect(css).toContain('.sugarcubes-native-node-card [data-testid="subgraph-enter-button"]');
    expect(css).not.toContain(
      '.lg-node[data-sugarcube-node="true"] [data-testid="subgraph-enter-button"]',
    );
    expect(css).not.toContain('.sugarcubes-native-node-card .lg-node-content');
    expect(css).toMatch(
      /\[data-sugarcube-cube-body\]\s*\{[^}]*padding-top:\s*0\s*!important;[^}]*padding-bottom:\s*0\s*!important;[^}]*overflow:\s*visible\s*!important;/s,
    );
    expect(css).toMatch(
      /\.sugarcubes-cube-node-face-host\s*\{[^}]*overflow:\s*clip;[^}]*contain:\s*layout paint style;/s,
    );
    expect(css).toMatch(
      /\[data-cube-face-body="header-only"\]\s+\[data-testid="node-inner-wrapper"\]\s*\{[^}]*overflow:\s*hidden;/s,
    );
    expect(css).not.toContain('sugarcubes-cube-editor-navigation');
  });

  test('keeps frame resize hit zones above card controls and boundary slots above resizing', () => {
    ensureCubeSurfaceStyles(document);

    const css = document.getElementById('sugarcubes-cube-surface-styles')?.textContent ?? '';
    expect(css).toMatch(/\[data-sugarcube-boundary-row\]\s*\{[^}]*z-index:\s*1001;/s);
    expect(css).toMatch(/\.sugarcubes-cube-node-edge-resize\s*\{[^}]*z-index:\s*1000;/s);
    expect(css).toMatch(/\[data-sugarcube-boundary-direction\]\s*\{[^}]*z-index:\s*1003;/s);
    expect(css).toMatch(/\[data-sugarcube-boundary-direction\]\s*\{[^}]*min-width:\s*2rem;/s);
    expect(css).toMatch(/\[data-sugarcube-boundary-direction\]\s*\{[^}]*min-height:\s*2rem;/s);
    expect(css).not.toContain('.sugarcubes-cube-face__ports');
    expect(css).not.toContain('.sugarcubes-cube-face__resize');
    expect(css).toMatch(
      /\[data-sugarcube-boundary-direction="output"\]\s*>\s*:not\(\[data-testid="slot-connection-dot"\]\)\s*\{[^}]*display:\s*none\s*!important;/s,
    );
    expect(css).toMatch(/\.sugarcubes-cube-port-leaders\s*\{[^}]*overflow:\s*hidden;/s);
  });

  test('projects the selected Cube colors onto cards and uses the darker body backdrop', () => {
    ensureCubeSurfaceStyles(document);

    const css = document.getElementById('sugarcubes-cube-surface-styles')?.textContent ?? '';
    const cubeBodyRule = css.match(/\[data-sugarcube-cube-body\]\s*\{([^}]*)\}/s)?.[1] ?? '';
    const cardHeaderRule =
      css.match(
        /\.sugarcubes-native-node-card\s+\[data-testid="node-inner-wrapper"\]\s*\{([^}]*)\}/s,
      )?.[1] ?? '';
    const cardBodyRule =
      css.match(
        /\.sugarcubes-native-node-card\s+\[data-testid\^="node-body-"\]\s*\{([^}]*)\}/s,
      )?.[1] ?? '';

    expect(cubeBodyRule).toMatch(
      /background-color:\s*var\(\s*--sugarcubes-cube-backdrop,\s*var\(--component-node-background\)\s*\)\s*!important;/,
    );
    expect(cardHeaderRule).toMatch(
      /background-color:\s*var\(--sugarcubes-cube-card-header\)\s*!important;/,
    );
    expect(cardBodyRule).toMatch(
      /background-color:\s*var\(--sugarcubes-cube-card-body\)\s*!important;/,
    );
  });

  test('centers definition identity independently of left title and right actions', () => {
    ensureCubeSurfaceStyles(document);

    const css = document.getElementById('sugarcubes-cube-surface-styles')?.textContent ?? '';
    const headerRule = css.match(/\.sugarcubes-cube-face__header\s*\{([^}]*)\}/s)?.[1] ?? '';
    expect(css).toMatch(
      /\.sugarcubes-cube-face__header\s*\{[^}]*display:\s*grid;[^}]*grid-template-columns:\s*minmax\(0,\s*1fr\)\s+auto\s+minmax\(0,\s*1fr\);/s,
    );
    expect(headerRule).not.toMatch(/cursor:/);
    expect(css).toMatch(
      /\.sugarcubes-cube-face__definition-badge\s*\{[^}]*display:\s*flex;[^}]*flex-direction:\s*column;[^}]*grid-column:\s*2;[^}]*text-align:\s*center;/s,
    );
    expect(css).toMatch(
      /\.sugarcubes-cube-face__definition-source\s*\{[^}]*font-size:\s*0\.625rem;/s,
    );
    expect(css).toMatch(/\.sugarcubes-cube-unsaved-indicator__ban\.pi\s*\{[^}]*--p-orange-400/s);
    expect(css).not.toContain('sugarcubes-cube-face__save-state');
    expect(css).toMatch(
      /\.sugarcubes-cube-face__actions\s*\{[^}]*grid-column:\s*3;[^}]*justify-self:\s*end;/s,
    );
    expect(css).toMatch(
      /\.sugarcubes-cube-face__icon img,\s*\.sugarcubes-cube-face__icon canvas\s*\{[^}]*object-fit:\s*contain;/s,
    );
  });

  test('uses Comfy elevation tokens for the pinned Cube metadata card', () => {
    ensureCubeSurfaceStyles(document);

    const css = document.getElementById('sugarcubes-cube-surface-styles')?.textContent ?? '';
    expect(css).toMatch(
      /\.sugarcubes-cube-editor-metadata\s*\{[^}]*box-shadow:\s*var\(--shadow-lg,/s,
    );
    expect(css).toMatch(
      /\.dark-theme\s+\.sugarcubes-cube-editor-metadata\s*\{[^}]*box-shadow:\s*var\(--shadow-xl,/s,
    );
  });

  test('reserves only boundary gutters that have actual ports', () => {
    ensureCubeSurfaceStyles(document);

    const css = document.getElementById('sugarcubes-cube-surface-styles')?.textContent ?? '';
    const contentRule =
      css.match(
        /\.sugarcubes-cube-face__content\s*\{\s*box-sizing:\s*border-box;([^}]*)\}/s,
      )?.[1] ?? '';

    expect(contentRule).toMatch(
      /width:\s*calc\(\s*100%\s*-\s*var\(--sugarcubes-cube-input-gutter-width,\s*0px\)\s*-\s*var\(--sugarcubes-cube-output-gutter-width,\s*0px\)\s*\);/,
    );
    expect(contentRule).toMatch(
      /margin-left:\s*var\(--sugarcubes-cube-input-gutter-width,\s*0px\);/,
    );
    expect(contentRule).toMatch(
      /margin-right:\s*var\(--sugarcubes-cube-output-gutter-width,\s*0px\);/,
    );
  });

  test('keeps Cube titlebar actions icon-only beside Comfy native controls', () => {
    ensureCubeSurfaceStyles(document);

    const css = document.getElementById('sugarcubes-cube-surface-styles')?.textContent ?? '';
    expect(css).toMatch(
      /\.sugarcubes-cube-face__actions button\s*\{[^}]*border:\s*0;[^}]*background:\s*transparent;/s,
    );
    expect(css).toMatch(
      /\.sugarcubes-cube-face__actions button\[hidden\]\s*\{[^}]*display:\s*none;/s,
    );
    expect(css).toMatch(/\.sugarcubes-cube-face__actions \.pi\s*\{[^}]*font-size:\s*1rem;/s);
  });

  test('keeps Comfy-owned corner handles above the custom Cube face', () => {
    ensureCubeSurfaceStyles(document);

    const css = document.getElementById('sugarcubes-cube-surface-styles')?.textContent ?? '';
    expect(css).toMatch(/\.lg-node\[data-sugarcube-node="true"\]\s*\{[^}]*overflow:\s*visible;/s);
    expect(css).toMatch(/\.sugarcubes-cube-node-face-host\s*\{[^}]*overflow:\s*clip;/s);
    expect(css).toMatch(
      /\.lg-node\[data-sugarcube-node="true"\]\s*>\s*\[aria-label\^="Resize from "\]\s*\{[^}]*z-index:\s*1002;/s,
    );
  });

  test('clips transformed card overflow because content height owns the vertical extent', () => {
    ensureCubeSurfaceStyles(document);

    const css = document.getElementById('sugarcubes-cube-surface-styles')?.textContent ?? '';
    const contentRule =
      css.match(
        /\.sugarcubes-cube-face__content\s*\{\s*box-sizing:\s*border-box;([^}]*)\}/s,
      )?.[1] ?? '';

    expect(contentRule).toMatch(/overflow:\s*clip;/);
    expect(contentRule).not.toMatch(/overflow-[xy]:\s*hidden;/);
  });

  test('releases only an active native combo popup from Cube clipping and stacking boundaries', () => {
    ensureCubeSurfaceStyles(document);

    const css = document.getElementById('sugarcubes-cube-surface-styles')?.textContent ?? '';
    const popperSelector = '\\[data-reka-popper-content-wrapper\\]';

    expect(css).toMatch(
      new RegExp(
        `\\.sugarcubes-cube-node-face-host:has\\(${popperSelector}\\)\\s*\\{[^}]*overflow:\\s*visible;[^}]*contain:\\s*layout style;`,
        's',
      ),
    );
    expect(css).toMatch(
      new RegExp(
        `\\.sugarcubes-cube-face:has\\(${popperSelector}\\)\\s*\\{[^}]*overflow:\\s*visible;`,
        's',
      ),
    );
    expect(css).toMatch(
      new RegExp(
        `\\.sugarcubes-cube-face__content:has\\(${popperSelector}\\)\\s*\\{[^}]*overflow:\\s*visible;[^}]*isolation:\\s*auto;`,
        's',
      ),
    );
    expect(css).toMatch(
      new RegExp(
        `\\.lg-node\\[data-sugarcube-node="true"\\]:has\\(${popperSelector}\\)\\s*\\{[^}]*z-index:\\s*10000\\s*!important;`,
        's',
      ),
    );
  });

  test('stretches the Cube face through the full resized native body', () => {
    ensureCubeSurfaceStyles(document);

    const css = document.getElementById('sugarcubes-cube-surface-styles')?.textContent ?? '';
    const faceRule = css.match(/\.sugarcubes-cube-face\s*\{([^}]*)\}/s)?.[1] ?? '';

    expect(faceRule).toMatch(/flex:\s*1\s+1\s+0;/);
    expect(faceRule).toMatch(/height:\s*auto;/);
    expect(faceRule).toMatch(/min-height:\s*0;/);
  });

  test('owns projected native-card geometry through CSS after Comfy rewrites inline transforms', () => {
    ensureCubeSurfaceStyles(document);

    const css = document.getElementById('sugarcubes-cube-surface-styles')?.textContent ?? '';
    expect(css).toMatch(
      /\.sugarcubes-native-node-card\s+\.lg-node\[data-cube-face-presentation="true"\]\s*\{[^}]*position:\s*relative\s*!important;[^}]*transform:\s*none\s*!important;[^}]*inset:\s*auto\s*!important;/s,
    );
    expect(css).toMatch(
      /\.sugarcubes-native-node-card\s+\.lg-node\[data-cube-face-presentation="true"\]\s*\{[^}]*width:\s*100%\s*!important;[^}]*height:\s*auto\s*!important;/s,
    );

    const styleElement = document.getElementById(
      'sugarcubes-cube-surface-styles',
    ) as HTMLStyleElement;
    const geometryRule = [...(styleElement.sheet?.cssRules ?? [])]
      .filter((rule): rule is CSSStyleRule => rule instanceof CSSStyleRule)
      .find((rule) => rule.selectorText.includes('[data-cube-face-presentation="true"]'));

    expect(geometryRule?.style.getPropertyValue('transform')).toBe('none');
    expect(geometryRule?.style.getPropertyPriority('transform')).toBe('important');
  });

  test('does not ship renderer-owned reveal popup styles', () => {
    ensureCubeSurfaceStyles(document);

    const css = document.getElementById('sugarcubes-cube-surface-styles')?.textContent ?? '';
    expect(css).not.toContain('.sugarcubes-cube-face__card-menu');
  });

  test('raises the focused native card so Comfy popovers clear sibling cards', () => {
    ensureCubeSurfaceStyles(document);

    const css = document.getElementById('sugarcubes-cube-surface-styles')?.textContent ?? '';
    expect(css).toMatch(/\.sugarcubes-cube-face__node-card:focus-within\s*\{[^}]*z-index:\s*10;/s);
    expect(css).not.toContain('.sugarcubes-native-node-card [role="listbox"]');
  });

  test('keeps activation controls in native header flow without overlay stacking', () => {
    ensureCubeSurfaceStyles(document);

    const css = document.getElementById('sugarcubes-cube-surface-styles')?.textContent ?? '';
    const activationRule =
      css.match(/\.sugarcubes-cube-face__activation\s*\{([^}]*)\}/s)?.[1] ?? '';

    expect(activationRule).toMatch(/position:\s*static;/);
    expect(activationRule).toMatch(/z-index:\s*auto;/);
    expect(activationRule).toMatch(/flex:\s*0\s+0\s+auto;/);
    expect(activationRule).toMatch(/margin-inline-start:\s*auto;/);
    expect(activationRule).not.toMatch(/\btop\s*:/);
    expect(activationRule).not.toMatch(/\bright\s*:/);
    expect(css).not.toMatch(/content:has\([^)]*popper[^)]*\)\s*\{[^}]*z-index:/s);
  });

  test('fits complete preview media inside the rail without cropping or enlarging the Cube', () => {
    ensureCubeSurfaceStyles(document);

    const css = document.getElementById('sugarcubes-cube-surface-styles')?.textContent ?? '';
    const railRule = css.match(/\.sugarcubes-cube-face__preview\s*\{([^}]*)\}/s)?.[1] ?? '';
    const mediaRule = css.match(/\.sugarcubes-cube-face__preview-media\s*\{([^}]*)\}/s)?.[1] ?? '';
    const itemGridRule =
      css.match(/\.sugarcubes-cube-face__preview-items\s*\{([^}]*)\}/s)?.[1] ?? '';
    const figureRule =
      css.match(/\.sugarcubes-cube-face__preview-media figure\s*\{([^}]*)\}/s)?.[1] ?? '';
    const imageRule =
      css.match(/\.sugarcubes-cube-face__preview-media img\s*\{([^}]*)\}/s)?.[1] ?? '';

    expect(railRule).toMatch(/position:\s*relative;/);
    expect(railRule).toMatch(/align-self:\s*stretch;/);
    expect(railRule).toMatch(/height:\s*auto;/);
    expect(railRule).toMatch(/min-height:\s*0;/);
    expect(railRule).toMatch(/overflow:\s*hidden;/);
    expect(railRule).toMatch(/--sugarcubes-cube-preview-edge-inset:\s*1\.125rem;/);
    expect(mediaRule).toMatch(/position:\s*absolute;/);
    expect(mediaRule).toMatch(/inset:\s*0\s+var\(--sugarcubes-cube-preview-edge-inset\);/);
    expect(mediaRule).toMatch(/height:\s*100%;/);
    expect(mediaRule).toMatch(/min-height:\s*0;/);
    expect(mediaRule).toMatch(/overflow:\s*hidden;/);
    expect(itemGridRule).toMatch(/display:\s*grid;/);
    expect(itemGridRule).toMatch(/flex:\s*1\s+1\s+0;/);
    expect(itemGridRule).toMatch(/min-width:\s*0;/);
    expect(itemGridRule).toMatch(/min-height:\s*0;/);
    expect(figureRule).toMatch(/width:\s*100%;/);
    expect(figureRule).toMatch(/height:\s*100%;/);
    expect(figureRule).toMatch(/min-height:\s*0;/);
    expect(imageRule).toMatch(/height:\s*100%\s*!important;/);
    expect(imageRule).toMatch(/min-height:\s*0;/);
    expect(imageRule).toMatch(/max-height:\s*100%\s*!important;/);
    expect(imageRule).not.toMatch(/aspect-ratio:/);
    expect(imageRule).not.toMatch(/background:/);
    expect(imageRule).toMatch(/object-fit:\s*contain;/);
    expect(imageRule).toMatch(/object-position:\s*center top;/);
    expect(imageRule).toMatch(/border-radius:\s*0;/);
    expect(imageRule).toMatch(/opacity:\s*1\s*!important;/);
    expect(imageRule).toMatch(/visibility:\s*visible\s*!important;/);
    expect(imageRule).toMatch(/z-index:\s*1;/);
    const contentRule =
      css.match(
        /\.sugarcubes-cube-face__content\s*\{\s*box-sizing:\s*border-box;([^}]*)\}/s,
      )?.[1] ?? '';
    expect(contentRule).toMatch(/gap:\s*0\.5rem;/);
    expect(contentRule).toMatch(/padding:\s*[\s\S]*?0\.5rem/);
    expect(css).not.toMatch(/data-preview-layout="rail"\][\s\S]*?transform:/);
  });

  test('stretches output sections into equal full-width horizontal bands', () => {
    ensureCubeSurfaceStyles(document);

    const css = document.getElementById('sugarcubes-cube-surface-styles')?.textContent ?? '';
    const outputGridRule =
      css.match(/\.sugarcubes-cube-face__preview-outputs\s*\{([^}]*)\}/s)?.[1] ?? '';

    expect(outputGridRule).toMatch(/grid-template-columns:\s*minmax\(0,\s*1fr\);/);
    expect(outputGridRule).toMatch(/align-items:\s*stretch;/);
    expect(outputGridRule).toMatch(/flex:\s*1\s+1\s+0;/);
    expect(css).toMatch(
      /\.sugarcubes-cube-face__preview-output\s*\{[^}]*gap:\s*var\(--sugarcubes-cube-preview-row-gap,\s*0\.75rem\);/s,
    );
    expect(css).not.toContain('sugarcubes-cube-face__preview-internal');
    expect(css).not.toContain('internal-primary');
    expect(css).toMatch(
      /\.sugarcubes-cube-face__preview-output\s*\+\s*\.sugarcubes-cube-face__preview-output\s*\{[^}]*border-top:/s,
    );
  });
});
