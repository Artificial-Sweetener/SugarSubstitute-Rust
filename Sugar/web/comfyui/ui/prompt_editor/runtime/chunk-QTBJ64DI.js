// frontend/comfyui/ui/prompt_editor/runtime/SugarPromptMenu.ts
import { ContextMenu } from "../../menus/ContextMenu.js";
var SugarPromptMenu = class {
  #menu;
  #root;
  #report;
  #publication = null;
  /** Bind focus return and observable command failures to one editor lifetime. */
  constructor(root, report) {
    this.#root = root;
    this.#report = report;
    this.#menu = new ContextMenu(root.ownerDocument);
  }
  /** Map literal domain entries without changing source or reconstructing commands. */
  show(menu, activate, dismiss) {
    this.hide();
    const publication = {};
    this.#publication = publication;
    const entries = (items) => items.map((item) => {
      if (item.kind === "separator") return { label: "", separator: true };
      if (item.kind === "submenu") return { label: item.label, children: entries(item.children) };
      if (item.kind === "heading") return { label: item.label, section: true };
      if (item.kind === "disabled") return { label: item.label, disabled: true };
      return {
        label: item.label,
        ...item.kind === "rich-rendering" ? { checked: item.checked, selection: "multiple" } : {},
        select: () => {
          void activate(item).catch(this.#report);
        }
      };
    });
    this.#menu.open({
      label: "Prompt actions",
      entries: entries(menu.items),
      anchor: { kind: "pointer", x: menu.anchor.left, y: menu.anchor.bottom },
      focusReturn: this.#root.querySelector(".cm-content"),
      returnFocusOnSelect: false,
      attributes: { "data-sc-prompt-menu": "" },
      closed: () => queueMicrotask(() => {
        if (this.#publication === publication) dismiss();
      })
    });
  }
  /** Clear presentation without retiring a replacement native invocation. */
  hide() {
    this.#publication = null;
    this.#menu.close();
  }
  /** Release the shared menu before the editor's DOM is removed. */
  dispose() {
    this.hide();
  }
};

export {
  SugarPromptMenu
};
