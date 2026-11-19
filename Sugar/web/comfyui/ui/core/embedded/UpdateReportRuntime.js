// integrations/shared/UpdateReportValidation.generated.ts
function isRecord(value) {
  return !!value && typeof value === "object" && !Array.isArray(value);
}
function isUpdateReportStateMessage(value) {
  return isRecord(value) && Object.keys(value).length === 6 && Object.hasOwn(value, "protocol") && value["protocol"] === "sugarcubes.update-report" && Object.hasOwn(value, "version") && value["version"] === 1 && Object.hasOwn(value, "session") && typeof value["session"] === "string" && value["session"].length <= 80 && new RegExp("^[a-zA-Z0-9-]{16,80}$").test(value["session"]) && Object.hasOwn(value, "type") && value["type"] === "state" && Object.hasOwn(value, "revision") && typeof value["revision"] === "number" && Number.isSafeInteger(value["revision"]) && value["revision"] >= 0 && value["revision"] <= 9007199254740991 && Object.hasOwn(value, "available") && typeof value["available"] === "boolean";
}
export {
  isUpdateReportStateMessage
};
