import type { Glyph, GlyphPatch, GlyphWorld, PatchOp, Priority } from "./index";

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
    glyphs[glyph.id] = glyph;
  }

  const lenses: Record<string, GlyphPatch> = {};
  for (const lens of definition.lenses ?? []) {
    lenses[lens.id] = lens;
  }

  return {
    world: {
      spec_version: definition.spec_version ?? "0.1.0",
      id: definition.id,
      name: definition.name,
      capabilities,
      glyphs,
      metadata: definition.metadata ?? {},
    },
    lenses,
  };
}

