// frontend/comfyui/ui/prompt_editor/runtime/SugarPromptStyles.ts
var installed = /* @__PURE__ */ new WeakSet();
function installSugarPromptStyles(document) {
  if (installed.has(document)) return;
  const style = document.createElement("style");
  style.dataset.scPromptStyles = "";
  style.textContent = `
.lg-node-widget :has(> .sc-cube-prompt-editor) > label {
  display: none;
}
.sugarprompt-editor {
  min-height: 0;
  position: relative;
  box-sizing: border-box;
}
.sugarprompt-editor .cm-editor.cm-focused { outline: none; }
.sugarprompt-editor .cm-content:focus { outline: none; }
.sugarprompt-editor .cm-content {
  padding:var(--sc-editor-content-padding);
  padding-left:calc(var(--sc-editor-content-padding-left) + var(--sugarprompt-content-inset,0px));
}
.sugarprompt-editor :is(.cm-editor[data-reorder-preview="true"],.sp-reorder-preview) .sp-reorder-chip {
  outline:1px solid var(--sc-accent-indicator,var(--sc-brand-border,Highlight));
  background-color:color-mix(in srgb,var(--sc-accent-indicator,var(--sc-brand-border,Highlight)) 12%,transparent);
  border-radius:9px; box-decoration-break:clone; -webkit-box-decoration-break:clone;
}
.sugarprompt-editor :is(.cm-editor[data-reorder-preview="true"],.sp-reorder-preview) .sp-reorder-chip:is(:hover,.sp-reorder-chip-active) {
  background-color:color-mix(in srgb,var(--sc-accent-indicator,var(--sc-brand-border,Highlight)) 22%,transparent);
}
.sugarprompt-editor .sp-reorder-empty { border:none; outline:none; background:transparent; }
`;
  document.head.append(style);
  installed.add(document);
}

export {
  installSugarPromptStyles
};
