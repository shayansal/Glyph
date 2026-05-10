import "./styles.css";
import { crmWorld, lenses } from "./demo/crmWorld";
import { applyPatch, GlyphspaceEngine, loadWorld, proposePatch, validatePatch, type GlyphPatch, type PatchProposal } from "./sdk";
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
      </div>
      <section>
        <h2>Policy</h2>
        <div class="policy"></div>
      </section>
    </aside>
    <section class="stage">
      <canvas aria-hidden="true"></canvas>
      <div class="stage-caption">Semantic glyph world, rendered as a spatial surface.</div>
    </section>
    <aside class="inspector">
      <section>
        <h2>Patch proposal</h2>
        <pre class="proposal"></pre>
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
const acceptButton = app.querySelector<HTMLButtonElement>(".accept")!;
const rejectButton = app.querySelector<HTMLButtonElement>(".reject")!;
let currentWorld = loadWorld(crmWorld);
let currentProposal: PatchProposal | undefined;

const engine = await GlyphspaceEngine.create(canvas);
engine.setAccessibilityMirror(mirrorEl);
await engine.loadWorld(currentWorld);
renderInspectors();
window.addEventListener("resize", () => engine.resize());

app.querySelector(".lens-list")?.addEventListener("click", async (event) => {
  const button = (event.target as HTMLElement).closest<HTMLButtonElement>("button[data-lens]");
  if (!button) return;
  const lens = lenses[button.dataset.lens ?? ""];
  if (!lens) return;
  currentWorld = applyPatch(loadWorld(crmWorld), lens);
  await engine.loadWorld(currentWorld);
  currentProposal = { patch: lens, explanation: `${lens.description} applied as a role lens.`, confidence: 1, rejected_operations: [], policy_warnings: [], before_summary: "Base CRM world", after_summary: "Role lens active" };
  renderInspectors();
});

app.querySelector<HTMLFormElement>(".prompt-form")?.addEventListener("submit", (event) => {
  event.preventDefault();
  const form = new FormData(event.currentTarget as HTMLFormElement);
  const prompt = String(form.get("prompt") ?? "");
  currentProposal = proposePatch(currentWorld, prompt);
  proposalEl.textContent = prettyJson(currentProposal);
  const report = validatePatch(currentWorld, currentProposal.patch);
  renderPolicy(report);
  acceptButton.disabled = !report.allowed || currentProposal.patch.ops.length === 0;
  rejectButton.disabled = false;
});

acceptButton.addEventListener("click", async () => {
  if (!currentProposal) return;
  currentWorld = applyPatch(currentWorld, currentProposal.patch);
  await engine.loadWorld(currentWorld);
  renderInspectors();
});

rejectButton.addEventListener("click", () => {
  currentProposal = undefined;
  proposalEl.textContent = "";
  renderPolicy({ allowed: true, warnings: [], violations: [] });
  acceptButton.disabled = true;
  rejectButton.disabled = true;
});

app.querySelector<HTMLButtonElement>(".reset")?.addEventListener("click", async () => {
  currentWorld = loadWorld(crmWorld);
  currentProposal = undefined;
  await engine.loadWorld(currentWorld);
  renderInspectors();
});

app.querySelector<HTMLButtonElement>(".export")?.addEventListener("click", () => {
  const patch: GlyphPatch = currentProposal?.patch ?? { spec_version: "0.1.0", id: "empty_patch", description: "No active proposal", ops: [] };
  proposalEl.textContent = prettyJson(patch);
});

function renderInspectors(): void {
  proposalEl.textContent = currentProposal ? prettyJson(currentProposal) : "";
  jsonEl.textContent = prettyJson(currentWorld);
  renderPolicy(currentProposal ? validatePatch(currentWorld, currentProposal.patch) : { allowed: true, warnings: [], violations: [] });
  acceptButton.disabled = !currentProposal;
  rejectButton.disabled = !currentProposal;
}

function renderPolicy(report: { allowed: boolean; warnings: string[]; violations: string[] }): void {
  policyEl.className = report.allowed ? "policy ok" : "policy blocked";
  policyEl.innerHTML = `
    <strong>${report.allowed ? "Allowed" : "Blocked"}</strong>
    <ul>
      ${[...report.violations, ...report.warnings].map((item) => `<li>${escapeHtml(item)}</li>`).join("") || "<li>No policy issues.</li>"}
    </ul>
  `;
}

function escapeHtml(value: string): string {
  return value.replace(/[&<>"']/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "\"": "&quot;", "'": "&#039;" })[char] ?? char);
}
