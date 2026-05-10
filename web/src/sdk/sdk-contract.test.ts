import {
  defineCapability,
  defineGlyph,
  defineGlyphApp,
  defineLens,
  GlyphspaceRuntime,
  inMemoryPatchStore,
  jsonSchema,
  localPolicyBackend,
  type AuditEvent,
  type HostAdapter,
} from "./index";
import { describe, expect, it } from "vitest";

interface DealUpdateInput {
  deal_id: string;
  stage: "lead" | "qualified" | "proposal" | "negotiation" | "closed_won" | "closed_lost";
}

interface DealUpdateOutput {
  deal_id: string;
  stage: DealUpdateInput["stage"];
}

const updateStage = defineCapability<DealUpdateInput, DealUpdateOutput>({
  id: "deal.update_stage",
  name: "Update Deal Stage",
  intent: "move a sales opportunity to a new pipeline stage",
  input_schema: jsonSchema<DealUpdateInput>({
    type: "object",
    required: ["deal_id", "stage"],
    properties: {
      deal_id: { type: "string" },
      stage: { enum: ["lead", "qualified", "proposal", "negotiation", "closed_won", "closed_lost"] },
    },
  }),
  output_schema: jsonSchema<DealUpdateOutput>({
    type: "object",
    required: ["deal_id", "stage"],
    properties: {
      deal_id: { type: "string" },
      stage: { type: "string" },
    },
  }),
  required_permissions: ["crm.deal.write"],
  risk: "medium",
});

const app = defineGlyphApp({
  id: "crm",
  name: "CRM",
  capabilities: [updateStage],
  glyphs: [
    defineGlyph({
      id: "deal_stage",
      kind: "button",
      label: "Update stage",
      capability_bindings: [{ capability_id: updateStage.id }],
    }),
  ],
  lenses: [
    defineLens({
      id: "founder",
      description: "Founder",
      ops: [{ type: "set_priority", glyph_id: "deal_stage", priority: "critical" }],
    }),
  ],
});

const compiled = app.compile();
const audits: AuditEvent[] = [];
const host: HostAdapter<{ selectedDealId: string }> = {
  surface: { kind: "headless" },
  accessibilityMirror: {} as HTMLElement,
  deviceProfile: { mode: "two_point_five_d", reducedMotion: false, maximumDepth: false },
  policyContext: { user_id: "demo", permissions: ["ui.personalize", "crm.deal.write"], can_personalize: true },
  patchStore: inMemoryPatchStore(),
  async invokeCapability(capability, input) {
    const typedInput = input as DealUpdateInput;
    return { capability_id: capability.id, output: { deal_id: typedInput.deal_id, stage: typedInput.stage } };
  },
  auditSink: (event) => {
    audits.push(event);
  },
};

const runtime = new GlyphspaceRuntime({
  app,
  host,
  policyBackend: localPolicyBackend(),
  initialState: { selectedDealId: "deal_1" },
});

runtime.updateState({ selectedDealId: "deal_2" });
runtime.loadWorld(compiled.world);

describe("SDK type contract", () => {
  it("compiles a typed app and runtime host", () => {
    expect(compiled.world.capabilities?.["deal.update_stage"]).toBeDefined();
    expect(audits.some((event) => event.action === "world.loaded")).toBe(true);
  });
});
