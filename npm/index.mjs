/**
 * Pixellint: validator for pixels, postbacks, conversion API payloads, and tracking URLs.
 * ESM entry point backed by the pixellint-core WASM build.
 */

import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const wasm = require("./wasm/pixellint.js");

/**
 * Validate a measurement artifact.
 * @param {string} artifact - The artifact, usually a URL.
 * @param {object} [options]
 * @param {"url"|"html"|"js"|"gtm"|"request"|"vast"|"postback"|"json"|"unknown"} [options.kind] - Artifact kind, default "url".
 * @param {"unknown"|"template"|"fired"} [options.state] - Whether macros are still unexpanded.
 * @param {string} [options.vendor] - Vendor the caller believes the artifact belongs to.
 * @returns {{reports: Array<{plugin_id: string, detected_vendor: string|null, violations: Array<object>}>}}
 */
export function validate(artifact, options = {}) {
  const { kind = "url", state, vendor } = options;
  return wasm.validate(kind, artifact, state, vendor);
}

/** Rulepacks this build ships, with their evidence levels. */
export function rulepacks() {
  return wasm.rulepacks();
}

/** The vendor endpoint directory. */
export function vendors() {
  return wasm.vendors();
}

/**
 * Attribute a host to a vendor.
 * @param {string} host
 * @returns {object|null} The directory entry, or null when the host is unknown.
 */
export function vendorForHost(host) {
  return wasm.vendor_for_host(host);
}

/** The pixellint-core version this build wraps. */
export function version() {
  return wasm.version();
}

/** True when no error-severity finding is present in a summary. */
export function isOk(summary) {
  return summary.reports.every((report) =>
    report.violations.every((violation) => violation.severity !== "error"),
  );
}
