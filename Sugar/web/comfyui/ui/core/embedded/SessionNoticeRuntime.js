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
export {
  isSessionNoticeHostMessage
};
