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
/** Render compact, theme-aware Cube model titles with safe DOM construction. */
const STYLE_ID = 'sugarcubes-model-pill-styles';
/** Create one accessible model/title composition without parsing dynamic markup. */
export function createCubeModelTitleElement(documentRef, presentation, options = {}) {
    ensureCubeModelPillStyles(documentRef);
    const root = documentRef.createElement('span');
    root.className = ['sugarcubes-model-title', options.className ?? ''].filter(Boolean).join(' ');
    root.setAttribute('aria-label', presentation.accessibleText);
    if (options.punchoutColor) {
        root.style.setProperty('--sugarcubes-model-pill-punchout', options.punchoutColor);
    }
    if (presentation.usesModelPill) {
        root.append(createModelPill(documentRef, presentation.modelText));
    }
    root.append(createTextPart(documentRef, 'name', presentation.nameText));
    if (presentation.suffixText) {
        root.append(createTextPart(documentRef, 'suffix', presentation.suffixText));
    }
    return root;
}
/** Replace one owned title host only when its semantic presentation changed. */
export function presentCubeModelTitle(host, presentation, options = {}) {
    const signature = JSON.stringify(presentation);
    if (host.dataset.sugarcubesModelTitleSignature === signature)
        return false;
    host.replaceChildren(createCubeModelTitleElement(host.ownerDocument, presentation, options));
    host.dataset.sugarcubesModelTitleSignature = signature;
    return true;
}
/** Create the inverse model label inside its inherited text-color fill. */
function createModelPill(documentRef, modelText) {
    const pill = documentRef.createElement('span');
    pill.className = 'sugarcubes-model-title__pill';
    pill.dataset.sugarcubesModelPill = '';
    pill.setAttribute('aria-hidden', 'true');
    const label = documentRef.createElement('span');
    label.className = 'sugarcubes-model-title__pill-label';
    label.textContent = modelText;
    pill.append(label);
    return pill;
}
/** Create one literal visible title fragment hidden behind the root accessible name. */
function createTextPart(documentRef, kind, text) {
    const part = documentRef.createElement('span');
    part.className = `sugarcubes-model-title__${kind}`;
    part.setAttribute('aria-hidden', 'true');
    part.textContent = text;
    return part;
}
/** Inject the one shared compact geometry and Comfy-token color contract. */
function ensureCubeModelPillStyles(documentRef) {
    if (documentRef.getElementById(STYLE_ID))
        return;
    const style = documentRef.createElement('style');
    style.id = STYLE_ID;
    style.textContent = `
    .sugarcubes-model-title {
      display: inline-flex;
      align-items: center;
      min-width: 0;
      max-width: 100%;
      gap: 0.3em;
      vertical-align: middle;
      white-space: nowrap;
    }
    .sugarcubes-model-title__pill {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      box-sizing: border-box;
      flex: 0 0 auto;
      max-width: 45%;
      padding: 0.08em 0.38em;
      border-radius: 999px;
      background: currentColor;
      font-size: 0.78em;
      font-weight: 700;
      line-height: 1;
      vertical-align: middle;
    }
    .sugarcubes-model-title__pill-label {
      overflow: hidden;
      color: var(
        --sugarcubes-model-pill-punchout,
        var(--comfy-menu-bg, var(--p-content-background, #171b20))
      );
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .sugarcubes-model-title__name,
    .sugarcubes-model-title__suffix {
      min-width: 0;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .sugarcubes-model-title__suffix {
      color: color-mix(in srgb, currentColor 82%, transparent);
    }
  `;
    (documentRef.head ?? documentRef.documentElement).append(style);
}
