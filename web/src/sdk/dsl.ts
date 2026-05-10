import type { AccessibilityNode, Glyph, GlyphPatch, GlyphPose, GlyphWorld, PatchOp, Priority } from "./index";

export interface JsonSchema<T> {
  readonly json: Record<string, unknown>;
  readonly __type?: T;
}

export function jsonSchema<T>(json: Record<string, unknown>): JsonSchema<T> {
  return { json };
}

export interface CapabilityDefinition<Input = unknown, Output = unknown> {
  id: string;
  name: string;
  description?: string;
  intent: string;
  input_schema?: JsonSchema<Input>;
  output_schema?: JsonSchema<Output>;
  required_permissions?: string[];
  risk?: "none" | "low" | "medium" | "high" | "critical";
  reversible?: boolean;
  requires_confirmation?: boolean;
  audit?: boolean;
  domain_tags?: string[];
  aliases?: string[];
}

export interface CompiledCapability {
  id: string;
  name: string;
  description: string;
  intent: string;
  input_schema: Record<string, unknown>;
  output_schema: Record<string, unknown>;
  required_permissions: string[];
  risk: "none" | "low" | "medium" | "high" | "critical";
  reversible: boolean;
  requires_confirmation: boolean;
  audit: boolean;
  domain_tags: string[];
  aliases: string[];
}

export type CapabilityInput<C> = C extends CapabilityDefinition<infer Input, unknown> ? Input : never;
export type CapabilityOutput<C> = C extends CapabilityDefinition<unknown, infer Output> ? Output : never;

export function defineCapability<Input, Output>(
  definition: CapabilityDefinition<Input, Output>,
): CapabilityDefinition<Input, Output> & { compile(): CompiledCapability } {
  return {
    ...definition,
    compile: () => compileCapability(definition),
  };
}

export function compileCapability(definition: CapabilityDefinition): CompiledCapability {
  return {
    id: definition.id,
    name: definition.name,
    description: definition.description ?? "",
    intent: definition.intent,
    input_schema: definition.input_schema?.json ?? {},
    output_schema: definition.output_schema?.json ?? {},
    required_permissions: definition.required_permissions ?? [],
    risk: definition.risk ?? "low",
    reversible: definition.reversible ?? true,
    requires_confirmation: definition.requires_confirmation ?? false,
    audit: definition.audit ?? true,
    domain_tags: definition.domain_tags ?? [],
    aliases: definition.aliases ?? [],
  };
}

export type GlyphDefinition = Glyph;

export function defineGlyph(definition: GlyphDefinition): GlyphDefinition {
  return definition;
}

export interface LensDefinition {
  id: string;
  description: string;
  ops: PatchOp[];
  role?: string;
  priority?: Priority;
}

export function defineLens(definition: LensDefinition): GlyphPatch {
  return {
    spec_version: "0.1.0",
    id: definition.id,
    description: definition.description,
    ops: definition.ops,
  };
}

export interface GlyphAppDefinition {
  id: string;
  name: string;
  spec_version?: string;
  capabilities?: Array<CapabilityDefinition & { compile?: () => CompiledCapability }>;
  glyphs?: GlyphDefinition[];
  lenses?: GlyphPatch[];
  metadata?: Record<string, unknown>;
}

export interface CompiledGlyphApp {
  world: GlyphWorld;
  lenses: Record<string, GlyphPatch>;
  toGlyphJson(): string;
  toLensJson(id: string): string;
}

export function defineGlyphApp(definition: GlyphAppDefinition): GlyphAppDefinition & { compile(): CompiledGlyphApp } {
  return {
    ...definition,
    compile: () => compileGlyphApp(definition),
  };
}

export function compileGlyphApp(definition: GlyphAppDefinition): CompiledGlyphApp {
  const capabilities: Record<string, CompiledCapability> = {};
  for (const capability of definition.capabilities ?? []) {
    const compiled = capability.compile?.() ?? compileCapability(capability);
    capabilities[compiled.id] = compiled;
  }

  const glyphs: Record<string, Glyph> = {};
  for (const glyph of definition.glyphs ?? []) {
    glyphs[glyph.id] = normalizeGlyph(glyph);
  }

  const lenses: Record<string, GlyphPatch> = {};
  for (const lens of definition.lenses ?? []) {
    lenses[lens.id] = lens;
  }

  const world: GlyphWorld = {
    spec_version: definition.spec_version ?? "0.1.0",
    id: definition.id,
    name: definition.name,
    capabilities,
    glyphs,
    edges: [],
    policies: [],
    spatial_semantics: {
      x_axis: "lateral relationship",
      y_axis: "hierarchy and flow",
      z_axis: "attention depth",
      center: "current focus",
      periphery: "ambient context",
    },
    metadata: definition.metadata ?? {},
  };

  return {
    world,
    lenses,
    toGlyphJson() {
      return toCanonicalGlyphJson(world);
    },
    toLensJson(id) {
      const lens = lenses[id];
      if (!lens) throw new Error(`Unknown lens ${id}`);
      return toCanonicalGlyphJson(lens);
    },
  };
}

function normalizeGlyph(glyph: Glyph): Glyph {
  return {
    ...glyph,
    semantic_role: glyph.semantic_role ?? defaultSemanticRole(glyph),
    priority: glyph.priority ?? "normal",
    pose: normalizePose(glyph.pose),
    state: {
      hidden: glyph.state?.hidden ?? false,
      collapsed: glyph.state?.collapsed ?? false,
      pinned: glyph.state?.pinned ?? false,
      selected: glyph.state?.selected ?? false,
      urgent: glyph.state?.urgent ?? false,
      changed: glyph.state?.changed ?? false,
    },
    style: {
      density: glyph.style?.density ?? "comfortable",
      high_contrast: glyph.style?.high_contrast ?? false,
      tokens: glyph.style?.tokens ?? {},
    },
    policy_zone: glyph.policy_zone ?? "optional",
    mandatory: glyph.mandatory ?? false,
    capability_bindings: (glyph.capability_bindings ?? []).map((binding) => ({
      capability_id: binding.capability_id,
      optional: binding.optional ?? false,
    })),
    accessibility: normalizeAccessibility(glyph.accessibility, glyph),
  };
}

function normalizePose(pose: Partial<GlyphPose> = {}): GlyphPose {
  return {
    x: pose.x ?? 0,
    y: pose.y ?? 0,
    z: pose.z ?? 0,
    scale: pose.scale ?? 1,
    rotation_x: pose.rotation_x ?? 0,
    rotation_y: pose.rotation_y ?? 0,
    rotation_z: pose.rotation_z ?? 0,
  };
}

function normalizeAccessibility(accessibility: AccessibilityNode | undefined, glyph: Glyph): AccessibilityNode {
  return {
    role: accessibility?.role ?? (glyph.capability_bindings?.length ? "button" : "group"),
    label: accessibility?.label ?? glyph.label,
    description: accessibility?.description ?? "",
    state: accessibility?.state ?? "",
    keyboard_action: accessibility?.keyboard_action ?? (glyph.capability_bindings?.length ? "activate" : null),
    focus_index: accessibility?.focus_index ?? null,
    bounding_rect: accessibility?.bounding_rect ?? null,
    spatial_description: accessibility?.spatial_description ?? `${glyph.label} in the semantic Glyphspace world`,
    children: accessibility?.children ?? [],
    live_region: accessibility?.live_region ?? false,
    reduced_motion: accessibility?.reduced_motion ?? false,
    high_contrast: accessibility?.high_contrast ?? false,
  };
}

function defaultSemanticRole(glyph: Glyph): string {
  if (glyph.kind === "metric") return "metric";
  if (glyph.kind === "warning") return glyph.mandatory ? "trust_surface" : "warning";
  if (glyph.kind === "agent") return "agent";
  if (glyph.kind === "data_region") return "data_region";
  if (glyph.capability_bindings?.length) return "action";
  return "content";
}

export function toCanonicalGlyphJson(value: unknown): string {
  return JSON.stringify(canonicalize(value));
}

export function canonicalize(value: unknown): unknown {
  if (Array.isArray(value)) return value.map((item) => canonicalize(item));
  if (!value || typeof value !== "object") return value;

  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .filter(([, entry]) => entry !== undefined)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, entry]) => [key, canonicalize(entry)]),
  );
}
