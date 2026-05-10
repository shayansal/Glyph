import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import developerKernelFixture from "../../../tests/conformance/developer-kernel-crm.glyph.json";
import { crmApp, compiledCrmApp, lenses } from "../demo/crmWorld";
import {
  dealUpdateStage,
  initialCrmState,
  invokeCrmCapability,
  type CrmState,
} from "../demo/crmDataSource";
import {
  GlyphspaceRuntime,
  createAccessibilityMirrorSnapshot,
  inMemoryPatchStore,
  localPolicyBackend,
  toCanonicalGlyphJson,
  wasmPolicyBackend,
  type AuditEvent,
  type HostAdapter,
} from "./index";

function host(stateAudits: AuditEvent[] = [], permissions = ["ui.personalize", "crm.deal.read", "crm.deal.write"]): HostAdapter<CrmState> {
  return {
    surface: { kind: "headless" },
    accessibilityMirror: {} as HTMLElement,
    patchStore: inMemoryPatchStore(),
    policyContext: { user_id: "kernel_test", permissions, can_personalize: true },
    deviceProfile: { mode: "two_point_five_d", reducedMotion: false, maximumDepth: false },
    invokeCapability: invokeCrmCapability,
    auditSink(event) {
      stateAudits.push(event);
    },
  };
}

describe("developer kernel conformance", () => {
  it("compiles the DSL to stable canonical .glyph.json", () => {
    const world = compiledCrmApp.world;
    const canonical = toCanonicalGlyphJson(world);

    expect(JSON.parse(canonical)).toEqual(world);
    expect(canonical).toBe(toCanonicalGlyphJson(crmApp.compile().world));
    expect(JSON.parse(canonical)).toEqual(developerKernelFixture);
    expect(world.edges).toEqual([]);
    expect(world.policies).toEqual([]);
    expect(world.glyphs.pipeline.accessibility?.label).toBe("Pipeline stages");
    expect(world.capabilities?.["deal.update_stage"]).toMatchObject({
      id: "deal.update_stage",
      required_permissions: ["crm.deal.write"],
      risk: "medium",
    });

  });

  it("keeps local and WASM policy results aligned", async () => {
    const local = localPolicyBackend();
    const wasm = await wasmPolicyBackend({
      module_or_path: readFileSync(new URL("../wasm/glyphspace_wasm_bg.wasm", import.meta.url)),
    });
    const unsafePatch = {
      spec_version: "0.1.0",
      id: "unsafe_hide_confirmation",
      description: "Hide confirmation",
      ops: [{ type: "hide" as const, glyph_id: "payment_confirmation" }],
    };

    expect(await local.validatePatch(compiledCrmApp.world, unsafePatch)).toEqual(
      await wasm.validatePatch(compiledCrmApp.world, unsafePatch),
    );
  });

  it("blocks capability invocation when the host lacks permission", async () => {
    const runtime = new GlyphspaceRuntime({
      app: crmApp,
      host: host([], ["ui.personalize", "crm.deal.read"]),
      policyBackend: localPolicyBackend(),
      initialState: initialCrmState(),
    });

    await expect(
      runtime.invokeCapability(dealUpdateStage, {
        deal_id: "deal_northstar",
        stage: "negotiation",
      }),
    ).rejects.toThrow("missing permission crm.deal.write");
  });

  it("runs the CRM capability loop and preserves the accessibility mirror", async () => {
    const audits: AuditEvent[] = [];
    const runtime = new GlyphspaceRuntime({
      app: crmApp,
      host: host(audits),
      policyBackend: localPolicyBackend(),
      initialState: initialCrmState(),
    });

    const nextWorld = await runtime.invokeCapability(dealUpdateStage, {
      deal_id: "deal_northstar",
      stage: "negotiation",
    });

    expect(runtime.currentState().deals.find((deal) => deal.id === "deal_northstar")?.stage).toBe("negotiation");
    expect(nextWorld.glyphs.pipeline.priority).toBe("high");
    expect(nextWorld.glyphs.pipeline.style?.tokens?.last_stage_update).toBe("negotiation");
    expect(nextWorld.glyphs.deal_northstar.style?.tokens?.stage).toBe("negotiation");
    expect(await runtime.patchStore().list()).toHaveLength(1);
    expect(audits.map((event) => event.action)).toEqual(["capability.invoked", "patch.applied"]);

    const personalizedWorld = await runtime.applyPatch(lenses.founder);
    const mirror = createAccessibilityMirrorSnapshot(personalizedWorld);
    expect(mirror.nodes.map((node) => node.id)).toContain("payment_confirmation");
    expect(mirror.nodes.find((node) => node.id === "pipeline")?.label).toBe("Pipeline stages");
  });
});
