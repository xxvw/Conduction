import { useCallback, useEffect, useMemo, useState } from "react";
import ReactFlow, {
  Background,
  Controls,
  Handle,
  Position,
  type Edge,
  type Node,
  type NodeChange,
  type NodeMouseHandler,
  type OnNodesDelete,
  applyNodeChanges,
} from "reactflow";
import "reactflow/dist/style.css";

import type {
  AutomationTrack,
  BuiltInTarget,
  Keyframe,
  TemplateFull,
} from "@/lib/ipc";

interface NodeGraphEditorProps {
  template: TemplateFull;
  editable: boolean;
  onTracksChange?: (next: AutomationTrack[]) => void;
  selectedTrackIdx: number | null;
  onSelectTrack: (idx: number | null) => void;
}

interface SourceData {
  trackIdx: number;
  keyframeCount: number;
  selected: boolean;
}

interface TargetData {
  trackIdx: number;
  label: string;
  color: string;
  selected: boolean;
}

// 1 track ごとに source / target を縦に並べる初期配置。
// 位置はユーザーがドラッグしても永続化しない (薄め MVP)。
const ROW_SPACING = 100;
const ROW_OFFSET = 24;
const SOURCE_X = 32;
const TARGET_X = 320;

function buildNodesAndEdges(
  template: TemplateFull,
  positions: Map<string, { x: number; y: number }>,
  selectedTrackIdx: number | null,
): { nodes: Node[]; edges: Edge[] } {
  const nodes: Node[] = [];
  const edges: Edge[] = [];
  template.tracks.forEach((track, i) => {
    const sId = `s${i}`;
    const tId = `t${i}`;
    const yDefault = ROW_OFFSET + i * ROW_SPACING;
    const sPos = positions.get(sId) ?? { x: SOURCE_X, y: yDefault };
    const tPos = positions.get(tId) ?? { x: TARGET_X, y: yDefault };
    const selected = selectedTrackIdx === i;
    nodes.push({
      id: sId,
      type: "source",
      position: sPos,
      data: {
        trackIdx: i,
        keyframeCount: track.keyframes.length,
        selected,
      } satisfies SourceData,
      selected,
    });
    nodes.push({
      id: tId,
      type: "target",
      position: tPos,
      data: {
        trackIdx: i,
        label: targetLabel(track.target),
        color: targetColor(track.target),
        selected,
      } satisfies TargetData,
      selected,
    });
    edges.push({
      id: `e${i}`,
      source: sId,
      target: tId,
      animated: selected,
      style: { stroke: selected ? "var(--c-accent)" : "var(--c-ink-6)" },
    });
  });
  return { nodes, edges };
}

function trackIdxFromNodeId(id: string): number | null {
  const n = parseInt(id.slice(1), 10);
  return Number.isFinite(n) ? n : null;
}

function SourceNode({ data }: { data: SourceData }) {
  return (
    <div
      className="rf-node rf-node-source"
      data-selected={data.selected || undefined}
    >
      <div className="rf-node-title">Keyframes</div>
      <div className="rf-node-sub">{data.keyframeCount} kf</div>
      <Handle type="source" position={Position.Right} />
    </div>
  );
}

function TargetNode({ data }: { data: TargetData }) {
  return (
    <div
      className="rf-node rf-node-target"
      data-selected={data.selected || undefined}
      style={{ borderColor: data.color }}
    >
      <Handle type="target" position={Position.Left} />
      <div className="rf-node-title" style={{ color: data.color }}>
        {data.label}
      </div>
    </div>
  );
}

const nodeTypes = { source: SourceNode, target: TargetNode };

// Add Track ダイアログ用: built-in target の選択肢。
const TARGET_OPTIONS: BuiltInTarget[] = [
  { type: "crossfader" },
  { type: "master_volume" },
  { type: "deck_volume", deck: "A" },
  { type: "deck_volume", deck: "B" },
  { type: "deck_eq_low", deck: "A" },
  { type: "deck_eq_low", deck: "B" },
  { type: "deck_eq_mid", deck: "A" },
  { type: "deck_eq_mid", deck: "B" },
  { type: "deck_eq_high", deck: "A" },
  { type: "deck_eq_high", deck: "B" },
  { type: "deck_filter", deck: "A" },
  { type: "deck_filter", deck: "B" },
  { type: "deck_echo_wet", deck: "A" },
  { type: "deck_echo_wet", deck: "B" },
  { type: "deck_reverb_wet", deck: "A" },
  { type: "deck_reverb_wet", deck: "B" },
];

export function NodeGraphEditor({
  template,
  editable,
  onTracksChange,
  selectedTrackIdx,
  onSelectTrack,
}: NodeGraphEditorProps) {
  // ドラッグで動かしたユーザー配置をローカルキャッシュ (template 変更で消える)。
  const [positions, setPositions] = useState<Map<string, { x: number; y: number }>>(
    () => new Map(),
  );

  // template が変わった (別 preset に切替、再 load 等) ら配置をクリア。
  useEffect(() => {
    setPositions(new Map());
  }, [template.id]);

  const { nodes: initialNodes, edges } = useMemo(
    () => buildNodesAndEdges(template, positions, selectedTrackIdx),
    [template, positions, selectedTrackIdx],
  );

  // react-flow が drag 中の位置を持つために state を内蔵する必要がある。
  // ただし source の真実は template + positions の組合せ。
  const [nodes, setNodes] = useState<Node[]>(initialNodes);
  useEffect(() => {
    setNodes(initialNodes);
  }, [initialNodes]);

  const onNodesChange = useCallback(
    (changes: NodeChange[]) => {
      // 位置変更のうち drag の終了したものだけ positions に取り込む。
      let posDirty = false;
      const nextPos = new Map(positions);
      for (const c of changes) {
        if (c.type === "position" && c.position && c.dragging === false) {
          nextPos.set(c.id, c.position);
          posDirty = true;
        }
      }
      if (posDirty) setPositions(nextPos);
      // 描画中の中間位置は内部 state に反映 (見た目だけ)。
      setNodes((cur) => applyNodeChanges(changes, cur));
    },
    [positions],
  );

  const onNodeClick: NodeMouseHandler = useCallback(
    (_e, node) => {
      const idx = trackIdxFromNodeId(node.id);
      if (idx == null) return;
      onSelectTrack(idx === selectedTrackIdx ? null : idx);
    },
    [onSelectTrack, selectedTrackIdx],
  );

  const onNodesDelete: OnNodesDelete = useCallback(
    (deleted) => {
      if (!editable || !onTracksChange) return;
      const removeIdxs = new Set<number>();
      for (const n of deleted) {
        const i = trackIdxFromNodeId(n.id);
        if (i != null) removeIdxs.add(i);
      }
      if (removeIdxs.size === 0) return;
      const next = template.tracks.filter((_, i) => !removeIdxs.has(i));
      onTracksChange(next);
      // 選択が消えた場合は解除
      if (selectedTrackIdx != null && removeIdxs.has(selectedTrackIdx)) {
        onSelectTrack(null);
      }
    },
    [editable, onTracksChange, template.tracks, selectedTrackIdx, onSelectTrack],
  );

  const onPaneClick = useCallback(() => {
    onSelectTrack(null);
  }, [onSelectTrack]);

  const [pickerOpen, setPickerOpen] = useState(false);

  const handleAddTrack = useCallback(
    (target: BuiltInTarget) => {
      if (!editable || !onTracksChange) return;
      const range = valueRange(target);
      // デフォルトの定数値: range の中央 (Crossfader/Filter は 0、Volume は 1 等)
      const def =
        range.min === 0 && range.max === 2
          ? 1.0
          : range.min === 0 && range.max === 1
            ? 0.0
            : range.min === -26 && range.max === 6
              ? 0.0
              : 0.0;
      const newTrack: AutomationTrack = {
        target,
        keyframes: [
          {
            position: { kind: "beats", value: 0 },
            value: def,
            curve: "linear",
          } as Keyframe,
          {
            position: { kind: "beats", value: template.duration_beats },
            value: def,
            curve: "linear",
          } as Keyframe,
        ],
      };
      onTracksChange([...template.tracks, newTrack]);
      onSelectTrack(template.tracks.length); // 新規 track を選択
      setPickerOpen(false);
    },
    [editable, onTracksChange, template.tracks, template.duration_beats, onSelectTrack],
  );

  return (
    <div className="node-graph-editor">
      {editable && (
        <div className="node-graph-toolbar">
          <button
            type="button"
            className="btn"
            onClick={() => setPickerOpen((v) => !v)}
          >
            + Add Track
          </button>
          <span className="hint" style={{ fontSize: "var(--fs-micro)" }}>
            Click a node to select · Delete key removes it ·{" "}
            {template.tracks.length} track
            {template.tracks.length === 1 ? "" : "s"}
          </span>
        </div>
      )}
      {pickerOpen && editable && (
        <div className="node-graph-picker">
          {TARGET_OPTIONS.map((t) => (
            <button
              key={targetKeyFor(t)}
              type="button"
              className="chip"
              onClick={() => handleAddTrack(t)}
            >
              {targetLabel(t)}
            </button>
          ))}
        </div>
      )}
      <div className="node-graph-canvas">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          onNodesChange={editable ? onNodesChange : undefined}
          onNodesDelete={editable ? onNodesDelete : undefined}
          onNodeClick={onNodeClick}
          onPaneClick={onPaneClick}
          deleteKeyCode={editable ? ["Backspace", "Delete"] : null}
          nodesDraggable={editable}
          nodesConnectable={false}
          elementsSelectable
          defaultViewport={{ x: 0, y: 0, zoom: 1 }}
          minZoom={0.4}
          maxZoom={1.5}
          proOptions={{ hideAttribution: true }}
        >
          <Background gap={16} color="rgba(255,255,255,0.10)" />
          <Controls showInteractive={false} />
        </ReactFlow>
      </div>
    </div>
  );
}

// ---------- target metadata helpers ----------
// AutomationTimeline と同じ命名規則を保つ (UI 上で見た目が一致するように)。

function targetLabel(target: BuiltInTarget): string {
  switch (target.type) {
    case "crossfader":
      return "Crossfader";
    case "master_volume":
      return "Master Vol";
    case "deck_volume":
      return `Deck ${target.deck} · Volume`;
    case "deck_eq_low":
      return `Deck ${target.deck} · EQ Low`;
    case "deck_eq_mid":
      return `Deck ${target.deck} · EQ Mid`;
    case "deck_eq_high":
      return `Deck ${target.deck} · EQ High`;
    case "deck_filter":
      return `Deck ${target.deck} · Filter`;
    case "deck_echo_wet":
      return `Deck ${target.deck} · Echo Wet`;
    case "deck_reverb_wet":
      return `Deck ${target.deck} · Reverb Wet`;
  }
}

function targetColor(target: BuiltInTarget): string {
  switch (target.type) {
    case "crossfader":
      return "#4FE3B2";
    case "master_volume":
      return "#FFC547";
    case "deck_volume":
      return target.deck === "A" ? "#4FE3B2" : "#E8915A";
    case "deck_eq_low":
    case "deck_eq_mid":
    case "deck_eq_high":
      return target.deck === "A" ? "#7AE655" : "#FF7A33";
    case "deck_filter":
      return "#8A9BE8";
    case "deck_echo_wet":
      return "#A089DC";
    case "deck_reverb_wet":
      return "#48E0F4";
  }
}

function valueRange(target: BuiltInTarget): { min: number; max: number } {
  switch (target.type) {
    case "crossfader":
    case "deck_filter":
      return { min: -1, max: 1 };
    case "master_volume":
    case "deck_volume":
      return { min: 0, max: 2 };
    case "deck_eq_low":
    case "deck_eq_mid":
    case "deck_eq_high":
      return { min: -26, max: 6 };
    case "deck_echo_wet":
    case "deck_reverb_wet":
      return { min: 0, max: 1 };
  }
}

function targetKeyFor(t: BuiltInTarget): string {
  if ("deck" in t) return `${t.type}.${t.deck}`;
  return t.type;
}
