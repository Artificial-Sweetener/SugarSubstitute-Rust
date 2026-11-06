import {
  CanvasAnchoredPicker
} from "./chunk-QXRZ6IZZ.js";
import {
  defineComponent,
  h
} from "./chunk-6PAJ5Y37.js";

// frontend/comfyui/ui/output_canvas/presentation/vue/CanvasScenePicker.ts
var ALL = "\0all";
var CanvasScenePicker = defineComponent({
  name: "CanvasScenePicker",
  props: {
    scenes: { type: Array, required: true },
    scene: { type: String, default: null },
    send: { type: Function, required: true }
  },
  /** Translate a picker row into the scene command of the shared view authority. */
  setup(props) {
    function choose(value) {
      if (value === ALL) {
        props.send({ type: "select-scene", scene: null });
        return;
      }
      const scene = props.scenes.find((entry) => entry.key === value);
      if (scene?.available) props.send({ type: "select-scene", scene: scene.key });
    }
    return () => h("span", { class: "sc-output-scene", "data-sc-fixed-navigation": "true" }, [
      h(CanvasAnchoredPicker, {
        label: "Output scene",
        value: props.scenes.some((entry) => entry.key === props.scene) ? props.scene ?? ALL : ALL,
        items: [
          { value: ALL, label: "All" },
          ...props.scenes.map((entry) => ({
            value: entry.key,
            label: entry.title,
            disabled: !entry.available
          }))
        ],
        stable: true,
        choose
      })
    ]);
  }
});

export {
  CanvasScenePicker
};
