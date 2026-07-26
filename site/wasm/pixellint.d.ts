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

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly rulepacks: () => [number, number, number];
    readonly validate: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number];
    readonly validate_url: (a: number, b: number) => [number, number, number];
    readonly vendor_for_host: (a: number, b: number) => [number, number, number];
    readonly vendors: () => [number, number, number];
    readonly version: () => [number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
