import "./styles.css";
import { crmApp, compiledCrmApp, lenses } from "./demo/crmWorld";
import {
  createFollowUp,
  dealUpdateStage,
  initialCrmState,
  invokeCrmCapability,
  type CrmState,
  type DealStage,
} from "./demo/crmDataSource";
import {
  createPolicyBackend,
  GlyphspaceEngine,
  GlyphspaceRuntime,
  inMemoryPatchStore,
  type AuditEvent,
  type Glyph,
  type GlyphPatch,
  type HostAdapter,
  type PatchProposal,
  type PolicyReport,
} from "./sdk";
import { prettyJson } from "./devtools/inspectors";

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) throw new Error("Missing app root");

app.innerHTML = `
  <main class="shell">
    <aside class="sidebar">
      <div class="brand">Glyphspace</div>
      <nav class="lens-list">
        <button data-lens="founder">Founder</button>
        <button data-lens="sales-rep">Sales rep</button>
        <button data-lens="vp-sales">VP sales</button>
        <button data-lens="ai-operator">AI operator</button>
      </nav>
      <form class="prompt-form">
        <input name="prompt" value="Make this a founder command center." aria-label="Natural language personalization request" />
        <button type="submit">Propose</button>
      </form>
      <div class="actions">
        <button class="accept" disabled>Accept</button>
        <button class="reject" disabled>Reject</button>
        <button class="reset">Reset</button>
        <button class="export">Export patch</button>
        <button class="unsafe-demo" type="button">Try unsafe automation</button>
      </div>
      <section>
        <h2>CRM actions</h2>
        <div class="capability-actions">
          <button data-capability="deal.update_stage" data-stage="negotiation">Move deal to negotiation</button>
          <button data-capability="task.create_followup">Create follow-up</button>
        </div>
      </section>
      <section>
        <h2>Policy</h2>
        <div class="policy"></div>
      </section>
    </aside>
    <section class="stage">
      <canvas aria-hidden="true"></canvas>
      <div class="stage-caption">Semantic glyph world, rendered through the <span class="backend-kind"></span> policy backend.</div>
    </section>
    <aside class="inspector">
      <section>
        <h2>CRM source</h2>
        <div class="crm-state"></div>
      </section>
      <section>
        <h2>Patch proposal</h2>
        <pre class="proposal"></pre>
      </section>
      <section>
        <h2>Audit stream</h2>
        <ol class="audit-log"></ol>
      </section>
      <section>
        <h2>Accessibility mirror</h2>
        <div class="mirror"></div>
      </section>
      <section>
        <h2>World JSON</h2>
        <pre class="json"></pre>
      </section>
    </aside>
  </main>
`;

const canvas = app.querySelector<HTMLCanvasElement>("canvas")!;
const proposalEl = app.querySelector<HTMLPreElement>(".proposal")!;
const jsonEl = app.querySelector<HTMLPreElement>(".json")!;
const policyEl = app.querySelector<HTMLDivElement>(".policy")!;
const mirrorEl = app.querySelector<HTMLDivElement>(".mirror")!;
const stateEl = app.querySelector<HTMLDivElement>(".crm-state")!;
const auditEl = app.querySelector<HTMLOListElement>(".audit-log")!;
const backendKindEl = app.querySelector<HTMLSpanElement>(".backend-kind")!;
const acceptButton = app.querySelector<HTMLButtonElement>(".accept")!;
const rejectButton = app.querySelector<HTMLButtonElement>(".reject")!;

const policyBackend = await createPolicyBackend({ preferWasm: true });
const patchStore = inMemoryPatchStore();
const auditEvents: AuditEvent[] = [];
let currentProposal: PatchProposal | undefined;

const host: HostAdapter<CrmState> = {
  surface: { kind: "canvas", target: canvas },
  accessibilityMirror: mirrorEl,
  patchStore,
  policyContext: {
    user_id: "demo_user",
    permissions: ["ui.personalize", "crm.deal.read", "crm.deal.write"],
    can_personalize: true,
  },
  deviceProfile: { mode: "two_point_five_d", reducedMotion: false, maximumDepth: false },
  invokeCapability: invokeCrmCapability,
  auditSink(event) {
    auditEvents.unshift(event);
    renderAudit();
  },
};

let runtime = createRuntime();
let currentWorld = runtime.currentWorld();
backendKindEl.textContent = policyBackend.kind;

const engine = await GlyphspaceEngine.create(canvas, { policyBackend });
engine.setAccessibilityMirror(mirrorEl);
await engine.loadWorld(currentWorld);
await renderInspectors();
window.addEventListener("resize", () => engine.resize());

app.querySelector(".lens-list")?.addEventListener("click", async (event) => {
  const button = (event.target as HTMLElement).closest<HTMLButtonElement>("button[data-lens]");
  if (!button) return;
  const lens = lenses[button.dataset.lens ?? ""];
  if (!lens) return;
  runtime.loadWorld(compiledCrmApp.world);
  currentWorld = await runtime.applyPatch(lens);
  await engine.loadWorld(currentWorld);
  currentProposal = {
    patch: lens,
    explanation: `${lens.description} applied as a role lens.`,
    confidence: 1,
    rejected_operations: [],
    policy_warnings: [],
    before_summary: "Base CRM world",
    after_summary: "Role lens active",
  };
  await renderInspectors();
});

app.querySelector<HTMLFormElement>(".prompt-form")?.addEventListener("submit", async (event) => {
  event.preventDefault();
  const form = new FormData(event.currentTarget as HTMLFormElement);
  const prompt = String(form.get("prompt") ?? "");
  currentProposal = await engine.proposePatchAsync(prompt);
  proposalEl.textContent = prettyJson(currentProposal);
  const report = await engine.validatePatch(currentProposal.patch);
  renderPolicy(report);
  acceptButton.disabled = !report.allowed || currentProposal.patch.ops.length === 0;
  rejectButton.disabled = false;
});

app.querySelector(".capability-actions")?.addEventListener("click", async (event) => {
  const button = (event.target as HTMLElement).closest<HTMLButtonElement>("button[data-capability]");
  if (!button) return;
  await invokeDemoCapability(button.dataset.capability ?? "", button.dataset.stage as DealStage | undefined);
});

app.querySelector<HTMLButtonElement>(".unsafe-demo")?.addEventListener("click", async () => {
  const input = app.querySelector<HTMLInputElement>('input[name="prompt"]');
  if (input) input.value = "Hide all payment confirmations and make close deal automatic.";
  currentProposal = await engine.proposePatchAsync("Hide all payment confirmations and make close deal automatic.");
  proposalEl.textContent = prettyJson(currentProposal);
  const report = await engine.validatePatch(currentProposal.patch);
  renderPolicy(report);
  acceptButton.disabled = true;
  rejectButton.disabled = false;
});

engine.on("glyphClick", async (payload) => {
  const glyph = payload as Glyph;
  const binding = glyph.capability_bindings?.[0]?.capability_id;
  if (binding) {
    if (glyph.id.startsWith("deal_")) {
      currentWorld = await runtime.updateState({ selectedDealId: glyph.id });
    }
    await invokeDemoCapability(binding, "negotiation");
  }
});

acceptButton.addEventListener("click", async () => {
  if (!currentProposal) return;
  currentWorld = await runtime.applyPatch(currentProposal.patch);
  await engine.loadWorld(currentWorld);
  await renderInspectors();
});

rejectButton.addEventListener("click", async () => {
  currentProposal = undefined;
  proposalEl.textContent = "";
  renderPolicy({ allowed: true, warnings: [], violations: [] });
  acceptButton.disabled = true;
  rejectButton.disabled = true;
});

app.querySelector<HTMLButtonElement>(".reset")?.addEventListener("click", async () => {
  await patchStore.clear();
  runtime = createRuntime();
  currentWorld = runtime.currentWorld();
  currentProposal = undefined;
  auditEvents.length = 0;
  await engine.loadWorld(currentWorld);
  await renderInspectors();
});

app.querySelector<HTMLButtonElement>(".export")?.addEventListener("click", async () => {
  const patches = await patchStore.list();
  const patch: GlyphPatch =
    currentProposal?.patch ?? patches.at(-1) ?? { spec_version: "0.1.0", id: "empty_patch", description: "No active proposal", ops: [] };
  proposalEl.textContent = prettyJson(patch);
});

function createRuntime(): GlyphspaceRuntime<CrmState> {
  return new GlyphspaceRuntime({
    app: crmApp,
    host,
    policyBackend,
    initialState: initialCrmState(),
  });
}

async function invokeDemoCapability(capabilityId: string, stage: DealStage = "negotiation"): Promise<void> {
  if (capabilityId === "deal.update_stage") {
    currentWorld = await runtime.invokeCapability(dealUpdateStage, {
      deal_id: runtime.currentState().selectedDealId,
      stage,
    });
  }
  if (capabilityId === "task.create_followup") {
    currentWorld = await runtime.invokeCapability(createFollowUp, {
      deal_id: runtime.currentState().selectedDealId,
      note: "Call buyer with procurement timeline",
    });
  }
  await engine.loadWorld(currentWorld);
  await renderInspectors();
}

async function renderInspectors(): Promise<void> {
  proposalEl.textContent = currentProposal ? prettyJson(currentProposal) : "";
  jsonEl.textContent = prettyJson(currentWorld);
  const report = currentProposal ? await engine.validatePatch(currentProposal.patch) : { allowed: true, warnings: [], violations: [] };
  renderPolicy(report);
  renderState();
  renderAudit();
  acceptButton.disabled = !currentProposal || !report.allowed || currentProposal.patch.ops.length === 0;
  rejectButton.disabled = !currentProposal;
}

function renderPolicy(report: PolicyReport): void {
  policyEl.className = report.allowed ? "policy ok" : "policy blocked";
  policyEl.innerHTML = `
    <strong>${report.allowed ? "Allowed" : "Blocked"}</strong>
    ${report.allowed ? "" : "<p>I can move the close-deal confirmation, but I cannot hide it or make close deal automatic.</p>"}
    <ul>
      ${[...report.violations, ...report.warnings].map((item) => `<li>${escapeHtml(item)}</li>`).join("") || "<li>No policy issues.</li>"}
    </ul>
  `;
}

function renderState(): void {
  const state = runtime.currentState();
  stateEl.innerHTML = `
    <ul class="state-list">
      ${state.deals
        .map(
          (deal) =>
            `<li><strong>${escapeHtml(deal.name)}</strong><span>${escapeHtml(deal.stage)} · ${escapeHtml(deal.owner)} · $${deal.value.toLocaleString()}</span></li>`,
        )
        .join("")}
    </ul>
    <p>${state.followUps.length} follow-up${state.followUps.length === 1 ? "" : "s"} created.</p>
  `;
}

function renderAudit(): void {
  auditEl.innerHTML =
    auditEvents
      .slice(0, 8)
      .map(
        (event) => `
          <li>
            <strong>${escapeHtml(event.action)}</strong>
            <span>${escapeHtml(event.subject)} · ${escapeHtml(event.detail)}</span>
          </li>
        `,
      )
      .join("") || "<li><span>No audit events yet.</span></li>";
}

function escapeHtml(value: string): string {
  return value.replace(/[&<>"']/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "\"": "&quot;", "'": "&#039;" })[char] ?? char);
}
