import {
  ModelPickerRouteRow
} from "./chunk-3AFG44E5.js";
import {
  ModelPickerWall
} from "./chunk-JHL2BQUZ.js";
import {
  defineComponent,
  h
} from "./chunk-57LGDI7T.js";

// frontend/comfyui/ui/model_library/ModelPickerChoices.ts
function filterModelChoices(items, query, folder) {
  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  return items.filter(
    (item) => (folder === "" || item.folder === folder || item.folder.startsWith(`${folder}/`)) && terms.every(
      (term) => [item.title, item.version, item.value, item.baseModel, ...item.trainedWords].join(" ").toLocaleLowerCase().includes(term)
    )
  );
}
function modelChoiceFolders(items, folder) {
  const prefix = folder ? `${folder}/` : "";
  const children = /* @__PURE__ */ new Set();
  for (const item of items) {
    if (!item.folder.startsWith(prefix)) continue;
    const child = item.folder.slice(prefix.length).split("/")[0];
    if (child) children.add(`${prefix}${child}`);
  }
  return [...children].sort((left, right) => left.localeCompare(right));
}

// frontend/comfyui/ui/model_library/vue/ModelPickerPopupContent.ts
var ModelPickerPopupContent = defineComponent({
  name: "SugarCubesModelPickerPopupContent",
  props: {
    items: { type: Array, required: true },
    visible: { type: Array, required: true },
    folder: { type: String, required: true },
    selected: { type: String, required: true },
    active: { type: Number, required: true },
    optionPrefix: { type: String, required: true },
    updateOffers: { type: Array, default: () => [] },
    compatibilityLabel: { type: String, default: "" },
    unfiltered: { type: Boolean, default: false }
  },
  emits: {
    toggleFilter: () => true,
    navigate: (_path) => true,
    select: (_value) => true,
    highlight: (_index) => true,
    geometry: (_width) => true,
    context: (_item, _event) => true,
    family: (_item) => true,
    updateContext: (_item, _event) => true
  },
  setup(props, { emit }) {
    const route = (label, path, child = false) => h(
      "button",
      {
        type: "button",
        class: child ? "sc-model-child-folder" : void 0,
        onClick: () => emit("navigate", path)
      },
      label
    );
    const filter = () => props.compatibilityLabel ? h(
      "button",
      {
        type: "button",
        class: "sc-model-filter-toggle",
        "aria-label": props.unfiltered ? `Show only ${props.compatibilityLabel}` : "Show all",
        "aria-pressed": props.unfiltered,
        title: props.unfiltered ? `Show only ${props.compatibilityLabel}` : "Show all models",
        onClick: () => emit("toggleFilter")
      },
      [
        h("span", { class: "sc-model-filter-icon", "aria-hidden": true }),
        props.unfiltered ? `Show only ${props.compatibilityLabel}` : "Show all"
      ]
    ) : null;
    return () => {
      const children = modelChoiceFolders(props.items, props.folder);
      return [
        props.folder || children.length || props.compatibilityLabel ? h("nav", { class: "sc-model-routes", "aria-label": "Model folders" }, [
          ...props.folder ? [
            h(ModelPickerRouteRow, { rowClass: "sc-model-breadcrumbs" }, () => [
              route("All", ""),
              ...props.folder.split("/").filter(Boolean).flatMap((part, index, parts) => [
                h(
                  "span",
                  { class: "sc-model-route-separator", "aria-hidden": true },
                  "\u203A"
                ),
                route(part, parts.slice(0, index + 1).join("/"))
              ]),
              children.length ? null : filter()
            ])
          ] : [],
          ...children.length || !props.folder && props.compatibilityLabel ? [
            h(ModelPickerRouteRow, { rowClass: "sc-model-folder-strip" }, () => [
              ...children.map((path) => route(path.split("/").at(-1) ?? path, path, true)),
              filter()
            ])
          ] : []
        ]) : null,
        props.visible.length ? h(ModelPickerWall, {
          items: props.visible,
          selected: props.selected,
          active: props.active,
          optionPrefix: props.optionPrefix,
          updateOffers: props.updateOffers,
          onActivate: (value) => emit("select", value),
          onHighlight: (index) => emit("highlight", index),
          onGeometry: (width) => emit("geometry", width),
          onContext: (item, event) => emit("context", item, event),
          onFamily: (item) => emit("family", item),
          onUpdateContext: (item, event) => emit("updateContext", item, event)
        }) : h("div", { class: "sc-model-empty" }, "No matching models")
      ];
    };
  }
});

export {
  filterModelChoices,
  ModelPickerPopupContent
};
