export type ArtifactKind =
  | "url"
  | "html"
  | "js"
  | "gtm"
  | "request"
  | "vast"
  | "postback"
  | "unknown";

export type ExpansionState = "unknown" | "template" | "fired";

export type Severity = "error" | "warning" | "info";

export type EvidenceLevel =
  | "normative"
  | "official_vendor"
  | "official_template"
  | "ecosystem_reference"
  | "heuristic";

export interface RuleSource {
  level: EvidenceLevel;
  name: string;
  reference: string | null;
}

export interface ViolationTarget {
  component: string;
  name: string | null;
  value: string | null;
  start: number;
  end: number;
}

export interface Violation {
  code: string;
  message: string;
  severity: Severity;
  field: string | null;
  fix_hint: string | null;
  source: RuleSource;
  targets?: ViolationTarget[];
}

export interface ValidationReport {
  plugin_id: string;
  detected_vendor: string | null;
  violations: Violation[];
}

export interface ValidationSummary {
  reports: ValidationReport[];
}

export interface RulePackMetadata {
  id: string;
  display_name: string;
  version: string;
  description: string;
  source_level: EvidenceLevel;
  vendor: string | null;
}

export interface VendorEntry {
  vendor: string;
  display_name: string;
  category: string;
  hosts: string[];
  rulepack?: string | null;
}

export interface ValidateOptions {
  /** Artifact kind. Defaults to "url". */
  kind?: ArtifactKind;
  /** Whether macros are still unexpanded. Defaults to "unknown". */
  state?: ExpansionState;
  /** Vendor the caller believes the artifact belongs to. */
  vendor?: string;
}

/** Validate a measurement artifact against every applicable rulepack. */
export function validate(artifact: string, options?: ValidateOptions): ValidationSummary;

/** True when no error-severity finding is present. */
export function isOk(summary: ValidationSummary): boolean;

/** Rulepacks this build ships, with their evidence levels. */
export function rulepacks(): RulePackMetadata[];

/** The vendor endpoint directory. */
export function vendors(): VendorEntry[];

/** Attribute a host to a vendor, or null when the host is unknown. */
export function vendorForHost(host: string): VendorEntry | null;

/** The pixellint-core version this build wraps. */
export function version(): string;
