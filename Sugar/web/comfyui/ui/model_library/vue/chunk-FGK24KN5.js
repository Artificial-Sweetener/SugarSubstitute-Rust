import {
  defineComponent,
  h
} from "./chunk-OJTDXCRK.js";

// frontend/comfyui/ui/model_library/vue/ModelBlankCard.ts
var eyeOffIcon = new URL("../../../../icons/model/eye-off.svg", import.meta.url).href;
var FILTERED_THUMBNAIL_REASON = "Hidden by your thumbnail safety setting";
var ModelBlankCard = defineComponent({
  name: "SugarCubesModelBlankCard",
  props: {
    state: { type: String, required: true }
  },
  setup(props) {
    return () => {
      if (props.state === "pending" || props.state === "retrying")
        return h(
          "span",
          {
            class: "sc-model-blank sc-model-blank-flash",
            role: "status",
            "aria-label": props.state === "retrying" ? "Retrying" : "Loading"
          },
          props.state === "retrying" ? h("span", { class: "sc-model-blank-text" }, "Retrying\u2026") : []
        );
      if (props.state === "filtered")
        return h(
          "span",
          { class: "sc-model-blank", title: FILTERED_THUMBNAIL_REASON },
          h("span", {
            class: "sc-model-blank-icon",
            role: "img",
            "aria-label": FILTERED_THUMBNAIL_REASON,
            style: { "--sc-model-blank-icon": `url("${eyeOffIcon}")` }
          })
        );
      return h(
        "span",
        { class: "sc-model-blank" },
        h(
          "span",
          { class: "sc-model-blank-text" },
          props.state === "not_found" ? "Not Found" : "No Thumbnail"
        )
      );
    };
  }
});

export {
  FILTERED_THUMBNAIL_REASON,
  ModelBlankCard
};
