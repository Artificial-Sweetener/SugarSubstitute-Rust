// integrations/shared/SessionNoticeValidation.generated.ts
function isRecord(value) {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
function isSessionNoticeNotSavedMessage(value) {
  return isRecord(value) && Object.keys(value).length === 7 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.session-notice" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "not-saved" && Object.hasOwn(value, "notice") && typeof value["notice"] === "number" && Number.isSafeInteger(value["notice"]) && value["notice"] >= 1 && value["notice"] <= 9007199254740991 && Object.hasOwn(value, "workspace") && typeof value["workspace"] === "boolean" && Object.hasOwn(value, "inputs") && typeof value["inputs"] === "boolean";
}
function isSessionNoticeNoneMessage(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.session-notice" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "none";
}
function isSessionNoticeHostMessage(value) {
  return isSessionNoticeNotSavedMessage(value) || isSessionNoticeNoneMessage(value);
}

// integrations/shared/SessionNoticeV2Validation.generated.ts
function isRecord2(value) {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
function isSessionNoticeTone(value) {
  return value === "info" || value === "warn" || value === "error";
}
function isSessionNoticeNotSavedMessage2(value) {
  return isRecord2(value) && Object.keys(value).length === 7 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.session-notice" && Object.hasOwn(value, "version") && value["version"] === 2 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "not-saved" && Object.hasOwn(value, "notice") && typeof value["notice"] === "number" && Number.isSafeInteger(value["notice"]) && value["notice"] >= 1 && value["notice"] <= 9007199254740991 && Object.hasOwn(value, "workspace") && typeof value["workspace"] === "boolean" && Object.hasOwn(value, "inputs") && typeof value["inputs"] === "boolean";
}
function isSessionNoticeLineMessage(value) {
  return isRecord2(value) && Object.keys(value).length === 8 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.session-notice" && Object.hasOwn(value, "version") && value["version"] === 2 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "line" && Object.hasOwn(value, "notice") && typeof value["notice"] === "number" && Number.isSafeInteger(value["notice"]) && value["notice"] >= 1 && value["notice"] <= 9007199254740991 && Object.hasOwn(value, "tone") && isSessionNoticeTone(value["tone"]) && Object.hasOwn(value, "text") && typeof value["text"] === "string" && value["text"].length <= 800 && Array.from(value["text"]).length <= 400 && new RegExp("^(?:[^\\uD800-\\uDFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF])*$").test(
    value["text"]
  ) && Object.hasOwn(value, "lasting") && typeof value["lasting"] === "boolean";
}
function isSessionNoticeNoneMessage2(value) {
  return isRecord2(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.session-notice" && Object.hasOwn(value, "version") && value["version"] === 2 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "none";
}
function isSessionNoticeHostMessage2(value) {
  return isSessionNoticeNotSavedMessage2(value) || isSessionNoticeLineMessage(value) || isSessionNoticeNoneMessage2(value);
}
export {
  isSessionNoticeHostMessage2 as isLinedSessionNoticeHostMessage,
  isSessionNoticeHostMessage
};
