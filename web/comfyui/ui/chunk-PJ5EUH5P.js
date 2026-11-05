import {
  CanvasAnchoredPicker
} from "./chunk-MMIFSN3Z.js";
import {
  defineComponent,
  h
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasRoutePicker.ts
var ROUTES = ["input", "output"];
var CanvasRoutePicker = defineComponent({
  name: "CanvasRoutePicker",
  props: {
    state: { type: Object, required: true },
    choose: { type: Function, required: true }
  },
  /** Forward only a different, declared route. */
  setup(props) {
    function choose(value) {
      const route = ROUTES.find((candidate) => candidate === value);
      if (route && route !== props.state.route) props.choose(route);
    }
    return () => props.state.inputAvailable ? h(
      "span",
      {
        class: "sc-output-route",
        "data-sc-fixed-navigation": "true",
        "data-route": props.state.route
      },
      [
        h(CanvasAnchoredPicker, {
          label: props.state.labels.group,
          value: props.state.route,
          items: ROUTES.map((route) => ({ value: route, label: props.state.labels[route] })),
          stable: true,
          choose
        })
      ]
    ) : null;
  }
});

export {
  CanvasRoutePicker
};
