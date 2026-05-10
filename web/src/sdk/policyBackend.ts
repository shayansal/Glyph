import { applyPatch, proposePatch, validatePatch, type GlyphPatch, type GlyphWorld, type PatchProposal } from "./index";
import wasmModuleUrl from "../wasm/glyphspace_wasm_bg.wasm?url";
import type { CapabilityDefinition } from "./dsl";
import type { PolicyContext } from "./host";

export interface PolicyReport {
  allowed: boolean;
  warnings: string[];
  violations: string[];
}

export interface PolicyBackend {
  readonly kind: "wasm" | "local";
  loadWorld(world: GlyphWorld): Promise<void>;
  validatePatch(world: GlyphWorld, patch: GlyphPatch): Promise<PolicyReport>;
  validateCapabilityInvocation(capability: CapabilityDefinition, context: PolicyContext): Promise<PolicyReport>;
  applyPatch(world: GlyphWorld, patch: GlyphPatch): Promise<GlyphWorld>;
  proposePatch(world: GlyphWorld, request: string): Promise<PatchProposal>;
}

export function localPolicyBackend(): PolicyBackend {
  return {
    kind: "local",
    async loadWorld() {
      return undefined;
    },
    async validatePatch(world, patch) {
      return validatePatch(world, patch);
    },
    async validateCapabilityInvocation(capability, context) {
      const violations = (capability.required_permissions ?? [])
        .filter((permission) => !context.permissions.includes(permission))
        .map((permission) => `missing permission ${permission}`);
      if ((capability.risk === "high" || capability.risk === "critical") && (!capability.requires_confirmation || !capability.audit)) {
        violations.push("high-risk capability requires confirmation and audit");
      }
      return { allowed: violations.length === 0, warnings: [], violations };
    },
    async applyPatch(world, patch) {
      return applyPatch(world, patch);
    },
    async proposePatch(world, request) {
      return proposePatch(world, request);
    },
  };
}

interface WasmEngineModule {
  default?: (moduleOrPath?: unknown) => Promise<unknown>;
  WasmGlyphspaceEngine: new () => {
    load_world(worldJson: string): void;
    validate_patch(patchJson: string): string;
    validate_capability_invocation(capabilityJson: string, policyContextJson: string): string;
    apply_patch(patchJson: string): string;
    propose_patch(request: string): string;
  };
}

export async function wasmPolicyBackend(moduleUrl?: string | { module_or_path: unknown }): Promise<PolicyBackend> {
  const wasmModule = typeof moduleUrl === "string"
    ? ((await import(/* @vite-ignore */ moduleUrl)) as WasmEngineModule)
    : ((await import("../wasm/glyphspace_wasm.js")) as WasmEngineModule);
  await wasmModule.default?.(typeof moduleUrl === "object" ? moduleUrl : { module_or_path: moduleUrl ?? wasmModuleUrl });
  const engine = new wasmModule.WasmGlyphspaceEngine();
  return {
    kind: "wasm",
    async loadWorld(world) {
      engine.load_world(JSON.stringify(world));
    },
    async validatePatch(world, patch) {
      engine.load_world(JSON.stringify(world));
      const report = JSON.parse(engine.validate_patch(JSON.stringify(patch))) as {
        allowed: boolean;
        warnings?: string[];
        violations?: Array<{ message: string }>;
      };
      return {
        allowed: report.allowed,
        warnings: report.warnings ?? [],
        violations: (report.violations ?? []).map((violation) => violation.message),
      };
    },
    async validateCapabilityInvocation(capability, context) {
      const report = JSON.parse(engine.validate_capability_invocation(JSON.stringify(capability), JSON.stringify(context))) as {
        allowed: boolean;
        warnings?: string[];
        violations?: Array<{ message: string }>;
      };
      return {
        allowed: report.allowed,
        warnings: report.warnings ?? [],
        violations: (report.violations ?? []).map((violation) => violation.message),
      };
    },
    async applyPatch(world, patch) {
      engine.load_world(JSON.stringify(world));
      return JSON.parse(engine.apply_patch(JSON.stringify(patch))) as GlyphWorld;
    },
    async proposePatch(world, request) {
      engine.load_world(JSON.stringify(world));
      return JSON.parse(engine.propose_patch(request)) as PatchProposal;
    },
  };
}


export async function createPolicyBackend(options: { preferWasm?: boolean; moduleUrl?: string } = {}): Promise<PolicyBackend> {
  if (!options.preferWasm) return localPolicyBackend();
  try {
    return await wasmPolicyBackend(options.moduleUrl);
  } catch {
    return localPolicyBackend();
  }
}
