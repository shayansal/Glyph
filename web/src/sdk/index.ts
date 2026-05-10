export type Priority = "low" | "normal" | "high" | "critical";
export type GlyphKind = "dot" | "cluster" | "card" | "button" | "panel" | "orb" | "room" | "surface" | "agent" | "data_region" | "metric" | "warning";

export interface GlyphPose {
  x: number;
  y: number;
  z: number;
  scale: number;
  rotation_x?: number;
  rotation_y?: number;
  rotation_z?: number;
}

export interface AccessibilityNode {
  role: string;
  label: string;
  description?: string;
  keyboard_action?: string | null;
  focus_index?: number | null;
  spatial_description?: string;
}

export interface Glyph {
  id: string;
  kind: GlyphKind;
  label: string;
  priority?: Priority;
  pose?: Partial<GlyphPose>;
  state?: { hidden?: boolean; collapsed?: boolean; pinned?: boolean; urgent?: boolean; changed?: boolean };
  style?: { density?: "calm" | "comfortable" | "dense"; high_contrast?: boolean; tokens?: Record<string, string> };
  policy_zone?: string;
  mandatory?: boolean;
  capability_bindings?: Array<{ capability_id: string; optional?: boolean }>;
  accessibility?: AccessibilityNode;
}

export interface GlyphWorld {
  spec_version: string;
  id: string;
  name: string;
  glyphs: Record<string, Glyph>;
  capabilities?: Record<string, unknown>;
  metadata?: Record<string, unknown>;
}

export type PatchOp =
  | { type: "move"; glyph_id: string; pose: GlyphPose }
  | { type: "resize"; glyph_id: string; scale: number }
  | { type: "set_priority"; glyph_id: string; priority: Priority }
  | { type: "collapse"; glyph_id: string }
  | { type: "expand"; glyph_id: string }
  | { type: "hide"; glyph_id: string }
  | { type: "show"; glyph_id: string }
  | { type: "set_density"; glyph_id: string; density: "calm" | "comfortable" | "dense" }
  | { type: "set_depth"; glyph_id: string; z: number }
  | { type: "set_accessibility_preference"; glyph_id: string; reduced_motion?: boolean; high_contrast?: boolean }
  | { type: "create_summary_glyph"; id: string; source_glyphs: string[]; label: string }
  | { type: "create_agent_glyph"; id: string; label: string; allowed_capabilities: string[] };

export interface GlyphPatch {
  spec_version: string;
  id: string;
  description: string;
  ops: PatchOp[];
}

export interface PatchProposal {
  patch: GlyphPatch;
  explanation: string;
  confidence: number;
  rejected_operations: string[];
  policy_warnings: string[];
  before_summary: string;
  after_summary: string;
}

type Handler = (payload: unknown) => void;

export function loadWorld(world: GlyphWorld): GlyphWorld {
  return structuredClone(world);
}

export function validatePatch(world: GlyphWorld, patch: GlyphPatch): { allowed: boolean; warnings: string[]; violations: string[] } {
  const violations: string[] = [];
  const warnings: string[] = [];
  for (const op of patch.ops) {
    const glyphId = "glyph_id" in op ? op.glyph_id : "id" in op ? op.id : "";
    const glyph = glyphId ? world.glyphs[glyphId] : undefined;
    if ("glyph_id" in op && !glyph) violations.push(`Missing glyph: ${op.glyph_id}`);
    if (op.type === "hide" && glyph && (glyph.mandatory || ["security", "legal", "payment", "compliance", "mandatory"].includes(glyph.policy_zone ?? ""))) {
      violations.push(`Cannot hide mandatory trust surface: ${glyph.label}`);
    }
    if (op.type === "create_agent_glyph") {
      for (const capability of op.allowed_capabilities) {
        if (!world.capabilities?.[capability]) violations.push(`Cannot create fake capability binding: ${capability}`);
      }
    }
  }
  if (patch.id.includes("unsafe")) warnings.push("AI may rearrange UI but may not create authority.");
  return { allowed: violations.length === 0, warnings, violations };
}

export function applyPatch(world: GlyphWorld, patch: GlyphPatch): GlyphWorld {
  const report = validatePatch(world, patch);
  if (!report.allowed) throw new Error(report.violations.join("; "));
  const next = structuredClone(world);
  for (const op of patch.ops) {
    if (op.type === "create_summary_glyph") {
      next.glyphs[op.id] = { id: op.id, kind: "card", label: op.label, priority: "normal" };
      continue;
    }
    if (op.type === "create_agent_glyph") {
      next.glyphs[op.id] = { id: op.id, kind: "agent", label: op.label, priority: "normal" };
      continue;
    }
    const glyph = next.glyphs[op.glyph_id];
    if (!glyph) continue;
    glyph.state ??= {};
    glyph.pose ??= {};
    glyph.style ??= {};
    if (op.type === "move") glyph.pose = { ...op.pose };
    if (op.type === "resize") glyph.pose.scale = op.scale;
    if (op.type === "set_priority") glyph.priority = op.priority;
    if (op.type === "collapse") glyph.state.collapsed = true;
    if (op.type === "expand") glyph.state.collapsed = false;
    if (op.type === "hide") glyph.state.hidden = true;
    if (op.type === "show") glyph.state.hidden = false;
    if (op.type === "set_density") glyph.style.density = op.density;
    if (op.type === "set_depth") glyph.pose.z = op.z;
    if (op.type === "set_accessibility_preference") {
      glyph.style.high_contrast = op.high_contrast ?? glyph.style.high_contrast;
      glyph.accessibility ??= { role: "group", label: glyph.label };
      glyph.accessibility.spatial_description = op.reduced_motion ? "Reduced-motion accessible layout" : glyph.accessibility.spatial_description;
    }
  }
  return next;
}

export function proposePatch(world: GlyphWorld, request: string): PatchProposal {
  const text = request.toLowerCase();
  if (text.includes("payment confirmation") || text.includes("automatic")) {
    return {
      patch: {
        spec_version: "0.1.0",
        id: "unsafe_request_rejected",
        description: "Rejected unsafe authority changes",
        ops: [{ type: "hide", glyph_id: "payment_confirmation" }]
      },
      explanation: "Policy rejected requests to hide confirmations or auto-run high-risk actions.",
      confidence: 0.95,
      rejected_operations: ["cannot hide confirmation", "cannot auto-run high-risk action"],
      policy_warnings: ["confirmation surfaces must remain visible", "AI may rearrange UI but may not create authority"],
      before_summary: `${Object.keys(world.glyphs).length} glyphs`,
      after_summary: "No unsafe authority change applied."
    };
  }
  const ops: PatchOp[] = [];
  if (text.includes("founder") || text.includes("revenue") || text.includes("risk")) {
    ops.push({ type: "set_priority", glyph_id: "revenue", priority: "critical" });
    ops.push({ type: "set_priority", glyph_id: "runway", priority: "critical" });
    ops.push({ type: "move", glyph_id: "revenue", pose: { x: 0, y: 1.2, z: 0.05, scale: 1.4 } });
    ops.push({ type: "move", glyph_id: "risks", pose: { x: -1.2, y: 0.3, z: 0.15, scale: 1.2 } });
    ops.push({ type: "collapse", glyph_id: "admin_tasks" });
  } else if (text.includes("low vision") || text.includes("accessible")) {
    for (const id of Object.keys(world.glyphs)) {
      ops.push({ type: "resize", glyph_id: id, scale: 1.25 });
      ops.push({ type: "set_accessibility_preference", glyph_id: id, reduced_motion: true, high_contrast: true });
    }
  } else {
    for (const [id, glyph] of Object.entries(world.glyphs)) {
      if ((glyph.priority ?? "normal") === "low") ops.push({ type: "collapse", glyph_id: id });
    }
    ops.push({ type: "set_density", glyph_id: "admin_tasks", density: "calm" });
  }
  return {
    patch: { spec_version: "0.1.0", id: "demo_rule_based_patch", description: request, ops },
    explanation: "Local rule-based adapter generated a policy-checkable patch.",
    confidence: 0.76,
    rejected_operations: [],
    policy_warnings: [],
    before_summary: `${Object.keys(world.glyphs).length} glyphs`,
    after_summary: "Priority, density, accessibility, and spatial placement may change."
  };
}

export class GlyphspaceEngine {
  private handlers = new Map<string, Handler[]>();
  private world?: GlyphWorld;
  private ctx: CanvasRenderingContext2D;
  private mirror?: HTMLElement;

  private constructor(private canvas: HTMLCanvasElement) {
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("Canvas 2D context unavailable");
    this.ctx = ctx;
  }

  static async create(canvas: HTMLCanvasElement): Promise<GlyphspaceEngine> {
    return new GlyphspaceEngine(canvas);
  }

  async loadWorld(world: GlyphWorld): Promise<void> {
    this.world = loadWorld(world);
    this.render();
  }

  setAccessibilityMirror(element: HTMLElement): void {
    this.mirror = element;
    this.renderMirror();
  }

  setLens(lensId: string): void {
    this.emit("lensChanged", lensId);
  }

  on(event: "glyphClick" | "patchProposed" | "lensChanged", handler: Handler): void {
    this.handlers.set(event, [...(this.handlers.get(event) ?? []), handler]);
  }

  proposePatch(request: string): PatchProposal {
    if (!this.world) throw new Error("World not loaded");
    const proposal = proposePatch(this.world, request);
    this.emit("patchProposed", proposal);
    return proposal;
  }

  applyPatch(patch: GlyphPatch): void {
    if (!this.world) throw new Error("World not loaded");
    this.world = applyPatch(this.world, patch);
    this.render();
  }

  resize(): void {
    const rect = this.canvas.getBoundingClientRect();
    const ratio = window.devicePixelRatio || 1;
    this.canvas.width = Math.max(1, Math.floor(rect.width * ratio));
    this.canvas.height = Math.max(1, Math.floor(rect.height * ratio));
    this.render();
  }

  destroy(): void {
    this.handlers.clear();
  }

  private emit(event: string, payload: unknown): void {
    for (const handler of this.handlers.get(event) ?? []) handler(payload);
  }

  private render(): void {
    this.resizeCanvasIfNeeded();
    const ctx = this.ctx;
    const world = this.world;
    ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
    const width = this.canvas.width;
    const height = this.canvas.height;
    ctx.fillStyle = "#0c1116";
    ctx.fillRect(0, 0, width, height);
    if (!world) return;
    const glyphs = Object.values(world.glyphs).filter((glyph) => !glyph.state?.hidden);
    glyphs.sort((a, b) => priorityRank(b.priority) - priorityRank(a.priority) || a.id.localeCompare(b.id));
    glyphs.forEach((glyph, index) => {
      const columns = width < 760 ? 2 : 4;
      const col = index % columns;
      const row = Math.floor(index / columns);
      const scale = glyph.pose?.scale ?? 1;
      const x = glyph.pose?.x ? width / 2 + glyph.pose.x * 140 : 120 + col * ((width - 220) / Math.max(1, columns - 1));
      const y = glyph.pose?.y ? height / 2 - glyph.pose.y * 120 : 130 + row * 120;
      const radius = glyph.state?.collapsed ? 20 : 42 * scale;
      const z = glyph.pose?.z ?? depthForPriority(glyph.priority);
      ctx.globalAlpha = Math.max(0.45, 1 - z * 0.3);
      ctx.fillStyle = colorForGlyph(glyph);
      ctx.beginPath();
      ctx.arc(x, y, radius, 0, Math.PI * 2);
      ctx.fill();
      ctx.lineWidth = glyph.mandatory ? 4 : 2;
      ctx.strokeStyle = glyph.mandatory ? "#f5c36b" : "#88a8ff";
      ctx.stroke();
      ctx.fillStyle = "#f4f7fb";
      ctx.font = `${Math.round(12 * scale)}px Inter, system-ui, sans-serif`;
      ctx.textAlign = "center";
      ctx.fillText(glyph.label, x, y + radius + 18);
    });
    ctx.globalAlpha = 1;
    this.renderMirror();
  }

  private renderMirror(): void {
    if (!this.world || !this.mirror) return;
    const glyphs = Object.values(this.world.glyphs).filter((glyph) => !glyph.state?.hidden);
    glyphs.sort((a, b) => (a.accessibility?.focus_index ?? 999) - (b.accessibility?.focus_index ?? 999));
    this.mirror.innerHTML = "";
    for (const glyph of glyphs) {
      const node = document.createElement("button");
      node.type = "button";
      node.className = "mirror-node";
      node.textContent = glyph.accessibility?.label ?? glyph.label;
      node.setAttribute("role", glyph.accessibility?.role ?? "group");
      node.addEventListener("click", () => this.emit("glyphClick", glyph));
      this.mirror.appendChild(node);
    }
  }

  private resizeCanvasIfNeeded(): void {
    const rect = this.canvas.getBoundingClientRect();
    const ratio = window.devicePixelRatio || 1;
    const width = Math.max(1, Math.floor(rect.width * ratio));
    const height = Math.max(1, Math.floor(rect.height * ratio));
    if (this.canvas.width !== width || this.canvas.height !== height) {
      this.canvas.width = width;
      this.canvas.height = height;
    }
  }
}

function priorityRank(priority: Priority = "normal"): number {
  return { low: 0, normal: 1, high: 2, critical: 3 }[priority];
}

function depthForPriority(priority: Priority = "normal"): number {
  return { critical: 0.05, high: 0.2, normal: 0.6, low: 1 }[priority];
}

function colorForGlyph(glyph: Glyph): string {
  if (glyph.policy_zone === "payment" || glyph.policy_zone === "compliance") return "#7d4f21";
  if (glyph.kind === "warning") return "#b24b5c";
  if (glyph.kind === "agent") return "#3f8f86";
  if (glyph.priority === "critical") return "#5e77ff";
  return "#273445";
}
