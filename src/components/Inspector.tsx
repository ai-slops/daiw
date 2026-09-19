import { ChevronLeft, ChevronRight, Link2, SlidersHorizontal, Trash2 } from "lucide-react";
import type { ExplainResult } from "../domain/api";
import type { DryRunResult } from "../domain/api";
import type { Definition, Instance, Modifier, Project, Track } from "../domain/project";
import { Button, EmptyState, Panel } from "./primitives";

type InspectorProps = {
  readonly project: Project;
  readonly instance: Instance | undefined;
  readonly onRemoveModifier: (modifierId: string) => void;
  readonly onMove: (start: number) => void;
  readonly explanation: ExplainResult | undefined;
  readonly dryRun: DryRunResult | undefined;
  readonly onPreviewMove: (start: number) => void;
};

const Linkage = ({ instance, definition }: { readonly instance: Instance; readonly definition: Definition }) => (
  <div className="linkage-card">
    <div className="linkage-card__nodes">
      <span><Link2 aria-hidden="true" size="var(--icon-sm)" /> SOURCE</span>
      <ChevronRight aria-hidden="true" size="var(--icon-sm)" />
      <span>INSTANCE</span>
    </div>
    <strong>{definition.name}</strong>
    <p>This clip reads notes from the shared source. Source edits update every linked instance; instance edits stay local.</p>
    <small>source rev {definition.revision} · {definition.notes.length} notes · {instance.overrides.length} overrides</small>
  </div>
);

const ModifierRow = ({ modifier, onRemove }: { readonly modifier: Modifier; readonly onRemove: () => void }) => (
  <div className="modifier-row">
    <SlidersHorizontal aria-hidden="true" size="var(--icon-sm)" />
    <div><strong>{modifier.name}</strong><small>{modifier.operations.length} operations</small></div>
    <button type="button" aria-label={`Remove ${modifier.name}`} onClick={onRemove}><Trash2 aria-hidden="true" size="var(--icon-sm)" /></button>
  </div>
);

export const Inspector = ({ project, instance, onRemoveModifier, onMove, explanation, dryRun, onPreviewMove }: InspectorProps) => {
  const definition = project.definitions.find((item) => item.id === instance?.definitionId);
  const track: Track | undefined = project.tracks.find((item) => item.id === instance?.trackId);
  const modifiers = project.modifiers.filter((item) => instance?.modifiers.includes(item.id) === true);

  return (
    <Panel title="Inspector" eyebrow="Selection" className="inspector-panel">
      {instance === undefined || definition === undefined ? <EmptyState>Select an arrangement clip to inspect its link and modifiers.</EmptyState> : (
        <div className="inspector-stack">
          <div className="inspector-name"><strong>{instance.name}</strong><small>{track?.name ?? "Unassigned"} · bar {Math.floor(instance.start / 4) + 1}</small></div>
          <Linkage instance={instance} definition={definition} />
          {explanation?.instance.id === instance.id ? <p className="effective-summary">Rust explanation: {explanation.definition.notes.length} source notes, {explanation.modifiers.length} modifiers, and {explanation.overrides.length} instance overrides.</p> : null}
          <section>
            <div className="section-label"><span>MODIFIER CHAIN</span><small>{modifiers.length}</small></div>
            {modifiers.length === 0 ? <p className="quiet-copy">No modifiers. The instance currently follows its source directly.</p> : null}
            {modifiers.map((modifier) => <ModifierRow key={modifier.id} modifier={modifier} onRemove={() => onRemoveModifier(modifier.id)} />)}
          </section>
          <section>
            <div className="section-label"><span>PLACEMENT</span></div>
            <dl className="property-grid">
              <div><dt>Start</dt><dd>{instance.start.toFixed(2)} beats</dd></div>
              <div><dt>Length</dt><dd>{instance.length.toFixed(2)} beats</dd></div>
              <div><dt>Track</dt><dd>{track?.name ?? "Unknown"}</dd></div>
              <div><dt>Local edits</dt><dd>{instance.overrides.length}</dd></div>
            </dl>
            <div className="nudge-actions">
              <Button icon={ChevronLeft} variant="ghost" onClick={() => onMove(Math.max(0, instance.start - 1))}>Back 1 beat</Button>
              <Button icon={ChevronRight} variant="ghost" onClick={() => onMove(instance.start + 1)}>Forward 1 beat</Button>
            </div>
            <Button variant="subtle" onClick={() => onPreviewMove(instance.start + 1)}>Check next move</Button>
            {dryRun === undefined ? null : <div className="dry-run" data-ready={dryRun.executable}>
              <strong>{dryRun.executable ? "Move is executable" : "Move is blocked"}</strong>
              <small>{dryRun.affectedInstances.length} affected instances · {dryRun.diagnostics.length} diagnostics</small>
              {dryRun.diagnostics.map((item) => <span key={`${item.code}-${item.entityId ?? "project"}`}>{item.severity}: {item.message}</span>)}
            </div>}
          </section>
          <p className="quiet-copy">Detaching a source is intentionally unavailable in Stage 1.</p>
        </div>
      )}
    </Panel>
  );
};
