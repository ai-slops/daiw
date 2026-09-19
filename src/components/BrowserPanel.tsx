import { AudioLines, Boxes, Layers3, Plus, Search, SlidersHorizontal } from "lucide-react";
import { useMemo, useState } from "react";
import type { Project } from "../domain/project";
import { Button, EmptyState, Panel, Tabs } from "./primitives";

type BrowserTab = "definitions" | "modifiers" | "assets";
const tabs = [
  { value: "definitions", label: "Clips" },
  { value: "modifiers", label: "Mods" },
  { value: "assets", label: "Audio" },
] as const;

type BrowserPanelProps = {
  readonly project: Project;
  readonly selectedInstanceId: string | undefined;
  readonly onApplyModifier: (modifierId: string) => void;
  readonly onCreateModifier: () => void;
  readonly onInstantiate: (definitionId: string) => void;
};

export const BrowserPanel = ({ project, selectedInstanceId, onApplyModifier, onCreateModifier, onInstantiate }: BrowserPanelProps) => {
  const [tab, setTab] = useState<BrowserTab>("definitions");
  const [query, setQuery] = useState("");
  const term = query.trim().toLocaleLowerCase();
  const definitions = useMemo(
    () => project.definitions.filter((item) => item.name.toLocaleLowerCase().includes(term)),
    [project.definitions, term],
  );

  return (
    <Panel title="Browser" eyebrow="Library" className="browser-panel">
      <label className="search-field">
        <Search aria-hidden="true" size="var(--icon-sm)" />
        <span className="sr-only">Search browser</span>
        <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search library" />
      </label>
      <Tabs value={tab} items={tabs} onChange={setTab} label="Browser categories" />
      <div className="browser-list">
        {tab === "definitions" && definitions.map((definition) => (
          <article className="browser-item" key={definition.id}>
            <span className="browser-item__icon"><Boxes aria-hidden="true" size="var(--icon-md)" /></span>
            <div><strong>{definition.name}</strong><small>{definition.notes.length} notes · {definition.length} beats</small></div>
            <Button variant="ghost" onClick={() => onInstantiate(definition.id)} disabled={project.tracks.length === 0} aria-label={`Place ${definition.name} in arrangement`}><Plus aria-hidden="true" size="var(--icon-sm)" /></Button>
          </article>
        ))}
        {tab === "definitions" && definitions.length === 0 ? <EmptyState>No matching clip definitions.</EmptyState> : null}
        {tab === "modifiers" && project.modifiers.map((modifier) => (
          <article className="browser-item" key={modifier.id}>
            <span className="browser-item__icon"><SlidersHorizontal aria-hidden="true" size="var(--icon-md)" /></span>
            <div><strong>{modifier.name}</strong><small>{modifier.operations.length} operations</small></div>
            <Button
              variant="ghost"
              onClick={() => onApplyModifier(modifier.id)}
              disabled={selectedInstanceId === undefined}
              aria-label={`Apply ${modifier.name} to selected clip`}
            ><Plus aria-hidden="true" size="var(--icon-sm)" /></Button>
          </article>
        ))}
        {tab === "modifiers" ? <Button icon={Plus} onClick={onCreateModifier}>New modifier</Button> : null}
        {tab === "assets" && project.assets.map((asset) => (
          <article className="browser-item" key={asset.id}>
            <span className="browser-item__icon"><AudioLines aria-hidden="true" size="var(--icon-md)" /></span>
            <div><strong>{asset.name}</strong><small>{asset.channels}ch · {asset.sampleRate / 1000} kHz</small></div>
            <span className="glyph-wave" aria-hidden="true" />
          </article>
        ))}
        {tab === "assets" && project.assets.length === 0 ? <EmptyState>Import audio to build the asset pool.</EmptyState> : null}
      </div>
    </Panel>
  );
};
