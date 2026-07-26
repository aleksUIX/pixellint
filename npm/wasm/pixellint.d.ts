/* tslint:disable */
/* eslint-disable */

/**
 * Every rulepack the engine ships, with its evidence level.
 */
export function rulepacks(): any;

/**
 * Validates one artifact and returns the full [`ValidationSummary`] as a plain
 * JS object.
 */
export function validate(artifact_kind: string, artifact: string, expansion_state?: string | null, claimed_vendor?: string | null): any;

/**
 * Validates a URL artifact with default options, the common case.
 */
export function validate_url(artifact: string): any;

/**
 * Attributes a host to a vendor, or returns `null` when the host is unknown.
 */
export function vendor_for_host(host: string): any;

/**
 * The vendor endpoint directory.
 */
export function vendors(): any;

/**
 * The `pixellint-core` version this build wraps.
 */
export function version(): string;
