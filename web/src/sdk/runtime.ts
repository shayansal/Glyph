import type { GlyphPatch, GlyphWorld } from "./index";
import type { CapabilityDefinition, CapabilityInput } from "./dsl";
import type { AuditEvent, HostAdapter } from "./host";
import type { PolicyBackend } from "./policyBackend";

export interface GlyphspaceRuntimeOptions<State> {
  app: { compile(): { world: GlyphWorld; lenses: Record<string, GlyphPatch> } };
  host: HostAdapter<State>;
  policyBackend: PolicyBackend;
  initialState: State;
  stateToPatch?: (state: State, world: GlyphWorld) => GlyphPatch | GlyphPatch[] | undefined;
}

export class GlyphspaceRuntime<State> {
  private world: GlyphWorld;
  private state: State;

  constructor(private readonly options: GlyphspaceRuntimeOptions<State>) {
    const compiled = options.app.compile();
    this.world = compiled.world;
    this.state = options.initialState;
  }

  currentWorld(): GlyphWorld {
    return structuredClone(this.world);
  }

  currentState(): State {
    return structuredClone(this.state);
  }

  loadWorld(world: GlyphWorld): void {
    this.world = structuredClone(world);
    void this.options.policyBackend.loadWorld(this.world);
    this.audit("world.loaded", world.id, `${Object.keys(world.glyphs).length} glyphs loaded`);
  }

  async updateState(nextState: Partial<State>): Promise<GlyphWorld> {
    this.state = { ...this.state, ...nextState };
    this.audit("state.updated", "app-state", Object.keys(nextState).join(", "));
    const patches = this.options.stateToPatch?.(this.state, this.world);
    for (const patch of Array.isArray(patches) ? patches : patches ? [patches] : []) {
      await this.applyPatch(patch);
    }
    return this.currentWorld();
  }

  patchStore() {
    return this.options.host.patchStore;
  }

  async applyPatch(patch: GlyphPatch): Promise<GlyphWorld> {
    const report = await this.options.policyBackend.validatePatch(this.world, patch);
    if (!report.allowed) {
      this.audit("patch.rejected", patch.id, report.violations.join("; "), patch);
      throw new Error(report.violations.join("; "));
    }
    this.world = await this.options.policyBackend.applyPatch(this.world, patch);
    await this.options.host.patchStore.save(patch);
    this.audit("patch.applied", patch.id, patch.description, patch);
    return this.currentWorld();
  }

  async invokeCapability<C extends CapabilityDefinition>(
    capability: C,
    input: CapabilityInput<C>,
  ): Promise<GlyphWorld> {
    const permissionReport = await this.options.policyBackend.validateCapabilityInvocation(
      capability,
      this.options.host.policyContext,
    );
    if (!permissionReport.allowed) {
      this.audit("capability.rejected", capability.id, permissionReport.violations.join("; "));
      throw new Error(permissionReport.violations.join("; "));
    }
    const result = await this.options.host.invokeCapability(capability, input, this.state, this.world);
    this.audit("capability.invoked", capability.id, JSON.stringify(result.output));
    if (result.patch) {
      return this.applyPatch(result.patch);
    }
    return this.currentWorld();
  }

  private audit(action: string, subject: string, detail: string, patch?: GlyphPatch): void {
    const event: AuditEvent = {
      id: `${Date.now()}-${Math.random().toString(36).slice(2)}`,
      timestamp: new Date().toISOString(),
      action,
      subject,
      detail,
      patch,
    };
    void this.options.host.auditSink(event);
  }
}
