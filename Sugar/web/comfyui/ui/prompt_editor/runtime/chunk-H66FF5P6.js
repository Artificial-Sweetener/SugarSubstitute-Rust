// frontend/comfyui/ui/prompt_editor/runtime/SugarPromptMenu.ts
import { ContextMenu } from "../../menus/ContextMenu.js";
var SugarPromptMenu = class {
  /**
   * Bind focus return and observable command failures to one editor lifetime. `loraMenu` gives
   * the rows about the LoRA whose banner the menu opened on, the same ones every model surface
   * shows. `presented` hears each time the menu opens and closes.
   */
  constructor(root, report, loraMenu, presented = () => void 0) {
    this.loraMenu = loraMenu;
    this.presented = presented;
    this.#root = root;
    this.#report = report;
    this.#menu = new ContextMenu(root.ownerDocument);
  }
  #menu;
  #root;
  #report;
  #publication = null;
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
    const lora = menu.actions?.kind === "inline" && menu.actions.resource.resolution === "found" ? this.loraMenu(menu.actions.resource.model.backendValue) : [];
    const own = entries(menu.items);
    this.#menu.open({
      label: "Prompt actions",
      entries: lora.length ? [...own, { label: "", separator: true }, ...lora] : own,
      anchor: { kind: "pointer", x: menu.anchor.left, y: menu.anchor.bottom },
      focusReturn: this.#root.querySelector(".cm-content"),
      returnFocusOnSelect: false,
      attributes: { "data-sc-prompt-menu": "" },
      closed: () => {
        if (this.#publication === publication) this.presented(false);
        queueMicrotask(() => {
          if (this.#publication === publication) dismiss();
        });
      }
    });
    this.presented(true);
  }
  /** Clear presentation without retiring a replacement native invocation. */
  hide() {
    if (this.#publication) this.presented(false);
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
