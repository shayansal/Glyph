import type { GlyphPatch, GlyphWorld } from "./index";
import type { CapabilityDefinition } from "./dsl";

export interface RenderSurface {
  kind: "canvas" | "webgpu" | "svg" | "headless";
  target?: HTMLCanvasElement | SVGElement | HTMLElement;
}

export interface DeviceProfile {
  mode: "two_d" | "two_point_five_d" | "three_d";
  reducedMotion: boolean;
  maximumDepth: boolean;
}

export interface PolicyContext {
  user_id: string;
  permissions: string[];
  can_personalize: boolean;
  allow_low_risk_ai_auto_apply?: boolean;
}

export interface CapabilityInvocationResult<Output = unknown> {
  capability_id: string;
  output: Output;
  patch?: GlyphPatch;
}

export interface PatchStore {
  list(): Promise<GlyphPatch[]>;
  save(patch: GlyphPatch): Promise<void>;
  clear(): Promise<void>;
}

export interface AuditEvent {
  id: string;
  timestamp: string;
  action: string;
  subject: string;
  detail: string;
  patch?: GlyphPatch;
}

export interface AccessibilityMirrorNode {
  id: string;
  role: string;
  label: string;
  description: string;
  focus_index: number;
  hidden: boolean;
  mandatory: boolean;
}

export interface AccessibilityMirrorSnapshot {
  nodes: AccessibilityMirrorNode[];
}

export interface HostAdapter<State = unknown> {
  surface: RenderSurface;
  accessibilityMirror: HTMLElement;
  patchStore: PatchStore;
  policyContext: PolicyContext;
  deviceProfile: DeviceProfile;
  invokeCapability(
    capability: CapabilityDefinition,
    input: unknown,
    state: State,
    world: GlyphWorld,
  ): Promise<CapabilityInvocationResult>;
  auditSink(event: AuditEvent): void | Promise<void>;
}

export function inMemoryPatchStore(seed: GlyphPatch[] = []): PatchStore {
  const patches = [...seed];
  return {
    async list() {
      return patches.map((patch) => structuredClone(patch));
    },
    async save(patch) {
      patches.push(structuredClone(patch));
    },
    async clear() {
      patches.length = 0;
    },
  };
}

export function createAccessibilityMirrorSnapshot(world: GlyphWorld): AccessibilityMirrorSnapshot {
  const nodes = Object.values(world.glyphs)
    .filter((glyph) => !glyph.state?.hidden || glyph.mandatory)
    .map((glyph, index) => ({
      id: glyph.id,
      role: glyph.accessibility?.role ?? (glyph.capability_bindings?.length ? "button" : "group"),
      label: glyph.accessibility?.label ?? glyph.label,
      description: glyph.accessibility?.description ?? "",
      focus_index: glyph.accessibility?.focus_index ?? 10_000 + index,
      hidden: glyph.state?.hidden ?? false,
      mandatory: glyph.mandatory ?? false,
    }))
    .sort((left, right) => left.focus_index - right.focus_index || left.id.localeCompare(right.id));

  return { nodes };
}
