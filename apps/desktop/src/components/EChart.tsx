// ECharts wrapper with tree-shaken modules, resize handling and theme-aware colors.

import { useEffect, useRef } from "react";
import { BarChart, LineChart, PieChart } from "echarts/charts";
import { GridComponent, LegendComponent, TooltipComponent } from "echarts/components";
import * as echarts from "echarts/core";
import { CanvasRenderer } from "echarts/renderers";

echarts.use([BarChart, LineChart, PieChart, GridComponent, LegendComponent, TooltipComponent, CanvasRenderer]);

export type ChartOption = echarts.EChartsCoreOption;

/** Series palette that reads well on light and dark backgrounds. */
export const PALETTE = ["#4f5bd5", "#22b893", "#f2a33a", "#e5566d", "#8a63d2", "#2fa3e0", "#9aa5b8", "#c36fb3", "#64c24f"];

export function themeColors() {
  const style = getComputedStyle(document.documentElement);
  return {
    text: style.getPropertyValue("--text-muted").trim() || "#5d6680",
    border: style.getPropertyValue("--border").trim() || "#dfe3ee",
  };
}

export function EChart({ option, height = 280 }: { option: ChartOption; height?: number }) {
  const ref = useRef<HTMLDivElement>(null);
  const chart = useRef<echarts.ECharts | null>(null);

  useEffect(() => {
    if (!ref.current) return undefined;
    const instance = echarts.init(ref.current, undefined, { renderer: "canvas" });
    chart.current = instance;
    const observer = new ResizeObserver(() => instance.resize());
    observer.observe(ref.current);
    return () => {
      observer.disconnect();
      instance.dispose();
      chart.current = null;
    };
  }, []);

  useEffect(() => {
    chart.current?.setOption({ color: PALETTE, ...option }, true);
  }, [option]);

  return <div ref={ref} className="chart" style={{ height }} />;
}
