import { useState, useMemo, useCallback } from 'react';
import type { SankeyData, SankeyLink } from '@/types/models';

interface SankeyDiagramProps {
  data: SankeyData;
  width?: number;
  height?: number;
  onLinkClick?: (link: SankeyLink) => void;
}

interface LayoutNode {
  id: string;
  name: string;
  type: string;
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
): { nodes: LayoutNode[]; links: LayoutLink[] } {
  const padding = 40;
  const nodeWidth = 20;
  const nodeGap = 12;
  const leftX = padding;
  const rightX = width - padding - nodeWidth;

  // Classify nodes as source or target
  const sourceIds = new Set(data.links.map((l) => l.source));
  const targetIds = new Set(data.links.map((l) => l.target));

  const sourceNodes = data.nodes.filter((n) => sourceIds.has(n.id));
  const targetNodes = data.nodes.filter((n) => targetIds.has(n.id) && !sourceIds.has(n.id));

  // Compute total values per node
  const nodeValueMap = new Map<string, number>();
  for (const link of data.links) {
    nodeValueMap.set(link.source, (nodeValueMap.get(link.source) ?? 0) + link.value);
    nodeValueMap.set(link.target, (nodeValueMap.get(link.target) ?? 0) + link.value);
  }

  const maxValue = Math.max(...nodeValueMap.values(), 1);
  const usableHeight = height - padding * 2;

  function layoutColumn(nodes: typeof data.nodes, x: number, colors: string[]): LayoutNode[] {
    const totalVal = nodes.reduce((s, n) => s + (nodeValueMap.get(n.id) ?? 0), 0);
    const totalGap = Math.max(0, (nodes.length - 1) * nodeGap);
    const availHeight = usableHeight - totalGap;

    let currentY = padding;
    return nodes.map((n, i) => {
      const val = nodeValueMap.get(n.id) ?? 0;
      const h = totalVal > 0 ? Math.max(8, (val / totalVal) * availHeight) : 20;
      const node: LayoutNode = {
        id: n.id,
        name: n.name,
        type: n.type,
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

  const layoutSources = layoutColumn(sourceNodes, leftX, SOURCE_COLORS);
  const layoutTargets = layoutColumn(targetNodes, rightX, TARGET_COLORS);
  const allNodes = [...layoutSources, ...layoutTargets];
  const nodeMap = new Map(allNodes.map((n) => [n.id, n]));

  // Track cumulative offsets for stacking links within each node
  const sourceOffsets = new Map<string, number>();
  const targetOffsets = new Map<string, number>();

  const layoutLinks: LayoutLink[] = [];
  for (const link of data.links) {
    const source = nodeMap.get(link.source);
    const target = nodeMap.get(link.target);
    if (!source || !target) continue;

    const bandwidth = maxValue > 0 ? Math.max(2, (link.value / maxValue) * 40) : 2;

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

    sourceOffsets.set(link.source, sOff + bandwidth + 2);
    targetOffsets.set(link.target, tOff + bandwidth + 2);
  }

  return { nodes: allNodes, links: layoutLinks };
}

export function SankeyDiagram({
  data,
  width = 700,
  height = 400,
  onLinkClick,
}: SankeyDiagramProps) {
  const [hoveredLink, setHoveredLink] = useState<number | null>(null);
  const [tooltip, setTooltip] = useState<{
    x: number;
    y: number;
    link: LayoutLink;
  } | null>(null);

  const layout = useMemo(() => computeLayout(data, width, height), [data, width, height]);
  const totalIncome = useMemo(
    () =>
      layout.nodes
        .filter((n) => layout.links.some((l) => l.source.id === n.id))
        .reduce((s, n) => s + n.totalValue, 0),
    [layout],
  );

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
  const flowSummary = `Money flow diagram showing ${layout.nodes.length} accounts and ${layout.links.length} flows. Top flows: ${topFlows.map((l) => `${l.source.name} to ${l.target.name} (${formatCurrency(l.value)})`).join('; ')}.`;

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
        {layout.nodes.map((node) => (
          <g key={node.id}>
            <rect
              x={node.x}
              y={node.y}
              width={nodeWidth}
              height={node.height}
              fill={node.color}
              rx={3}
            />
            <text
              x={
                layout.links.some((l) => l.source.id === node.id)
                  ? node.x - 6
                  : node.x + nodeWidth + 6
              }
              y={node.y + node.height / 2}
              textAnchor={layout.links.some((l) => l.source.id === node.id) ? 'end' : 'start'}
              dominantBaseline="central"
              className="text-xs"
              fill="var(--color-text)"
            >
              {node.name}
            </text>
            <text
              x={
                layout.links.some((l) => l.source.id === node.id)
                  ? node.x - 6
                  : node.x + nodeWidth + 6
              }
              y={node.y + node.height / 2 + 14}
              textAnchor={layout.links.some((l) => l.source.id === node.id) ? 'end' : 'start'}
              dominantBaseline="central"
              className="text-[10px]"
              fill="var(--color-text-secondary)"
            >
              {formatCurrency(node.totalValue)}
            </text>
          </g>
        ))}
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
            {totalIncome > 0 &&
              ` (${((tooltip.link.value / totalIncome) * 100).toFixed(1)}% of income)`}
          </p>
        </div>
      )}
    </div>
  );
}
