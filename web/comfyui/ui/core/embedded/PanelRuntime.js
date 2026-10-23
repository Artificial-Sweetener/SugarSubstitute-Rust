// integrations/shared/PanelValidation.generated.ts
function isRecord(value) {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
function isPanelPresentation(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "pluginMode") && typeof value["pluginMode"] === "boolean" && Object.hasOwn(value, "visualStyle") && (value["visualStyle"] === "comfy" || value["visualStyle"] === "fluent" || value["visualStyle"] === "spectrum" || value["visualStyle"] === "photopro") && Object.hasOwn(value, "photoProPalette") && (value["photoProPalette"] === "black" || value["photoProPalette"] === "dark-gray" || value["photoProPalette"] === "medium-gray" || value["photoProPalette"] === "light-gray") && Object.hasOwn(value, "accentColor") && (typeof value["accentColor"] === "string" && value["accentColor"].length <= 7 && new RegExp("^#[a-fA-F0-9]{6}$").test(value["accentColor"]) || value["accentColor"] === null) && Object.hasOwn(value, "resizeCornerSize") && typeof value["resizeCornerSize"] === "number" && Number.isSafeInteger(value["resizeCornerSize"]) && value["resizeCornerSize"] >= 0 && value["resizeCornerSize"] <= 64;
}
function isPanelReady(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.panel" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "ready";
}
function isPanelConfigure(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.panel" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "configure" && Object.hasOwn(value, "presentation") && isPanelPresentation(value["presentation"]);
}
function isPanelApplied(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.panel" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "applied";
}
function isPanelPing(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.panel" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "ping";
}
function isPanelPong(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.panel" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "pong" && Object.hasOwn(value, "connected") && typeof value["connected"] === "boolean";
}
function isPanelRejected(value) {
  return isRecord(value) && Object.keys(value).length === 5 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.panel" && Object.hasOwn(value, "version") && value["version"] === 3 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "rejected" && Object.hasOwn(value, "reason") && (value["reason"] === "unsupported-presentation" || value["reason"] === "presentation-failed");
}
function isPanelMessage(value) {
  return isPanelReady(value) || isPanelConfigure(value) || isPanelApplied(value) || isPanelPing(value) || isPanelPong(value) || isPanelRejected(value);
}

// integrations/shared/PanelPresentation.ts
function samePanelPresentation(left, right) {
  if (!left || !right) return left === right;
  return left.pluginMode === right.pluginMode && left.visualStyle === right.visualStyle && left.photoProPalette === right.photoProPalette && left.accentColor === right.accentColor && left.resizeCornerSize === right.resizeCornerSize;
}
function freezePanelPresentation(value) {
  return Object.freeze({ ...value });
}

// integrations/shared/PanelSession.ts
var PanelClientSession = class {
  /** Bind one application-neutral client to an externally verified transport session. */
  constructor(session, port) {
    this.session = session;
    this.port = port;
  }
  #applied = false;
  /** Announce readiness until the native panel confirms presentation configuration. */
  ready() {
    if (!this.#applied) this.#send({ type: "ready" });
  }
  /** Apply validated configuration or answer a heartbeat after presentation is accepted. */
  receive(value) {
    if (!isPanelMessage(value) || value.session !== this.session) return false;
    if (value.type === "configure") {
      try {
        if (!this.port.apply(value.presentation)) {
          this.#send({ type: "rejected", reason: "unsupported-presentation" });
          return true;
        }
        this.#applied = true;
        this.#send({ type: "applied" });
      } catch (error) {
        this.port.report(error);
        this.#send({ type: "rejected", reason: "presentation-failed" });
      }
    } else if (value.type === "ping" && this.#applied)
      this.#send({ type: "pong", connected: this.port.connected() });
    else return false;
    return true;
  }
  /** Attach authoritative client metadata to schema-checked response kinds. */
  #send(message) {
    this.port.send({ protocol: "sugarcubes.panel", version: 3, session: this.session, ...message });
  }
};
export {
  PanelClientSession,
  freezePanelPresentation,
  samePanelPresentation
};
