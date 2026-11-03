import {
  defineComponent,
  h
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasRouteTabs.ts
var ROUTES = ["input", "output"];
var CanvasRouteTabs = defineComponent({
  name: "CanvasRouteTabs",
  props: {
    state: { type: Object, required: true },
    choose: { type: Function, required: true }
  },
  /** Follow tab-list keyboard conventions over the enabled routes. */
  setup(props) {
    function navigate(event) {
      if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
      if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
      event.preventDefault();
      event.stopPropagation();
      const next = event.key === "Home" || event.key !== "End" && props.state.route === "output" ? "input" : "output";
      if (next === "input" && !props.state.inputAvailable) return;
      const target = event.currentTarget;
      if (target instanceof HTMLElement)
        target.querySelector(`[data-route="${next}"]`)?.focus();
      props.choose(next);
    }
    return () => h("div", { class: "sc-output-tabstrip sc-output-route" }, [
      h(
        "nav",
        {
          class: "sc-output-tabs",
          role: "tablist",
          "aria-label": props.state.labels.group,
          onKeydown: navigate
        },
        ROUTES.map((route) => {
          const selected = props.state.route === route, disabled = route === "input" && !props.state.inputAvailable;
          return h(
            "button",
            {
              key: route,
              type: "button",
              role: "tab",
              "data-route": route,
              "aria-selected": selected,
              tabIndex: selected ? 0 : -1,
              disabled,
              title: disabled ? props.state.reason : void 0,
              onClick: () => {
                if (!disabled && !selected) props.choose(route);
              }
            },
            props.state.labels[route]
          );
        })
      )
    ]);
  }
});

export {
  CanvasRouteTabs
};
