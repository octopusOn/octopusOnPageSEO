// ECharts wrapper with tree-shaken modules, resize handling and colors from the design tokens.

import { useEffect, useRef, useState } from "react";
import { BarChart, LineChart, PieChart, ScatterChart, TreemapChart } from "echarts/charts";
import { GridComponent, LegendComponent, TooltipComponent } from "echarts/components";
import * as echarts from "echarts/core";
import { CanvasRenderer } from "echarts/renderers";

echarts.use([BarChart, LineChart, PieChart, ScatterChart, TreemapChart, GridComponent, LegendComponent, TooltipComponent, CanvasRenderer]);

export type ChartOption = echarts.EChartsCoreOption;

export interface ChartTokens {
  /** Categorical series colors in their validated, fixed order. */
  series: string[];
  text: string;
  ink: string;
  grid: string;
  axis: string;
  surface: string;
  raised: string;
  line: string;
  good: string;
  info: string;
  warning: string;
  critical: string;
  sans: string;
  mono: string;
}

/** Reads the chart colors of the active theme from the design tokens. */
export function chartTokens(): ChartTokens {
  const style = getComputedStyle(document.documentElement);
  const token = (name: string) => style.getPropertyValue(`--${name}`).trim();
  return {
    series: Array.from({ length: 8 }, (_, index) => token(`chart-${index + 1}`)),
    text: token("ink-faint"),
    ink: token("ink"),
    grid: token("chart-grid"),
    axis: token("chart-axis"),
    surface: token("surface-1"),
    raised: token("surface-raised"),
    line: token("line"),
    good: token("good"),
    info: token("info"),
    warning: token("warning"),
    critical: token("critical"),
    sans: token("font-sans"),
    mono: token("font-mono"),
  };
}

/** Chart colors that follow theme changes (explicit choice or the system setting). */
export function useChartTokens(): ChartTokens {
  const [tokens, setTokens] = useState(chartTokens);
  useEffect(() => {
    const refresh = () => setTokens(chartTokens());
    const observer = new MutationObserver(refresh);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    const media = window.matchMedia("(prefers-color-scheme: light)");
    media.addEventListener("change", refresh);
    return () => {
      observer.disconnect();
      media.removeEventListener("change", refresh);
    };
  }, []);
  return tokens;
}

/** Axis styling shared by the cartesian charts: recessive grid, labels in faint ink. */
export function axisStyle(tokens: ChartTokens) {
  return {
    axisLine: { lineStyle: { color: tokens.axis } },
    axisTick: { show: false },
    axisLabel: { color: tokens.text, fontFamily: tokens.mono, fontSize: 11 },
    splitLine: { lineStyle: { color: tokens.grid } },
  };
}

export function EChart({
  option,
  height = 280,
  onClick,
}: {
  option: ChartOption;
  height?: number;
  onClick?: (data: Record<string, unknown> | undefined) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const chart = useRef<echarts.ECharts | null>(null);
  const tokens = useChartTokens();

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
    const instance = chart.current;
    if (!instance || !onClick) return undefined;
    const handler = (params: { data?: unknown }) => onClick(params.data as Record<string, unknown> | undefined);
    instance.on("click", handler);
    return () => {
      instance.off("click", handler);
    };
  }, [onClick]);

  useEffect(() => {
    const merged: Record<string, unknown> = {
      color: tokens.series,
      textStyle: { fontFamily: tokens.sans, color: tokens.text },
      ...option,
    };
    // The page's legend and tooltip settings extend the themed defaults instead of replacing them.
    if (option.legend && !Array.isArray(option.legend)) {
      merged.legend = {
        icon: "roundRect",
        itemWidth: 10,
        itemHeight: 10,
        ...(option.legend as object),
        textStyle: { color: tokens.text, ...((option.legend as { textStyle?: object }).textStyle ?? {}) },
      };
    }
    if (option.tooltip && !Array.isArray(option.tooltip)) {
      merged.tooltip = {
        backgroundColor: tokens.raised,
        borderColor: tokens.line,
        textStyle: { color: tokens.ink, fontFamily: tokens.sans, fontSize: 12 },
        axisPointer: { lineStyle: { color: tokens.axis }, crossStyle: { color: tokens.axis } },
        extraCssText: "border-radius: 10px; box-shadow: var(--shadow-raised);",
        ...(option.tooltip as object),
      };
    }
    chart.current?.setOption(merged, true);
  }, [option, tokens]);

  return <div ref={ref} className="chart" style={{ height }} />;
}
