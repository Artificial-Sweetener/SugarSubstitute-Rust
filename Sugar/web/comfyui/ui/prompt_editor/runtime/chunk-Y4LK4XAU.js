import {
  featureDefinitions
} from "./chunk-HOER6HVA.js";

// frontend/comfyui/ui/prompt_editor/runtime/PromptDocumentMode.ts
function promptDocumentKind(document) {
  return document === "tag-list" ? "prompt" : document;
}
var TAG_LIST_PREFERENCES = (preferences) => ({
  ...preferences,
  allowed: Object.fromEntries(featureDefinitions.map(({ id }) => [id, false]))
});
function promptDocumentPreferences(document, preferences) {
  return document === "tag-list" ? TAG_LIST_PREFERENCES(preferences) : preferences;
}
var PromptLineGutter = class {
  #strip;
  #frame;
  #measure;
  #digits = 0;
  #inset = 0;
  /** Draw the strip over `frame`, the element the editor's scroller fills. */
  constructor(frame) {
    const document = frame.ownerDocument;
    this.#frame = frame;
    this.#strip = document.createElement("div");
    this.#strip.className = "sc-prompt-line-gutter";
    this.#strip.setAttribute("aria-hidden", "true");
    this.#measure = document.createElement("span");
    this.#measure.className = "sc-prompt-line-gutter__measure";
    this.#strip.append(this.#measure);
    frame.append(this.#strip);
  }
  /**
   * The strip's width for `lineCount` lines: numbers are padded to at least two digits, as the
   * Python editor padded them, so the strip widens only at 100 lines and each power of ten.
   */
  inset(lineCount) {
    const digits = Math.max(2, String(Math.max(1, lineCount)).length);
    if (digits !== this.#digits) {
      this.#digits = digits;
      this.#measure.textContent = "0".repeat(digits);
      this.#inset = Math.ceil(this.#measure.getBoundingClientRect().width) + 12;
      this.#strip.style.width = `${String(this.#inset)}px`;
    }
    return this.#inset;
  }
  /** Redraw the numbers of the lines `snapshot` shows. */
  paint(snapshot, scroller) {
    if (snapshot.status !== "ready" || !scroller) return;
    const offset = scroller.getBoundingClientRect().top - this.#frame.getBoundingClientRect().top;
    const rows = snapshot.lines.map((line) => {
      const row = this.#frame.ownerDocument.createElement("div");
      row.className = "sc-prompt-line-gutter__row";
      if (line.lineIndex % 2 === 1) row.classList.add("is-alternate");
      if (line.lineIndex === snapshot.currentLineIndex && snapshot.focusActive)
        row.classList.add("is-current");
      row.style.top = `${String(offset + line.top / snapshot.scaleY)}px`;
      row.style.height = `${String(line.height / snapshot.scaleY)}px`;
      row.textContent = String(line.lineIndex + 1).padStart(this.#digits, "0");
      return row;
    });
    this.#strip.replaceChildren(this.#measure, ...rows);
  }
  dispose() {
    this.#strip.remove();
  }
};
var promptLineGutterCss = [
  ".sc-prompt-document {",
  "  --sugarprompt-line-alternate: light-dark(#0000000c, #ffffff10);",
  "  --sugarprompt-line-active: color-mix(in srgb, var(--sc-accent-indicator, var(--sc-brand-border, Highlight)) 15%, transparent);",
  "}",
  ".sc-prompt-line-gutter {",
  "  position: absolute;",
  "  inset: 0 auto 0 0;",
  "  overflow: hidden;",
  "  pointer-events: none;",
  "  border-right: 1px solid light-dark(#0000001a, #ffffff1a);",
  '  font: 12px/1 ui-monospace, "Cascadia Mono", Consolas, monospace;',
  "  color: light-dark(rgb(118 118 118), rgb(176 176 176));",
  "  z-index: 1;",
  "}",
  ".sc-prompt-line-gutter__measure { position: absolute; visibility: hidden; }",
  ".sc-prompt-line-gutter__row {",
  "  position: absolute;",
  "  left: 0;",
  "  right: 0;",
  "  display: flex;",
  "  justify-content: center;",
  "  padding-top: 3px;",
  "  box-sizing: border-box;",
  "}",
  ".sc-prompt-line-gutter__row.is-alternate { background: var(--sugarprompt-line-alternate); }",
  ".sc-prompt-line-gutter__row.is-current {",
  "  background: var(--sugarprompt-line-active);",
  "  color: var(--sc-accent-indicator, var(--sc-brand-border, Highlight));",
  "}"
].join("\n");

export {
  promptDocumentKind,
  promptDocumentPreferences,
  PromptLineGutter,
  promptLineGutterCss
};
