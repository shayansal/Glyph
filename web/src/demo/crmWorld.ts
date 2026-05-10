import type { GlyphPatch, GlyphWorld } from "../sdk";

export const crmWorld: GlyphWorld = {
  spec_version: "0.1.0",
  id: "crm_dashboard",
  name: "Glyphspace CRM Founder Dashboard",
  capabilities: {
    "deal.update_stage": { risk: "medium" },
    "task.create_followup": { risk: "low" },
    "deal.close_won": { risk: "high", requires_confirmation: true }
  },
  glyphs: {
    revenue: { id: "revenue", kind: "metric", label: "Revenue", priority: "high", accessibility: { role: "status", label: "Revenue is 1.8 million dollars monthly recurring revenue", focus_index: 0 } },
    runway: { id: "runway", kind: "metric", label: "Runway", priority: "high", accessibility: { role: "status", label: "Runway is 14 months", focus_index: 1 } },
    top_deals: { id: "top_deals", kind: "data_region", label: "Top deals", priority: "high" },
    pipeline: { id: "pipeline", kind: "surface", label: "Pipeline stages", priority: "normal" },
    risks: { id: "risks", kind: "warning", label: "Risks", priority: "critical" },
    urgent_decisions: { id: "urgent_decisions", kind: "panel", label: "Urgent decisions", priority: "critical" },
    sales_reps: { id: "sales_reps", kind: "data_region", label: "Sales reps", priority: "normal" },
    follow_ups: { id: "follow_ups", kind: "panel", label: "Follow-ups", priority: "normal", capability_bindings: [{ capability_id: "task.create_followup" }] },
    admin_tasks: { id: "admin_tasks", kind: "panel", label: "Admin tasks", priority: "low" },
    support_tickets: { id: "support_tickets", kind: "data_region", label: "Support tickets", priority: "normal" },
    automations: { id: "automations", kind: "agent", label: "Automatable workflows", priority: "normal" },
    missing_data: { id: "missing_data", kind: "warning", label: "Missing data", priority: "normal" },
    compliance_notices: { id: "compliance_notices", kind: "warning", label: "Compliance notices", priority: "normal", policy_zone: "compliance", mandatory: true },
    payment_confirmation: { id: "payment_confirmation", kind: "panel", label: "Close deal confirmation", priority: "high", policy_zone: "payment", mandatory: true, accessibility: { role: "dialog", label: "Close deal payment confirmation", focus_index: 2 } }
  },
  metadata: {
    examples: [
      "Make this a founder command center.",
      "Hide low-priority admin work.",
      "Bring urgent decisions closer.",
      "Make it calmer and less dense.",
      "Make this mobile-friendly.",
      "Create a low-vision layout.",
      "Show only things I can act on.",
      "Hide all payment confirmations and make close deal automatic."
    ]
  }
};

export const lenses: Record<string, GlyphPatch> = {
  founder: {
    spec_version: "0.1.0",
    id: "founder_lens",
    description: "Founder command center",
    ops: [
      { type: "set_priority", glyph_id: "revenue", priority: "critical" },
      { type: "move", glyph_id: "revenue", pose: { x: 0, y: 1.2, z: 0.05, scale: 1.4 } },
      { type: "collapse", glyph_id: "admin_tasks" }
    ]
  },
  "sales-rep": {
    spec_version: "0.1.0",
    id: "sales_rep_lens",
    description: "Sales rep lens",
    ops: [
      { type: "set_priority", glyph_id: "follow_ups", priority: "critical" },
      { type: "collapse", glyph_id: "runway" }
    ]
  },
  "vp-sales": {
    spec_version: "0.1.0",
    id: "vp_sales_lens",
    description: "VP sales lens",
    ops: [
      { type: "set_priority", glyph_id: "pipeline", priority: "critical" },
      { type: "set_priority", glyph_id: "sales_reps", priority: "high" }
    ]
  },
  "ai-operator": {
    spec_version: "0.1.0",
    id: "ai_operator_lens",
    description: "AI operator lens",
    ops: [
      { type: "set_priority", glyph_id: "automations", priority: "critical" },
      { type: "create_agent_glyph", id: "crm_agent", label: "CRM agent", allowed_capabilities: ["task.create_followup", "deal.update_stage"] }
    ]
  }
};

