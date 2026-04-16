import { useState, useMemo, useCallback } from 'react';
import type { SankeyData, SankeyLink } from '@/types/models';

interface SankeyDiagramProps {
  data: SankeyData;
  width?: number;
  height?: number;
  onLinkClick?: (link: SankeyLink) => void;
  /** Called when a right-column (spending category) node is clicked. */
  onCategoryClick?: (category: string) => void;
}

interface LayoutNode {
  id: string;
  name: string;
  type: string;
  column: string;
  x: number;
  y: number;
  height: number;
  totalValue: number;
  color: string;
}

interface LayoutLink {
  source: LayoutNode;
  target: LayoutNode;
  value: number;
  sourceY: number;
  targetY: number;
  bandwidth: number;
  original: SankeyLink;
}

const INCOME_COLORS = ['#22C55E', '#16A34A', '#15803D', '#4ADE80'];
const SOURCE_COLORS = ['#3B82F6', '#6366F1', '#8B5CF6', '#06B6D4'];
const TARGET_COLORS = [
  '#EF4444',
  '#F59E0B',
  '#22C55E',
  '#EC4899',
  '#14B8A6',
  '#F97316',
  '#84CC16',
  '#A855F7',
];

function formatCurrency(value: number): string {
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 0,
    maximumFractionDigits: 0,
  }).format(value);
}

function computeLayout(
  data: SankeyData,
  width: number,
  height: number,
): { nodes: LayoutNode[]; links: LayoutLink[]; has3Columns: boolean } {
  const basePadding = 20;
  const nodeWidth = 18;
  const nodeGap = 8;
  const minNodeHeight = 4;
  const charWidth = 7; // approximate px per character at text-xs (12px)
  const labelGap = 8; // space between bar and label

  // ── 1. Classify nodes into columns ──────────────────────────────
  const hasColumnHints = data.nodes.some((n) => 'column' in n && n.column);

  // Collect unique column names in a deterministic order.
  // Default order: left, primary, middle, secondary, right.
  const COLUMN_ORDER = ['left', 'primary', 'middle', 'secondary', 'right'];

  let columnGroups: Map<string, typeof data.nodes>;

  if (hasColumnHints) {
    columnGroups = new Map<string, typeof data.nodes>();
    for (const n of data.nodes) {
      const col = n.column ?? 'middle';
      if (!columnGroups.has(col)) columnGroups.set(col, []);
      columnGroups.get(col)!.push(n);
    }
    // Sort columns by the predefined order
    const sorted = new Map<string, typeof data.nodes>();
    for (const key of COLUMN_ORDER) {
      if (columnGroups.has(key)) sorted.set(key, columnGroups.get(key)!);
    }
    // Any unknown columns go at the end
    for (const [key, val] of columnGroups) {
      if (!sorted.has(key)) sorted.set(key, val);
    }
    columnGroups = sorted;
  } else {
    // Legacy 2-column fallback
    const sourceIds = new Set(data.links.map((l) => l.source));
    const targetIds = new Set(data.links.map((l) => l.target));
    columnGroups = new Map([
      ['left', data.nodes.filter((n) => sourceIds.has(n.id))],
      ['right', data.nodes.filter((n) => targetIds.has(n.id) && !sourceIds.has(n.id))],
    ]);
  }

  // Remove empty columns
  for (const [key, nodes] of columnGroups) {
    if (nodes.length === 0) columnGroups.delete(key);
  }

  const numColumns = columnGroups.size;
  const columnKeys = [...columnGroups.keys()];

  // Compute dynamic left/right padding based on the longest labels in
  // the first and last columns so text is never clipped.
  const firstColKey = columnKeys[0];
  const lastColKey = columnKeys[columnKeys.length - 1];
  const firstColNodes = firstColKey ? (columnGroups.get(firstColKey) ?? []) : [];
  const lastColNodes = lastColKey ? (columnGroups.get(lastColKey) ?? []) : [];

  const maxFirstLabel = firstColNodes.reduce((m, n) => Math.max(m, n.name.length), 0);
  const maxLastLabel = lastColNodes.reduce((m, n) => Math.max(m, n.name.length), 0);

  const leftPadding = basePadding + maxFirstLabel * charWidth + labelGap;
  const rightPadding = basePadding + maxLastLabel * charWidth + labelGap;

  // Compute X position for each column — evenly spaced within the padded area
  const columnX = new Map<string, number>();
  const totalXSpace = width - leftPadding - rightPadding - nodeWidth;
  columnKeys.forEach((key, i) => {
    const x = numColumns > 1 ? leftPadding + (i / (numColumns - 1)) * totalXSpace : leftPadding;
    columnX.set(key, x);
  });

  // ── 2. Compute node flow-through values ─────────────────────────
  // A node's "value" = max(sum_incoming, sum_outgoing). This ensures
  // the node is tall enough for its widest side of links.
  const nodeIn = new Map<string, number>();
  const nodeOut = new Map<string, number>();
  for (const link of data.links) {
    nodeOut.set(link.source, (nodeOut.get(link.source) ?? 0) + link.value);
    nodeIn.set(link.target, (nodeIn.get(link.target) ?? 0) + link.value);
  }
  const nodeValueMap = new Map<string, number>();
  for (const n of data.nodes) {
    nodeValueMap.set(n.id, Math.max(nodeIn.get(n.id) ?? 0, nodeOut.get(n.id) ?? 0));
  }

  // ── 3. Compute unified scale (pixels per dollar) ────────────────
  // The tallest column (by total node value + gaps) determines the
  // scale. All columns use the same scale so a $5k band has the same
  // pixel thickness everywhere in the diagram.
  const verticalPadding = 40;
  const usableHeight = height - verticalPadding * 2;

  // For each column, figure out how much "value space" needs to fit in usableHeight
  let maxColumnVal = 0;
  let maxColumnGaps = 0;
  for (const [, col] of columnGroups) {
    const val = col.reduce((s, n) => s + (nodeValueMap.get(n.id) ?? 0), 0);
    const gaps = Math.max(0, col.length - 1) * nodeGap;
    if (val > maxColumnVal) {
      maxColumnVal = val;
      maxColumnGaps = gaps;
    }
  }

  // pixelsPerDollar: the tallest column fills the available height
  const ppd = maxColumnVal > 0 ? (usableHeight - maxColumnGaps) / maxColumnVal : 1;

  // ── 4. Layout each column with unified scale & vertical centering ─
  // Color palette per column type
  const COLUMN_COLORS: Record<string, string[]> = {
    left: INCOME_COLORS,
    primary: SOURCE_COLORS,
    middle: SOURCE_COLORS,
    secondary: ['#8B5CF6', '#A855F7', '#7C3AED', '#6D28D9'],
    right: TARGET_COLORS,
  };

  function layoutColumn(
    nodes: typeof data.nodes,
    x: number,
    colors: string[],
    columnLabel: string,
  ): LayoutNode[] {
    // Sort nodes: largest first for better visual flow
    const sorted = [...nodes].sort(
      (a, b) => (nodeValueMap.get(b.id) ?? 0) - (nodeValueMap.get(a.id) ?? 0),
    );

    // Compute total column pixel height
    const totalPx = sorted.reduce(
      (s, n) => s + Math.max(minNodeHeight, (nodeValueMap.get(n.id) ?? 0) * ppd),
      0,
    );
    const gaps = Math.max(0, sorted.length - 1) * nodeGap;
    const columnHeight = totalPx + gaps;

    // Center the column vertically
    let currentY = verticalPadding + (usableHeight - columnHeight) / 2;

    return sorted.map((n, i) => {
      const val = nodeValueMap.get(n.id) ?? 0;
      const h = Math.max(minNodeHeight, val * ppd);
      const node: LayoutNode = {
        id: n.id,
        name: n.name,
        type: n.type,
        column: columnLabel,
        x,
        y: currentY,
        height: h,
        totalValue: val,
        color: colors[i % colors.length] ?? '#888',
      };
      currentY += h + nodeGap;
      return node;
    });
  }

  const allNodes: LayoutNode[] = [];
  for (const [key, nodes] of columnGroups) {
    const x = columnX.get(key) ?? leftPadding;
    const colors = COLUMN_COLORS[key] ?? SOURCE_COLORS;
    allNodes.push(...layoutColumn(nodes, x, colors, key));
  }
  const nodeMap = new Map(allNodes.map((n) => [n.id, n]));

  // ── 5. Layout links with unified bandwidth = value * ppd ────────
  // Sort links by value descending so large bands are laid down first
  // (reduces visual crossings).
  const sortedLinks = [...data.links].sort((a, b) => b.value - a.value);

  const sourceOffsets = new Map<string, number>();
  const targetOffsets = new Map<string, number>();

  const layoutLinks: LayoutLink[] = [];
  for (const link of sortedLinks) {
    const source = nodeMap.get(link.source);
    const target = nodeMap.get(link.target);
    if (!source || !target) continue;

    // Unified scale: bandwidth uses the same ppd as node heights
    const bandwidth = Math.max(1.5, link.value * ppd);

    const sOff = sourceOffsets.get(link.source) ?? 0;
    const tOff = targetOffsets.get(link.target) ?? 0;

    layoutLinks.push({
      source,
      target,
      value: link.value,
      sourceY: source.y + sOff + bandwidth / 2,
      targetY: target.y + tOff + bandwidth / 2,
      bandwidth,
      original: link,
    });

    sourceOffsets.set(link.source, sOff + bandwidth);
    targetOffsets.set(link.target, tOff + bandwidth);
  }

  return { nodes: allNodes, links: layoutLinks, has3Columns: hasColumnHints };
}

export function SankeyDiagram({
  data,
  width = 700,
  height = 400,
  onLinkClick,
  onCategoryClick,
}: SankeyDiagramProps) {
  const [hoveredLink, setHoveredLink] = useState<number | null>(null);
  const [tooltip, setTooltip] = useState<{
    x: number;
    y: number;
    link: LayoutLink;
  } | null>(null);

  const layout = useMemo(() => computeLayout(data, width, height), [data, width, height]);
  // Compute total outflow per source node for percentage display in tooltips
  const sourceOutflows = useMemo(() => {
    const map = new Map<string, number>();
    for (const link of layout.links) {
      map.set(link.source.id, (map.get(link.source.id) ?? 0) + link.value);
    }
    return map;
  }, [layout]);

  const handleMouseEnter = useCallback(
    (index: number, event: React.MouseEvent) => {
      setHoveredLink(index);
      const link = layout.links[index];
      if (link) {
        setTooltip({ x: event.clientX, y: event.clientY, link });
      }
    },
    [layout.links],
  );

  const handleMouseLeave = useCallback(() => {
    setHoveredLink(null);
    setTooltip(null);
  }, []);

  if (!data.nodes.length || !data.links.length) {
    return (
      <div className="flex h-64 items-center justify-center text-[var(--color-text-secondary)]">
        No flow data available for this period
      </div>
    );
  }

  const nodeWidth = 20;

  // Build accessible summary of top flows
  const topFlows = layout.links
    .slice()
    .sort((a, b) => b.value - a.value)
    .slice(0, 5);
  const columnDesc = layout.has3Columns
    ? 'Three-column money flow diagram showing income sources, accounts, and spending categories'
    : 'Money flow diagram';
  const flowSummary = `${columnDesc} with ${layout.nodes.length} nodes and ${layout.links.length} flows. Top flows: ${topFlows.map((l) => `${l.source.name} to ${l.target.name} (${formatCurrency(l.value)})`).join('; ')}.`;

  return (
    <div className="relative" role="img" aria-label={flowSummary}>
      <svg width={width} height={height} className="overflow-visible" aria-hidden="true">
        {/* Draw links */}
        {layout.links.map((link, i) => {
          const sx = link.source.x + nodeWidth;
          const tx = link.target.x;
          const midX = (sx + tx) / 2;
          const path = `M ${sx},${link.sourceY} C ${midX},${link.sourceY} ${midX},${link.targetY} ${tx},${link.targetY}`;
          const isHovered = hoveredLink === i;

          return (
            <path
              key={i}
              d={path}
              fill="none"
              stroke={link.source.color}
              strokeWidth={link.bandwidth}
              strokeOpacity={isHovered ? 0.8 : 0.3}
              className="cursor-pointer transition-opacity"
              onMouseEnter={(e) => handleMouseEnter(i, e)}
              onMouseLeave={handleMouseLeave}
              onClick={() => onLinkClick?.(link.original)}
            />
          );
        })}

        {/* Draw nodes */}
        {layout.nodes.map((node) => {
          // Determine label positioning:
          // - First column: labels to the left
          // - Last column: labels to the right
          // - Interior columns: labels centered above the node
          const colIdx = layout.has3Columns
            ? ['left', 'primary', 'middle', 'secondary', 'right'].indexOf(node.column) >= 0
              ? ['left', 'primary', 'middle', 'secondary', 'right'].indexOf(node.column)
              : 1
            : layout.links.some((l) => l.source.id === node.id)
              ? 0
              : 2;
          const isFirstCol = colIdx === 0;
          const isLastCol = node.column === 'right' || (!layout.has3Columns && colIdx === 2);
          const isInterior = !isFirstCol && !isLastCol;

          let labelX: number;
          let labelAnchor: 'start' | 'middle' | 'end';
          let labelY: number;

          if (isInterior) {
            labelX = node.x + nodeWidth / 2;
            labelAnchor = 'middle';
            labelY = node.y - 6;
          } else if (isFirstCol) {
            labelX = node.x - 6;
            labelAnchor = 'end';
            labelY = node.y + node.height / 2;
          } else {
            labelX = node.x + nodeWidth + 6;
            labelAnchor = 'start';
            labelY = node.y + node.height / 2;
          }

          // In 2-column mode (no column hints), all target nodes are clickable.
          // In multi-column mode, only right-column nodes are clickable.
          const isTarget = !layout.has3Columns
            ? !layout.links.some((l) => l.source.id === node.id) &&
              layout.links.some((l) => l.target.id === node.id)
            : node.column === 'right';
          const isClickable = isTarget && onCategoryClick;

          // Spender-role virtual nodes (ADR-008 Amendment 2) represent
          // direct spending from a primary account routed through the
          // secondary column to keep the layout strictly unidirectional.
          // Render with a dashed outline so users see they're a "role"
          // of an existing primary account rather than a separate one.
          const isSpenderRole = node.type === 'spender_role';

          return (
            <g
              key={node.id}
              className={isClickable ? 'cursor-pointer' : ''}
              onClick={isClickable ? () => onCategoryClick(node.name) : undefined}
              data-node-type={node.type}
            >
              <rect
                x={node.x}
                y={node.y}
                width={nodeWidth}
                height={node.height}
                fill={node.color}
                stroke={isSpenderRole ? 'var(--color-text-secondary)' : 'none'}
                strokeDasharray={isSpenderRole ? '3 2' : undefined}
                rx={3}
              />
              <text
                x={labelX}
                y={isInterior ? labelY : node.y + node.height / 2}
                textAnchor={labelAnchor}
                dominantBaseline={isInterior ? 'auto' : 'central'}
                className="text-xs"
                fill="var(--color-text)"
              >
                {node.name}
              </text>
            </g>
          );
        })}
      </svg>

      <span className="sr-only">{flowSummary}</span>

      {/* Tooltip */}
      {tooltip && (
        <div
          className="pointer-events-none fixed z-50 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg)] p-3 shadow-lg"
          style={{ left: tooltip.x + 12, top: tooltip.y - 10 }}
        >
          <p className="text-sm font-medium text-[var(--color-text)]">
            {tooltip.link.source.name} → {tooltip.link.target.name}
          </p>
          <p className="text-sm text-[var(--color-text-secondary)]">
            {formatCurrency(tooltip.link.value)}
            {(() => {
              const srcTotal = sourceOutflows.get(tooltip.link.source.id) ?? 0;
              return srcTotal > 0
                ? ` (${((tooltip.link.value / srcTotal) * 100).toFixed(1)}% of ${tooltip.link.source.name} outflow)`
                : '';
            })()}
          </p>
        </div>
      )}
    </div>
  );
}
