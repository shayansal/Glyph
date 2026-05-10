import {
  defineCapability,
  jsonSchema,
  type CapabilityDefinition,
  type CapabilityInvocationResult,
  type GlyphPatch,
  type GlyphWorld,
} from "../sdk";

export type DealStage = "lead" | "qualified" | "proposal" | "negotiation" | "closed_won" | "closed_lost";

export interface Deal {
  id: string;
  name: string;
  stage: DealStage;
  owner: string;
  value: number;
}

export interface CrmState {
  deals: Deal[];
  selectedDealId: string;
  followUps: Array<{ id: string; deal_id: string; note: string }>;
}

export interface UpdateStageInput {
  deal_id: string;
  stage: DealStage;
}

export interface UpdateStageOutput {
  deal_id: string;
  stage: DealStage;
}

export interface CreateFollowUpInput {
  deal_id: string;
  note: string;
}

export interface CreateFollowUpOutput {
  id: string;
  deal_id: string;
}

export const dealUpdateStage = defineCapability<UpdateStageInput, UpdateStageOutput>({
  id: "deal.update_stage",
  name: "Update Deal Stage",
  description: "Move a sales opportunity to a new pipeline stage.",
  intent: "move a sales opportunity to a new pipeline stage",
  input_schema: jsonSchema<UpdateStageInput>({
    type: "object",
    required: ["deal_id", "stage"],
    properties: {
      deal_id: { type: "string" },
      stage: { enum: ["lead", "qualified", "proposal", "negotiation", "closed_won", "closed_lost"] },
    },
  }),
  output_schema: jsonSchema<UpdateStageOutput>({
    type: "object",
    required: ["deal_id", "stage"],
    properties: {
      deal_id: { type: "string" },
      stage: { type: "string" },
    },
  }),
  required_permissions: ["crm.deal.write"],
  risk: "medium",
  reversible: true,
  audit: true,
});

export const createFollowUp = defineCapability<CreateFollowUpInput, CreateFollowUpOutput>({
  id: "task.create_followup",
  name: "Create Follow-up",
  intent: "create a sales follow-up task",
  input_schema: jsonSchema<CreateFollowUpInput>({
    type: "object",
    required: ["deal_id", "note"],
    properties: {
      deal_id: { type: "string" },
      note: { type: "string" },
    },
  }),
  output_schema: jsonSchema<CreateFollowUpOutput>({
    type: "object",
    required: ["id", "deal_id"],
    properties: {
      id: { type: "string" },
      deal_id: { type: "string" },
    },
  }),
  required_permissions: ["crm.deal.write"],
  risk: "low",
});

export const closeWon = defineCapability<UpdateStageInput, UpdateStageOutput>({
  id: "deal.close_won",
  name: "Close Deal Won",
  intent: "mark a deal as won",
  required_permissions: ["crm.deal.write"],
  risk: "high",
  requires_confirmation: true,
  audit: true,
});

export const crmCapabilities = [dealUpdateStage, createFollowUp, closeWon];

export function initialCrmState(): CrmState {
  return {
    selectedDealId: "deal_northstar",
    deals: [
      { id: "deal_northstar", name: "Northstar Health", stage: "proposal", owner: "Mira", value: 420000 },
      { id: "deal_aster", name: "Aster Bank", stage: "negotiation", owner: "Jon", value: 780000 },
      { id: "deal_orbit", name: "Orbit Supply", stage: "qualified", owner: "Nia", value: 260000 },
    ],
    followUps: [],
  };
}

export async function invokeCrmCapability(
  capability: CapabilityDefinition,
  input: unknown,
  state: CrmState,
  _world: GlyphWorld,
): Promise<CapabilityInvocationResult> {
  if (capability.id === "deal.update_stage") {
    const update = input as UpdateStageInput;
    const deal = state.deals.find((candidate) => candidate.id === update.deal_id);
    if (!deal) throw new Error(`Unknown deal ${update.deal_id}`);
    deal.stage = update.stage;
    return {
      capability_id: capability.id,
      output: update,
      patch: stagePatch(update),
    };
  }

  if (capability.id === "task.create_followup") {
    const followUpInput = input as CreateFollowUpInput;
    const id = `followup_${state.followUps.length + 1}`;
    state.followUps.push({ id, deal_id: followUpInput.deal_id, note: followUpInput.note });
    return {
      capability_id: capability.id,
      output: { id, deal_id: followUpInput.deal_id },
      patch: followUpPatch(id, followUpInput),
    };
  }

  throw new Error(`Capability ${capability.id} is not implemented by the CRM data source`);
}

function stagePatch(update: UpdateStageInput): GlyphPatch {
  return {
    spec_version: "0.1.0",
    id: `stage_${update.deal_id}_${update.stage}`,
    description: `Updated ${update.deal_id} to ${update.stage}.`,
    ops: [
      { type: "set_style_token", glyph_id: "pipeline", key: "last_stage_update", value: update.stage },
      { type: "set_priority", glyph_id: "pipeline", priority: "high" },
    ],
  };
}

function followUpPatch(id: string, input: CreateFollowUpInput): GlyphPatch {
  return {
    spec_version: "0.1.0",
    id,
    description: `Created follow-up for ${input.deal_id}.`,
    ops: [
      { type: "set_priority", glyph_id: "follow_ups", priority: "high" },
      { type: "set_style_token", glyph_id: "follow_ups", key: "last_followup", value: input.note },
    ],
  };
}

