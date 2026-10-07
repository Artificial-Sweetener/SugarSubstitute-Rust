import {
  ModalDismissal
} from "./chunk-LODHLXUH.js";
import {
  defineComponent,
  h,
  nextTick,
  onMounted,
  ref,
  watchEffect
} from "./chunk-3W5PGMRW.js";

// frontend/comfyui/ui/model_library/vue/CivitaiCredentialLayer.ts
var globeIcon = new URL("../../../../icons/model/globe.svg", import.meta.url).href;
var CivitaiCredentialLayer = defineComponent({
  name: "SugarCubesCivitaiCredentialLayer",
  props: {
    busy: { type: Boolean, required: true },
    error: { type: String, required: true },
    actions: { type: Object, required: true }
  },
  setup(props) {
    const key = ref("");
    const visible = ref(false);
    const input = ref(null);
    const panel = ref(null);
    const dismissal = new ModalDismissal(
      document,
      () => panel.value ?? void 0,
      () => ({}),
      () => props.actions.cancelCredential()
    );
    watchEffect(() => dismissal.setBusy(props.busy));
    onMounted(() => {
      void nextTick(() => input.value?.focus());
    });
    const keyboard = (event) => {
      event.stopPropagation();
      if (event.key === "Escape") {
        dismissal.escape(event);
      }
      if (event.key !== "Tab") return;
      const controls = [
        ...panel.value?.querySelectorAll(
          "a[href],button:not(:disabled),input:not(:disabled)"
        ) ?? []
      ];
      const first = controls[0], last = controls.at(-1);
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last?.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first?.focus();
      }
    };
    return () => h(
      "div",
      {
        class: "sc-model-credential-layer",
        onKeydown: keyboard,
        onPointerdownCapture: (event) => dismissal.press(event),
        onClick: (event) => dismissal.outside(event)
      },
      [
        h(
          "form",
          {
            ref: panel,
            class: "sc-model-credential-panel sugarcubes-modal__panel",
            role: "dialog",
            "aria-modal": "true",
            "aria-label": "CivitAI API key",
            onSubmit: (event) => {
              event.preventDefault();
              if (!props.busy && key.value.trim()) {
                const candidate = key.value;
                key.value = "";
                props.actions.saveCredential(candidate);
              }
            }
          },
          [
            h("div", { class: "sugarcubes-modal__panel-header" }, [
              h(
                "h2",
                { class: "sugarcubes-modal__panel-title" },
                "This model needs a CivitAI API key"
              ),
              h("button", {
                type: "button",
                class: "p-button p-component sugarcubes-modal__close",
                "aria-label": "Close dialog",
                title: "Close",
                disabled: props.busy,
                onClick: () => dismissal.dismiss()
              })
            ]),
            h(
              "p",
              "Sign in to CivitAI or create an account.\nThen create an API key in Account Settings and paste it here."
            ),
            h(
              "a",
              {
                class: "p-button p-button-secondary",
                href: "https://civitai.com/user/account",
                target: "_blank",
                rel: "noopener noreferrer"
              },
              [
                h("span", {
                  class: "sc-model-action-icon",
                  "aria-hidden": "true",
                  style: { "--sc-model-action-icon": `url("${globeIcon}")` }
                }),
                "Open CivitAI account settings"
              ]
            ),
            h("div", { class: "sc-model-credential-input sugarcubes-modal__input-group" }, [
              h("input", {
                ref: input,
                type: visible.value ? "text" : "password",
                placeholder: "Paste CivitAI API key",
                "aria-label": "Paste CivitAI API key",
                autocomplete: "off",
                spellcheck: false,
                disabled: props.busy,
                value: key.value,
                onInput: (event) => {
                  if (event.target instanceof HTMLInputElement) key.value = event.target.value;
                }
              }),
              h(
                "button",
                {
                  type: "button",
                  "aria-label": visible.value ? "Hide API key" : "Show API key",
                  disabled: props.busy,
                  onClick: () => {
                    visible.value = !visible.value;
                    input.value?.focus();
                  }
                },
                [h("span", { class: "sc-model-credential-eye", "aria-hidden": "true" })]
              )
            ]),
            props.error ? h("p", { role: "alert", class: "sc-model-credential-error" }, props.error) : null,
            h("div", { class: "sc-model-credential-actions" }, [
              h(
                "button",
                {
                  type: "button",
                  class: "p-button p-button-secondary",
                  disabled: props.busy,
                  onClick: props.actions.cancelCredential
                },
                "Cancel"
              ),
              h(
                "button",
                {
                  type: "button",
                  class: "p-button p-button-secondary",
                  disabled: props.busy,
                  onClick: props.actions.publicOnly
                },
                "Show models that don't require a key"
              ),
              h(
                "button",
                {
                  type: "submit",
                  class: "p-button p-button-primary",
                  disabled: props.busy || !key.value.trim()
                },
                "Save and continue"
              )
            ])
          ]
        )
      ]
    );
  }
});

export {
  CivitaiCredentialLayer
};
