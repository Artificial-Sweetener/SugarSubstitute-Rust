// integrations/shared/ReleaseNotesValidation.generated.ts
function isRecord(value) {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
function isReleaseNotesFailure(value) {
  return value === "unavailable" || value === "not-published";
}
function isReleaseNotesRelease(value) {
  return isRecord(value) && Object.keys(value).length === 2 && Object.hasOwn(value, "version") && typeof value["version"] === "string" && value["version"].length <= 64 && new RegExp("^[a-z0-9.-]{1,64}$").test(value["version"]) && Object.hasOwn(value, "notes") && typeof value["notes"] === "string" && value["notes"].length <= 262144;
}
function isReleaseNotesStateMessage(value) {
  return isRecord(value) && Object.keys(value).length === 7 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.release-notes" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "state" && Object.hasOwn(value, "revision") && typeof value["revision"] === "number" && Number.isSafeInteger(value["revision"]) && value["revision"] >= 1 && value["revision"] <= 9007199254740991 && Object.hasOwn(value, "product") && typeof value["product"] === "string" && value["product"].length <= 64 && Object.hasOwn(value, "currentVersion") && typeof value["currentVersion"] === "string" && value["currentVersion"].length <= 64 && new RegExp("^[a-z0-9.-]{1,64}$").test(value["currentVersion"]);
}
function isReleaseNotesOpenMessage(value) {
  return isRecord(value) && Object.keys(value).length === 4 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.release-notes" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "open";
}
function isReleaseNotesNotesMessage(value) {
  return isRecord(value) && Object.keys(value).length === 7 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.release-notes" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "notes" && Object.hasOwn(value, "request") && typeof value["request"] === "string" && value["request"].length <= 48 && new RegExp("^[a-z0-9.-]{1,48}$").test(value["request"]) && Object.hasOwn(value, "releases") && Array.isArray(value["releases"]) && value["releases"].length >= 1 && value["releases"].length <= 64 && value["releases"].every((item) => isReleaseNotesRelease(item)) && Object.hasOwn(value, "omitted") && typeof value["omitted"] === "number" && Number.isSafeInteger(value["omitted"]) && value["omitted"] >= 0 && value["omitted"] <= 9007199254740991;
}
function isReleaseNotesFailedMessage(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.release-notes" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "failed" && Object.hasOwn(value, "request") && typeof value["request"] === "string" && value["request"].length <= 48 && new RegExp("^[a-z0-9.-]{1,48}$").test(value["request"]) && Object.hasOwn(value, "reason") && isReleaseNotesFailure(value["reason"]);
}
function isReleaseNotesHostMessage(value) {
  return isReleaseNotesStateMessage(value) || isReleaseNotesOpenMessage(value) || isReleaseNotesNotesMessage(value) || isReleaseNotesFailedMessage(value);
}
export {
  isReleaseNotesHostMessage
};
