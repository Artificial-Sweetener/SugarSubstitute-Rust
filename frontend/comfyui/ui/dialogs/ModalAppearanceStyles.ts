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
/** Style shared SugarCubes dialog chrome and native form controls with Fluent tokens. */
const STYLE_ID = 'sugarcubes-modal-appearance';
const scope = '[data-sc-modal-appearance]:not([data-sc-modal-appearance="comfy"])';
const comfy = '[data-sc-modal-appearance="comfy"]';
const styles = `
${comfy} .sugarcubes-modal {
  box-sizing:border-box; padding:0; border:1px solid var(--interface-stroke,#444);
  border-radius:12px; background:var(--comfy-menu-bg,#202020); color:var(--fg-color,#eee);
  font:14px/1.4 var(--font-family,"Inter",sans-serif); box-shadow:0 16px 48px #0004;
}
${comfy} .p-dialog-header { display:flex; align-items:flex-start; justify-content:space-between; gap:16px; padding:24px 24px 16px; }
${comfy} .p-dialog-title { min-width:0; flex:1; }
${comfy} .p-dialog-footer { padding:16px 24px 24px; }
${comfy} .p-button {
  display:inline-flex; align-items:center; justify-content:center; min-height:32px; padding:5px 12px; margin:0;
  border:1px solid var(--interface-stroke,#444); border-radius:6px;
  color:var(--fg-color,#eee); background:var(--secondary-background,#303030); font:inherit; cursor:pointer;
}
${comfy} .p-button:hover { background:var(--secondary-background-hover,#393939); }
${comfy} .p-button:focus-visible { outline:2px solid var(--p-primary-color,#60cdff); outline-offset:2px; }
${comfy} .sugarcubes-modal__footer-actions .p-button:not(.p-button-secondary):not(:disabled) {
  background:var(--p-primary-color,#0078d4); border-color:transparent; color:var(--p-primary-contrast-color,#fff);
}
${comfy} .p-button:disabled { opacity:.45; cursor:default; }
${comfy} .sugarcubes-modal__close { flex:none; position:relative; width:32px; height:32px; padding:0; border-color:transparent; background:transparent; }
${comfy} .sugarcubes-modal__close::before,${comfy} .sugarcubes-modal__close::after { content:''; position:absolute; width:14px; height:1px; background:currentColor; transform:rotate(45deg); }
${comfy} .sugarcubes-modal__close::after { transform:rotate(-45deg); }
${scope} :is(.sugarcubes-modal,.sugarcubes-modal__panel) {
  box-sizing:border-box; min-width:0; padding:0; border:1px solid var(--sc-border);
  border-radius:8px; background:var(--sc-surface-secondary); color:var(--sc-text);
  box-shadow:var(--sc-shadow-large); font-family:var(--sc-font-family); font-size:14px; line-height:1.4; overflow-wrap:anywhere;
}
${scope} :is(.p-dialog-header,.sugarcubes-modal__panel-header) { display:flex; align-items:flex-start; justify-content:space-between; gap:16px; }
${scope} .p-dialog-header { padding:24px 24px 16px; border:0; border-radius:8px 8px 0 0; background:transparent; color:inherit; }
${scope} .p-dialog-title { min-width:0; flex:1; }
${scope} :is(.p-dialog-title,.sugarcubes-modal__panel-title) { font-size:20px; line-height:28px; font-weight:600; }
${scope} .p-dialog-content { box-sizing:border-box; min-width:0; padding:0 24px 24px; background:transparent; color:inherit; }
${scope} .p-dialog-footer { padding:16px 24px 24px; background:transparent; border:0; border-radius:0 0 8px 8px; }
${scope} .sugarcubes-modal__footer-actions { margin-left:auto; justify-content:flex-end; }
${scope} :is(.sugarcubes-modal__field,.sugarcubes-modal__form) { min-width:0; gap:8px; }
${scope} .sugarcubes-modal__form { gap:16px; }
${scope} :is(.sugarcubes-modal__field-label,.sugarcubes-modal__checkbox-label) { font-size:14px; font-weight:400; }
${scope} :is(.sugarcubes-modal__text-input,.sugarcubes-modal__input-group) {
  box-sizing:border-box; min-width:0; max-width:100%; min-height:32px; margin:0; padding:5px 10px;
  border:1px solid var(--sc-border); border-bottom-color:var(--sc-border-accessible);
  border-radius:4px; background:var(--sc-surface); color:var(--sc-text);
  font-family:var(--sc-font-family); font-size:14px; line-height:20px; outline:0;
}
${scope} .sugarcubes-modal__text-input:hover { border-color:var(--sc-border-hover); border-bottom-color:var(--sc-border-accessible-hover); }
${scope} .sugarcubes-modal__text-input:focus { border-bottom-color:var(--sc-brand-border); box-shadow:inset 0 -1px var(--sc-brand-border); }
${scope} .sugarcubes-modal__input-group { padding:0; }
${scope} .sugarcubes-modal__input-group:focus-within { border-bottom-color:var(--sc-brand-border); box-shadow:inset 0 -1px var(--sc-brand-border); }
${scope} .sugarcubes-modal__text-input:disabled { background:var(--sc-surface-disabled); color:var(--sc-text-disabled); border-color:var(--sc-border-disabled); }
${scope} .sugarcubes-modal__text-input::placeholder { color:var(--sc-text-subtle); }
${scope} textarea.sugarcubes-modal__text-input { resize:vertical; }
${scope} select.sugarcubes-modal__text-input { cursor:pointer; }
${scope} :is(input[type="checkbox"],input[type="radio"]) { width:20px; height:20px; margin:0; accent-color:var(--sc-brand-fill); flex:none; }
${scope} .p-button {
  display:inline-flex; box-sizing:border-box; align-items:center; justify-content:center; gap:8px;
  min-width:0; min-height:32px; margin:0; padding:5px 12px; border:1px solid var(--sc-border);
  border-bottom-color:var(--sc-border-accessible); border-radius:4px;
  background:var(--sc-surface); color:var(--sc-text);
  font-family:var(--sc-font-family); font-size:14px; line-height:20px; cursor:pointer; text-decoration:none; white-space:nowrap;
}
${scope} .p-button[hidden] { display:none; }
${scope} .p-button:hover { background:var(--sc-surface-hover); }
${scope} .p-button:active { background:var(--sc-surface-pressed); }
${scope} .p-button:focus-visible { outline:2px solid var(--sc-focus-ring); outline-offset:2px; }
${scope} :is(.sugarcubes-modal__footer-actions .p-button:not(.p-button-secondary),.p-button-primary):not(:disabled) {
  background:var(--sc-brand-fill); color:var(--sc-text-on-accent); border-color:transparent;
}
${scope} :is(.sugarcubes-modal__footer-actions .p-button:not(.p-button-secondary),.p-button-primary):not(:disabled):hover { background:var(--sc-brand-fill-hover); }
${scope} :is(.sugarcubes-modal__footer-actions .p-button:not(.p-button-secondary),.p-button-primary):not(:disabled):active { background:var(--sc-brand-fill-pressed); }
${scope} .p-button:disabled { color:var(--sc-text-disabled); background:var(--sc-surface-disabled); border-color:var(--sc-border-disabled); cursor:default; opacity:1; }
${scope} .sugarcubes-modal__close { position:relative; flex:none; width:32px; height:32px; padding:0; border-color:transparent; background:transparent; }
${scope} .sugarcubes-modal__close::before,${scope} .sugarcubes-modal__close::after { content:''; position:absolute; width:14px; height:1px; background:currentColor; transform:rotate(45deg); }
${scope} .sugarcubes-modal__close::after { transform:rotate(-45deg); }
${scope} .sugarcubes-modal__close:hover:not(:disabled) { border-color:var(--sc-border-hover); background:var(--sc-surface-hover); }
${scope} :is(.sugarcubes-modal__error,.sugarcubes-modal__field-error) { color:var(--sc-danger-text); }
${scope} :is(.sugarcubes-modal__field-help,.sugarcubes-modal__description) { color:var(--sc-text-secondary); }
${scope} :is(.sugarcubes-selection-dialog__option,.sugarcubes-default-review__choice,.sugarcubes-create-cube__preview) { background:var(--sc-surface-tertiary); border-color:var(--sc-border); }
`;

/** Inject one document-level stylesheet for every shared modal consumer. */
export function injectModalAppearanceStyles(document: Document): void {
  if (!document.head || document.getElementById(STYLE_ID)) return;
  const style = document.createElement('style');
  style.id = STYLE_ID;
  style.textContent = styles;
  document.head.append(style);
}
