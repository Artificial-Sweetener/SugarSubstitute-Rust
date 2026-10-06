import {
  createModelProviderIcon
} from "./chunk-5HRVOV4C.js";
import {
  defineComponent,
  onBeforeUnmount,
  onMounted,
  watch
} from "./chunk-57LGDI7T.js";

// frontend/comfyui/ui/model_library/vue/ModelContextMenuView.ts
import { ContextMenu } from "../../menus/ContextMenu.js";

// frontend/comfyui/ui/model_library/ModelContextProjection.ts
function projectModelContext(items, command) {
  return items.map((item) => ({
    label: item.label,
    disabled: !item.enabled,
    ...item.separator ? { separator: true } : {},
    ...item.checked === void 0 ? {} : { checked: item.checked },
    ...item.children ? { children: projectModelContext(item.children, command) } : {},
    ...item.providerIcon ? { icon: (document2) => createModelProviderIcon(document2, item.providerIcon ?? "") } : {},
    select: () => {
      if (item.url) window.open(item.url, "_blank", "noopener,noreferrer");
      else command(item);
    }
  }));
}

// frontend/comfyui/ui/model_library/vue/ModelContextMenuView.ts
var ModelContextMenuView = defineComponent({
  name: "SugarCubesModelContextMenuView",
  props: {
    entries: { type: Array, required: true },
    x: { type: Number, required: true },
    y: { type: Number, required: true },
    label: { type: String, default: "Model actions" }
  },
  emits: { close: () => true, action: (_entry) => true },
  setup(props, { emit }) {
    const menu = new ContextMenu(document);
    let disposed = false;
    const open = () => {
      disposed = true;
      menu.close();
      disposed = false;
      menu.open({
        label: props.label,
        entries: projectModelContext(props.entries, (item) => emit("action", item)),
        anchor: { kind: "pointer", x: props.x, y: props.y },
        closed: () => {
          if (!disposed) emit("close");
        }
      });
    };
    onMounted(open);
    watch(() => [props.entries, props.x, props.y], open);
    onBeforeUnmount(() => {
      disposed = true;
      menu.close();
    });
    return () => null;
  }
});

export {
  projectModelContext,
  ModelContextMenuView
};
