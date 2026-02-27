import * as yaml from "js-yaml";
import yaml__default from "js-yaml";
import * as d3 from "d3";
import * as d3$1 from "d3-array";
function createFormatter(formatString, type = "auto") {
  if (!formatString) {
    return (value) => String(value);
  }
  if (type === "auto") {
    const isDateFormat = /%[A-Za-z]/.test(formatString);
    type = isDateFormat ? "date" : "number";
  }
  if (type === "date") {
    return createDateFormatter(formatString);
  } else {
    return createNumberFormatter(formatString);
  }
}
function createNumberFormatter(formatString) {
  try {
    if (formatString === "~s") {
      return d3.format("~s");
    } else if (formatString.includes("%")) {
      return d3.format(formatString);
    } else {
      return d3.format(formatString);
    }
  } catch (error) {
    console.warn(`[Formatter] Invalid number format: ${formatString}`, error);
    return (value) => String(value);
  }
}
function createDateFormatter(formatString) {
  try {
    const formatter = d3.timeFormat(formatString);
    return (value) => {
      if (value instanceof Date) {
        return formatter(value);
      } else if (typeof value === "string" || typeof value === "number") {
        return formatter(new Date(value));
      }
      return String(value);
    };
  } catch (error) {
    console.warn(`[Formatter] Invalid date format: ${formatString}`, error);
    return (value) => String(value);
  }
}
function createChartTooltip(container) {
  return d3.select(container).append("div").attr("class", "chart-tooltip").style("transition", "opacity 0.2s");
}
function positionTooltip(tooltip, event, options = {}) {
  const { offsetX = 10, offsetY = -10 } = options;
  tooltip.style("left", event.clientX + offsetX + "px").style("top", event.clientY + offsetY + "px");
}
const LEGEND_FONT_SIZE = "11px";
const LEGEND_FONT_FAMILY = "system-ui";
const LEGEND_TEXT_COLOR = "var(--chartml-text)";
const SYMBOL_SIZE = 12;
const SYMBOL_RADIUS = 2;
const SYMBOL_TO_TEXT_GAP = 6;
const ITEM_PADDING = 12;
const ROW_HEIGHT = 20;
const MAX_LABEL_LENGTH = 20;
const MAX_ROWS = 3;
function measureTextWidth(text, fontSize = LEGEND_FONT_SIZE, fontFamily = LEGEND_FONT_FAMILY) {
  const svg = d3.select("body").append("svg").style("position", "absolute").style("visibility", "hidden");
  const textEl = svg.append("text").style("font-size", fontSize).style("font-family", fontFamily).text(text);
  const width = textEl.node().getComputedTextLength();
  svg.remove();
  return width;
}
function truncateLabel(label, maxLength = MAX_LABEL_LENGTH) {
  const str = String(label);
  if (str.length <= maxLength) return { text: str, truncated: false };
  return { text: str.substring(0, maxLength - 1) + "…", truncated: true };
}
function calculateLegendLayout(items, availableWidth, options = {}) {
  const {
    maxRows = MAX_ROWS,
    itemPadding = ITEM_PADDING,
    symbolSize = SYMBOL_SIZE,
    symbolToTextGap = SYMBOL_TO_TEXT_GAP,
    rowHeight = ROW_HEIGHT,
    maxLabelLength = MAX_LABEL_LENGTH
  } = options;
  const rows = [[]];
  let currentRowWidth = 0;
  let currentRowIndex = 0;
  let overflow = false;
  let overflowCount = 0;
  const overflowItems = [];
  for (let i = 0; i < items.length; i++) {
    const item = items[i];
    const { text: displayLabel, truncated } = truncateLabel(item.label, maxLabelLength);
    const textWidth = measureTextWidth(displayLabel);
    const itemWidth = symbolSize + symbolToTextGap + textWidth + itemPadding;
    if (currentRowWidth + itemWidth > availableWidth && rows[currentRowIndex].length > 0) {
      currentRowIndex++;
      if (currentRowIndex >= maxRows) {
        overflow = true;
        overflowCount = items.length - i;
        for (let j = i; j < items.length; j++) {
          overflowItems.push(items[j]);
        }
        break;
      }
      rows.push([]);
      currentRowWidth = 0;
    }
    rows[currentRowIndex].push({
      ...item,
      displayLabel,
      fullLabel: item.label,
      truncated,
      textWidth,
      itemWidth,
      index: i
    });
    currentRowWidth += itemWidth;
  }
  const totalHeight = rows.length * rowHeight;
  return {
    rows,
    totalHeight,
    overflow,
    overflowCount,
    overflowItems
  };
}
const LINE_STYLE_DASH_PATTERNS$1 = { dashed: "8 4", dotted: "2 4" };
function renderSymbol(group, mark, color, size = SYMBOL_SIZE, lineStyle = null, opacity = null) {
  if (mark === "line") {
    const line = group.append("line").attr("x1", 0).attr("y1", size / 2).attr("x2", size).attr("y2", size / 2).attr("stroke", color).attr("stroke-width", 2.5).attr("stroke-linecap", "round");
    if (lineStyle && LINE_STYLE_DASH_PATTERNS$1[lineStyle]) {
      line.attr("stroke-dasharray", LINE_STYLE_DASH_PATTERNS$1[lineStyle]);
    }
  } else if (mark === "range") {
    group.append("rect").attr("width", size).attr("height", size).attr("rx", SYMBOL_RADIUS).attr("fill", color).attr("opacity", opacity || 0.15);
  } else if (mark === "scatter" || mark === "point") {
    group.append("circle").attr("cx", size / 2).attr("cy", size / 2).attr("r", size / 2 - 1).attr("fill", color);
  } else {
    group.append("rect").attr("width", size).attr("height", size).attr("rx", SYMBOL_RADIUS).attr("fill", color);
  }
}
function createLegend(container, items, config = {}) {
  const {
    x = 0,
    y = 0,
    width = 600,
    align = "center",
    // 'left', 'center', 'right'
    maxRows = MAX_ROWS,
    rowHeight = ROW_HEIGHT,
    onItemHover = null,
    onItemLeave = null
  } = config;
  if (items.length <= 1) {
    return { group: null, height: 0 };
  }
  const layout = calculateLegendLayout(items, width, { maxRows, rowHeight });
  const legendGroup = container.append("g").attr("class", "chart-legend").attr("transform", `translate(${x}, ${y})`);
  const tooltip = d3.select("body").append("div").attr("class", "legend-tooltip").style("position", "fixed").style("background", "var(--chartml-surface)").style("color", "var(--chartml-text-strong)").style("padding", "6px 10px").style("border-radius", "4px").style("font-size", "11px").style("font-family", LEGEND_FONT_FAMILY).style("pointer-events", "none").style("opacity", 0).style("z-index", 1e4).style("box-shadow", "var(--chartml-shadow)").style("border", "1px solid var(--chartml-border)").style("max-width", "300px").style("white-space", "pre-wrap");
  layout.rows.forEach((row, rowIndex) => {
    const rowWidth = row.reduce((sum, item) => sum + item.itemWidth, 0);
    let startX = 0;
    if (align === "center") {
      startX = (width - rowWidth) / 2;
    } else if (align === "right") {
      startX = width - rowWidth;
    }
    let currentX = startX;
    row.forEach((item) => {
      const itemGroup = legendGroup.append("g").attr("class", "legend-item").attr("transform", `translate(${currentX}, ${rowIndex * rowHeight})`).attr("data-index", item.index).attr("data-field", item.field || "").style("cursor", "pointer");
      renderSymbol(itemGroup, item.mark || "bar", item.color, SYMBOL_SIZE, item.lineStyle || null, item.opacity || null);
      itemGroup.append("text").attr("x", SYMBOL_SIZE + SYMBOL_TO_TEXT_GAP).attr("y", SYMBOL_SIZE - 1).style("font-size", LEGEND_FONT_SIZE).style("font-family", LEGEND_FONT_FAMILY).style("fill", LEGEND_TEXT_COLOR).text(item.displayLabel);
      itemGroup.on("mouseenter", function(event) {
        if (item.truncated) {
          tooltip.style("opacity", 1).text(item.fullLabel);
        }
        d3.select(this).style("opacity", 1);
        legendGroup.selectAll(".legend-item").filter(function() {
          return this !== event.currentTarget;
        }).style("opacity", 0.3);
        if (onItemHover) {
          onItemHover(item, event);
        }
      }).on("mousemove", function(event) {
        if (item.truncated) {
          tooltip.style("left", event.clientX + 10 + "px").style("top", event.clientY - 10 + "px");
        }
      }).on("mouseleave", function(event) {
        tooltip.style("opacity", 0);
        legendGroup.selectAll(".legend-item").style("opacity", 1);
        if (onItemLeave) {
          onItemLeave(item, event);
        }
      });
      currentX += item.itemWidth;
    });
  });
  if (layout.overflow && layout.overflowCount > 0) {
    const lastRowIndex = layout.rows.length - 1;
    const lastRow = layout.rows[lastRowIndex];
    const lastRowWidth = lastRow.reduce((sum, item) => sum + item.itemWidth, 0);
    let overflowX = 0;
    if (align === "center") {
      overflowX = (width + lastRowWidth) / 2 + 8;
    } else if (align === "right") {
      overflowX = width + 8;
    } else {
      overflowX = lastRowWidth + 8;
    }
    const overflowText = `+${layout.overflowCount} more`;
    const overflowGroup = legendGroup.append("g").attr("class", "legend-overflow").attr("transform", `translate(${overflowX}, ${lastRowIndex * rowHeight})`).style("cursor", "pointer");
    overflowGroup.append("text").attr("y", SYMBOL_SIZE - 1).style("font-size", LEGEND_FONT_SIZE).style("font-family", LEGEND_FONT_FAMILY).style("fill", "var(--chartml-text-secondary)").style("font-style", "italic").text(overflowText);
    const overflowTooltipContent = layout.overflowItems.map((item) => item.label).join("\n");
    overflowGroup.on("mouseenter", function(event) {
      tooltip.style("opacity", 1).text(overflowTooltipContent);
    }).on("mousemove", function(event) {
      tooltip.style("left", event.clientX + 10 + "px").style("top", event.clientY - 10 + "px");
    }).on("mouseleave", function() {
      tooltip.style("opacity", 0);
    });
  }
  return {
    group: legendGroup,
    height: layout.totalHeight,
    layout,
    /**
     * Highlight a specific legend item by index or field
     */
    highlight: (indexOrField) => {
      legendGroup.selectAll(".legend-item").each(function() {
        const el = d3.select(this);
        const idx = parseInt(el.attr("data-index"));
        const field = el.attr("data-field");
        const match = indexOrField === idx || indexOrField === field;
        el.style("opacity", match ? 1 : 0.3);
      });
    },
    /**
     * Reset all legend items to normal opacity
     */
    reset: () => {
      legendGroup.selectAll(".legend-item").style("opacity", 1);
    },
    /**
     * Clean up tooltip on destroy
     */
    destroy: () => {
      tooltip.remove();
    }
  };
}
function calculateLegendHeight(items, availableWidth, options = {}) {
  if (items.length <= 1) return 0;
  const layout = calculateLegendLayout(items, availableWidth, options);
  return layout.totalHeight + 10;
}
const AXIS_LABEL_FONT_SIZE = "12px";
const AXIS_LABEL_FONT_FAMILY = "system-ui";
const LINE_STYLE_DASH_PATTERNS = { dashed: "8 4", dotted: "2 4" };
function getAnimationDuration(baseMs, animationEnabled) {
  return animationEnabled ? baseMs : 0;
}
function calculateXAxisMargin(data, xField, xScale, chartWidth) {
  if (data.length === 0) return { marginNeeded: 40, rotationDegrees: 0 };
  const tempSvg = d3.select("body").append("svg").style("position", "absolute").style("visibility", "hidden");
  const tempAxis = d3.axisBottom(xScale);
  const tempG = tempSvg.append("g").call(tempAxis);
  const labels = tempG.selectAll("text");
  const labelCount = labels.size();
  if (labelCount === 0) {
    tempSvg.remove();
    return { marginNeeded: 40, rotationDegrees: 0 };
  }
  labels.style("font-size", AXIS_LABEL_FONT_SIZE).style("font-family", AXIS_LABEL_FONT_FAMILY);
  let maxLabelWidth = 0;
  let maxLabelHeight = 0;
  labels.each(function() {
    const bbox = this.getBBox();
    maxLabelWidth = Math.max(maxLabelWidth, bbox.width);
    maxLabelHeight = Math.max(maxLabelHeight, bbox.height);
  });
  tempSvg.remove();
  const tickWidth = xScale.bandwidth ? xScale.step() : chartWidth / labelCount;
  const labelMargin = 6;
  const wouldOverlap = maxLabelWidth + labelMargin > tickWidth;
  if (!wouldOverlap) {
    return { marginNeeded: 40, rotationDegrees: 0 };
  }
  const maxRotation = 45;
  const rotationRadians = Math.asin(Math.min((maxLabelHeight + labelMargin) / tickWidth, 1));
  const rotationDegrees = Math.min(Math.max(rotationRadians * (180 / Math.PI), 0), maxRotation);
  const radians = rotationDegrees * (Math.PI / 180);
  const rotatedHeight = maxLabelWidth * Math.sin(radians) + maxLabelHeight * Math.cos(radians);
  const marginNeeded = Math.ceil(rotatedHeight) + 10;
  return { marginNeeded, rotationDegrees };
}
function handleXAxisLabelOverlap(xAxis, xScale, chartWidth, isDateScale, rotationDegrees) {
  const labels = xAxis.selectAll("text");
  const labelCount = labels.size();
  if (labelCount === 0) return;
  labels.style("font-size", AXIS_LABEL_FONT_SIZE).style("font-family", AXIS_LABEL_FONT_FAMILY);
  if (rotationDegrees === 0) {
    labels.style("text-anchor", "middle");
    return;
  }
  labels.style("text-anchor", "end").attr("transform", `rotate(-${rotationDegrees})`).attr("dx", "-0.5em").attr("dy", "0.15em");
  let maxLabelWidth = 0;
  labels.each(function() {
    const bbox = this.getBBox();
    maxLabelWidth = Math.max(maxLabelWidth, bbox.width);
  });
  const tickWidth = xScale.bandwidth ? xScale.step() : chartWidth / labelCount;
  const labelMargin = 6;
  const radians = rotationDegrees * (Math.PI / 180);
  const rotatedWidth = maxLabelWidth * Math.cos(radians);
  const overlapRatio = (rotatedWidth + labelMargin) / tickWidth;
  if (overlapRatio > 1.5 && labelCount > 8) {
    const skipInterval = Math.ceil(overlapRatio / 2);
    labels.each(function(d, i) {
      if (i % skipInterval !== 0) {
        d3.select(this).style("opacity", 0);
      }
    });
  }
}
function setupSvgContainer(container, width, height, marginTop, marginRight, marginBottom, marginLeft) {
  container.innerHTML = "";
  d3.select(container).style("position", "relative");
  const chartWidth = width - marginLeft - marginRight;
  const chartHeight = height - marginTop - marginBottom;
  const svg = d3.select(container).append("svg").attr("width", "100%").attr("height", height).attr("viewBox", [0, 0, width, height]).style("max-width", "100%").style("display", "block").style("overflow", "hidden");
  const g = svg.append("g").attr("transform", `translate(${marginLeft},${marginTop})`);
  return { svg, g, chartWidth, chartHeight };
}
function sanitizeClassName(str) {
  if (!str || typeof str !== "string") return "unknown";
  return str.replace(/[^a-zA-Z0-9_-]/g, "_").replace(/^[0-9]/, "_$&");
}
function determineScaleTypes(data, xField) {
  const isDateScale = data.length > 0 && data[0][xField] instanceof Date;
  return { isDateScale };
}
function detectTimeInterval(data, xField) {
  if (data.length < 2) return null;
  const uniqueDates = Array.from(new Set(data.map((d) => d[xField].getTime()))).sort((a, b) => a - b);
  if (uniqueDates.length < 2) return null;
  const intervals = [];
  for (let i = 1; i < uniqueDates.length; i++) {
    intervals.push(uniqueDates[i] - uniqueDates[i - 1]);
  }
  intervals.sort((a, b) => a - b);
  const medianInterval = intervals[Math.floor(intervals.length / 2)];
  return medianInterval;
}
function getSmartTooltipFormat(data, xField) {
  const interval = detectTimeInterval(data, xField);
  if (!interval) {
    return d3.utcFormat("%b %d, %Y");
  }
  const MS_DAY = 24 * 60 * 60 * 1e3;
  const MS_MONTH = 30 * MS_DAY;
  if (interval < MS_DAY) {
    return d3.utcFormat("%b %d, %Y %H:%M");
  } else if (interval < MS_MONTH) {
    return d3.utcFormat("%b %d, %Y");
  } else {
    return d3.utcFormat("%B %Y");
  }
}
function getSmartTimestampFormat(data, xField) {
  const interval = detectTimeInterval(data, xField);
  if (!interval) {
    return d3.utcFormat("%b %d");
  }
  const uniqueDates = Array.from(new Set(data.map((d) => d[xField].getTime()))).sort((a, b) => a - b);
  const totalSpan = uniqueDates.length >= 2 ? uniqueDates[uniqueDates.length - 1] - uniqueDates[0] : 0;
  const MS_HOUR = 60 * 60 * 1e3;
  const MS_DAY = 24 * MS_HOUR;
  const MS_WEEK = 7 * MS_DAY;
  const MS_MONTH = 30 * MS_DAY;
  if (interval < MS_DAY) {
    if (totalSpan < MS_DAY * 2) {
      return d3.utcFormat("%H:%M");
    } else {
      return d3.utcFormat("%b %d %H:%M");
    }
  } else if (interval < MS_WEEK) {
    return d3.utcFormat("%b %d");
  } else if (interval < MS_MONTH) {
    return d3.utcFormat("%b %d");
  } else {
    return d3.utcFormat("%b '%y");
  }
}
function calculateDateScaleBarWidth(data, xField, chartWidth) {
  if (data.length <= 1) return 20;
  const uniqueTimes = Array.from(new Set(data.map((d) => d[xField].getTime()))).sort((a, b) => a - b);
  const dataPointCount = uniqueTimes.length;
  if (dataPointCount <= 1) return 20;
  let minGap = Infinity;
  for (let i = 1; i < uniqueTimes.length; i++) {
    const gap = uniqueTimes[i] - uniqueTimes[i - 1];
    if (gap < minGap) minGap = gap;
  }
  const totalSpan = uniqueTimes[uniqueTimes.length - 1] - uniqueTimes[0];
  if (totalSpan === 0) return 20;
  const minGapFraction = minGap / totalSpan;
  const inset = chartWidth / (2 * dataPointCount);
  const effectiveWidth = chartWidth - 2 * inset;
  const calculatedWidth = effectiveWidth * minGapFraction * 0.7;
  const maxBarWidth = chartWidth * 0.15;
  const barWidth = Math.max(2, Math.min(calculatedWidth, maxBarWidth));
  return barWidth;
}
function createScales(data, rows, xField, chartWidth, chartHeight, isDateScale, mode, axes = {}) {
  var _a, _b, _c, _d, _e, _f;
  let x;
  if (isDateScale) {
    const uniqueDates = new Set(data.map((d) => d[xField].getTime()));
    const dataPointCount = uniqueDates.size;
    let inset = 30;
    if (dataPointCount >= 2) {
      inset = chartWidth / (2 * dataPointCount);
    }
    x = d3.scaleUtc().domain(d3.extent(data, (d) => d[xField])).range([inset, chartWidth - inset]);
  } else {
    const barCount2 = data.length;
    let padding;
    if (barCount2 <= 10) {
      padding = 0.2;
    } else if (barCount2 <= 20) {
      padding = 0.15;
    } else if (barCount2 <= 40) {
      padding = 0.1;
    } else {
      padding = 0.05;
    }
    x = d3.scaleBand().domain(data.map((d) => d[xField])).range([0, chartWidth]).padding(padding);
  }
  const leftRows = rows.filter((r) => (!r.axis || r.axis === "left") && r.mark !== "range");
  const rightRows = rows.filter((r) => r.axis === "right" && r.mark !== "range");
  const leftRangeRows = rows.filter((r) => (!r.axis || r.axis === "left") && r.mark === "range");
  const rightRangeRows = rows.filter((r) => r.axis === "right" && r.mark === "range");
  let yLeftMin, yLeftMax;
  const leftFields = leftRows.map((r) => r.field);
  const leftMarks = leftRows.map((r) => r.mark);
  const barCount = leftMarks.filter((m) => m === "bar").length;
  const areaCount = leftMarks.filter((m) => m === "area").length;
  const hasStackedBars = barCount > 1 && mode === "stacked";
  const hasStackedAreas = areaCount > 1 && mode === "stacked";
  const hasNormalizedAreas = areaCount > 1 && mode === "normalized";
  if (hasNormalizedAreas) {
    yLeftMin = 0;
    yLeftMax = 1;
  } else if (hasStackedBars || hasStackedAreas) {
    yLeftMin = 0;
    yLeftMax = d3.max(data, (d) => d3.sum(leftFields, (field) => d[field] || 0));
  } else {
    const allLeftValues = data.flatMap((d) => leftFields.map((field) => d[field] || 0));
    yLeftMin = Math.min(0, d3.min(allLeftValues) || 0);
    yLeftMax = d3.max(allLeftValues) || 1;
  }
  if (leftRangeRows.length > 0) {
    const rangeValues = data.flatMap(
      (d) => leftRangeRows.flatMap((r) => [d[r.upper] || 0, d[r.lower] || 0])
    );
    const rangeMin = d3.min(rangeValues) || 0;
    const rangeMax = d3.max(rangeValues) || 0;
    yLeftMin = Math.min(yLeftMin, rangeMin);
    yLeftMax = Math.max(yLeftMax, rangeMax);
  }
  if (((_a = axes.left) == null ? void 0 : _a.min) !== void 0) yLeftMin = axes.left.min;
  if (((_b = axes.left) == null ? void 0 : _b.max) !== void 0) yLeftMax = axes.left.max;
  const yLeft = d3.scaleLinear().domain([yLeftMin, yLeftMax]).range([chartHeight, 0]);
  if (((_c = axes.left) == null ? void 0 : _c.nice) !== false) {
    yLeft.nice();
  }
  let yRight = null;
  if (rightRows.length > 0 || rightRangeRows.length > 0) {
    const rightFields = rightRows.map((r) => r.field);
    const allRightValues = data.flatMap((d) => rightFields.map((field) => d[field] || 0));
    let yRightMin = Math.min(0, d3.min(allRightValues) || 0);
    let yRightMax = d3.max(allRightValues) || 1;
    if (rightRangeRows.length > 0) {
      const rangeValues = data.flatMap(
        (d) => rightRangeRows.flatMap((r) => [d[r.upper] || 0, d[r.lower] || 0])
      );
      const rangeMin = d3.min(rangeValues) || 0;
      const rangeMax = d3.max(rangeValues) || 0;
      yRightMin = Math.min(yRightMin, rangeMin);
      yRightMax = Math.max(yRightMax, rangeMax);
    }
    if (((_d = axes.right) == null ? void 0 : _d.min) !== void 0) yRightMin = axes.right.min;
    if (((_e = axes.right) == null ? void 0 : _e.max) !== void 0) yRightMax = axes.right.max;
    yRight = d3.scaleLinear().domain([yRightMin, yRightMax]).range([chartHeight, 0]);
    if (((_f = axes.right) == null ? void 0 : _f.nice) !== false) {
      yRight.nice();
    }
  }
  return { x, yLeft, yRight };
}
function measureLabelWidths$1(labels, fontSize = AXIS_LABEL_FONT_SIZE, fontFamily = AXIS_LABEL_FONT_FAMILY) {
  const svg = d3.select("body").append("svg").style("position", "absolute").style("visibility", "hidden").style("width", "0").style("height", "0");
  const measurements = labels.map((label) => {
    const text = svg.append("text").style("font-size", fontSize).style("font-family", fontFamily).text(label);
    const width = text.node().getComputedTextLength();
    text.remove();
    return width;
  });
  svg.remove();
  return measurements;
}
function addAxesAndLabels(g, svg, scales, axes, chartWidth, chartHeight, marginLeft, marginRight, marginBottom, isDateScale, mode, container, data, xField, width, labelRotationDegrees) {
  var _a, _b, _c, _d, _e, _f, _g, _h;
  const { x, yLeft, yRight } = scales;
  let xAxisGenerator = d3.axisBottom(x);
  if (isDateScale) {
    const uniqueDates = Array.from(new Set(data.map((d) => d[xField].getTime()))).map((time) => new Date(time)).sort((a, b) => a - b);
    const dataPointCount = uniqueDates.length;
    const maxFittableTicks = Math.floor(chartWidth / 50);
    if (dataPointCount <= maxFittableTicks) {
      xAxisGenerator = xAxisGenerator.tickValues(uniqueDates);
    } else {
      const suggestedTicks = Math.max(4, Math.min(maxFittableTicks, 10));
      xAxisGenerator = xAxisGenerator.ticks(suggestedTicks);
    }
    if ((_a = axes.x) == null ? void 0 : _a.format) {
      const formatter = createFormatter(axes.x.format, "date");
      xAxisGenerator = xAxisGenerator.tickFormat(formatter);
    } else {
      xAxisGenerator = xAxisGenerator.tickFormat(getSmartTimestampFormat(data, xField));
    }
  } else {
    if ((_b = axes.x) == null ? void 0 : _b.format) {
      const formatter = createFormatter(axes.x.format, "auto");
      xAxisGenerator = xAxisGenerator.tickFormat(formatter);
    }
  }
  const xAxis = g.append("g").attr("transform", `translate(0,${chartHeight})`).style("color", "var(--chartml-axis-line)").call(xAxisGenerator);
  handleXAxisLabelOverlap(xAxis, x, chartWidth, isDateScale, labelRotationDegrees);
  if (isDateScale) {
    xAxis.select(".domain").remove();
    g.append("line").attr("x1", 0).attr("x2", chartWidth).attr("y1", chartHeight).attr("y2", chartHeight).attr("stroke", "currentColor").attr("stroke-width", 1);
  }
  if ((_c = axes.x) == null ? void 0 : _c.label) {
    g.append("text").attr("x", chartWidth / 2).attr("y", chartHeight + marginBottom - 5).attr("text-anchor", "middle").style("font-size", "14px").style("font-family", AXIS_LABEL_FONT_FAMILY).style("fill", "var(--chartml-text)").text(axes.x.label);
  }
  let yAxisLeftGenerator = d3.axisLeft(yLeft).ticks(5);
  if (mode === "normalized") {
    yAxisLeftGenerator = yAxisLeftGenerator.tickFormat(d3.format(".0%"));
  } else if ((_d = axes.left) == null ? void 0 : _d.format) {
    const formatter = createFormatter(axes.left.format, "number");
    yAxisLeftGenerator = yAxisLeftGenerator.tickFormat(formatter);
  }
  const yAxisLeft = g.append("g").style("color", "var(--chartml-axis-line)").call(yAxisLeftGenerator);
  yAxisLeft.selectAll("text").style("font-size", AXIS_LABEL_FONT_SIZE).style("font-family", AXIS_LABEL_FONT_FAMILY);
  let maxTickWidth = 0;
  yAxisLeft.selectAll("text").each(function() {
    const bbox = this.getBBox();
    maxTickWidth = Math.max(maxTickWidth, bbox.width);
  });
  const hasLeftLabel = !!((_e = axes.left) == null ? void 0 : _e.label);
  let requiredSpace;
  if (hasLeftLabel) {
    const gap = 10;
    const axisLabelSpace = 30;
    requiredSpace = maxTickWidth + gap + axisLabelSpace;
  } else {
    const buffer = 15;
    requiredSpace = maxTickWidth + buffer;
  }
  const availableSpace = marginLeft;
  let effectiveMarginLeft = marginLeft;
  if (requiredSpace > availableSpace) {
    const additionalMargin = requiredSpace - availableSpace;
    effectiveMarginLeft = marginLeft + additionalMargin;
    const currentTransform = g.attr("transform");
    const newTransform = currentTransform.replace(
      `translate(${marginLeft},`,
      `translate(${effectiveMarginLeft},`
    );
    g.attr("transform", newTransform);
  } else if (requiredSpace < availableSpace && !hasLeftLabel) {
    effectiveMarginLeft = requiredSpace;
    const currentTransform = g.attr("transform");
    const newTransform = currentTransform.replace(
      `translate(${marginLeft},`,
      `translate(${effectiveMarginLeft},`
    );
    g.attr("transform", newTransform);
  }
  if ((_f = axes.left) == null ? void 0 : _f.label) {
    g.append("text").attr("transform", "rotate(-90)").attr("x", -chartHeight / 2).attr("y", -effectiveMarginLeft + 15).attr("text-anchor", "middle").style("font-size", "14px").style("font-family", AXIS_LABEL_FONT_FAMILY).style("fill", "var(--chartml-text)").text(axes.left.label);
  }
  if (yRight) {
    let yAxisRightGenerator = d3.axisRight(yRight).ticks(5);
    if ((_g = axes.right) == null ? void 0 : _g.format) {
      const formatter = createFormatter(axes.right.format, "number");
      yAxisRightGenerator = yAxisRightGenerator.tickFormat(formatter);
    }
    const yAxisRight = g.append("g").attr("transform", `translate(${chartWidth}, 0)`).style("color", "var(--chartml-axis-line)").call(yAxisRightGenerator);
    yAxisRight.selectAll("text").style("font-size", AXIS_LABEL_FONT_SIZE).style("font-family", AXIS_LABEL_FONT_FAMILY);
    let maxRightTickWidth = 0;
    yAxisRight.selectAll("text").each(function() {
      const bbox = this.getBBox();
      maxRightTickWidth = Math.max(maxRightTickWidth, bbox.width);
    });
    if ((_h = axes.right) == null ? void 0 : _h.label) {
      const gap = 10;
      const labelOffset = chartWidth + maxRightTickWidth + gap + 15;
      g.append("text").attr("transform", "rotate(-90)").attr("x", -chartHeight / 2).attr("y", labelOffset).attr("text-anchor", "middle").style("font-size", "14px").style("font-family", AXIS_LABEL_FONT_FAMILY).style("fill", "var(--chartml-text)").text(axes.right.label);
    }
  }
}
function addGridLines(g, scales, chartWidth, chartHeight, gridConfig = {}) {
  const { x, yLeft } = scales;
  const config = {
    x: gridConfig.x !== void 0 ? gridConfig.x : false,
    // Vertical grid lines off by default
    y: gridConfig.y !== void 0 ? gridConfig.y : true,
    // Horizontal grid lines on by default
    color: gridConfig.color || "var(--chartml-grid)",
    opacity: gridConfig.opacity !== void 0 ? gridConfig.opacity : 0.5,
    dashArray: gridConfig.dashArray || null
  };
  if (config.y && yLeft) {
    const gridY = g.append("g").attr("class", "grid grid-y").call(
      d3.axisLeft(yLeft).tickSize(-chartWidth).tickFormat("")
    );
    gridY.selectAll("line").style("stroke", config.color).style("stroke-opacity", config.opacity);
    if (config.dashArray) {
      gridY.selectAll("line").style("stroke-dasharray", config.dashArray);
    }
    gridY.select(".domain").style("opacity", 0);
  }
  if (config.x && x) {
    const gridX = g.append("g").attr("class", "grid grid-x").attr("transform", `translate(0,${chartHeight})`).call(
      d3.axisBottom(x).tickSize(-chartHeight).tickFormat("")
    );
    gridX.selectAll("line").style("stroke", config.color).style("stroke-opacity", config.opacity);
    if (config.dashArray) {
      gridX.selectAll("line").style("stroke-dasharray", config.dashArray);
    }
    gridX.select(".domain").style("opacity", 0);
  }
}
function addDataLabels(g, data, row, x, yScale, chartHeight, xField, isDateScale, animation) {
  if (!row.dataLabels || !row.dataLabels.show) {
    return;
  }
  const config = row.dataLabels;
  const position = config.position || "top";
  const format = config.format || null;
  const fontSize = config.fontSize || 12;
  const color = config.color || "var(--chartml-text)";
  const formatter = format ? createFormatter(format, "number") : (v) => v.toLocaleString();
  const labels = g.selectAll(`.label-${row.field}`).data(data).join("text").attr("class", `label label-${row.field}`).attr("x", (d) => {
    if (isDateScale) {
      return x(d[xField]);
    } else {
      return x(d[xField]) + x.bandwidth() / 2;
    }
  }).attr("y", (d) => {
    const value = d[row.field] || 0;
    const yPos = yScale(value);
    if (row.mark === "bar") {
      if (position === "top") {
        return yPos - 5;
      } else if (position === "center") {
        return yPos + (chartHeight - yPos) / 2;
      } else if (position === "end") {
        return chartHeight - 5;
      }
    } else if (row.mark === "line" || row.mark === "area") {
      if (position === "top") {
        return yPos - 5;
      } else if (position === "center") {
        return yPos;
      }
    }
    return yPos - 5;
  }).attr("text-anchor", "middle").attr("font-size", fontSize).attr("font-family", "system-ui").attr("fill", color).attr("font-weight", "500").text((d) => {
    const value = d[row.field];
    return value != null ? formatter(value) : "";
  });
  labels.attr("opacity", 0).transition().delay(getAnimationDuration(200, animation)).duration(getAnimationDuration(200, animation)).attr("opacity", 0.9);
}
function renderBarMark(g, data, row, x, yScale, chartHeight, color, tooltip, container, xField, isDateScale, chartWidth, animation) {
  const maxBarWidth = chartWidth * 0.2;
  let barWidth, xOffset;
  if (isDateScale) {
    barWidth = calculateDateScaleBarWidth(data, xField, chartWidth);
    xOffset = 0;
  } else {
    const bandwidth = x.bandwidth();
    barWidth = Math.min(bandwidth, maxBarWidth);
    xOffset = (bandwidth - barWidth) / 2;
  }
  const sanitizedField = sanitizeClassName(row.field);
  const tooltipFormatter = isDateScale ? getSmartTooltipFormat(data, xField) : null;
  const bars = g.selectAll(`.bar-${sanitizedField}`).data(data).join("rect").attr("class", `bar bar-${sanitizedField}`).attr("x", (d) => {
    if (isDateScale) {
      return x(d[xField]) - barWidth / 2;
    } else {
      return x(d[xField]) + xOffset;
    }
  }).attr("width", barWidth).attr("fill", color).attr("opacity", 0.9).style("cursor", "pointer").on("mouseenter", function(event, d) {
    const barColor = d3.select(this).attr("fill");
    d3.select(this).transition().duration(getAnimationDuration(200, animation)).attr("opacity", 1).attr("stroke", d3.color(barColor).darker(0.5)).attr("stroke-width", 2);
    const xValue = isDateScale ? tooltipFormatter(d[xField]) : d[xField];
    tooltip.style("opacity", 1).html(`<strong>${xValue}</strong><br/>${row.label || row.field}: ${d[row.field].toLocaleString()}`);
  }).on("mousemove", function(event) {
    positionTooltip(tooltip, event);
  }).on("mouseleave", function() {
    d3.select(this).transition().duration(getAnimationDuration(200, animation)).attr("opacity", 0.9).attr("stroke", "none");
    tooltip.style("opacity", 0);
  });
  if (animation) {
    bars.style("pointer-events", "none").attr("y", chartHeight).attr("height", 0).transition().duration(400).attr("y", (d) => yScale(d[row.field] || 0)).attr("height", (d) => chartHeight - yScale(d[row.field] || 0)).on("end", function() {
      d3.select(this).style("pointer-events", "auto");
    });
  } else {
    bars.attr("y", (d) => yScale(d[row.field] || 0)).attr("height", (d) => chartHeight - yScale(d[row.field] || 0));
  }
  addDataLabels(g, data, row, x, yScale, chartHeight, xField, isDateScale, animation);
}
function renderStackedBars(g, data, barRows, x, yScale, chartHeight, colors, tooltip, container, xField, isDateScale, mode, chartWidth, animation) {
  const fields = barRows.map((r) => r.field);
  const maxBarWidth = chartWidth * 0.2;
  let barWidth, xOffset;
  if (isDateScale) {
    barWidth = calculateDateScaleBarWidth(data, xField, chartWidth);
    xOffset = 0;
  } else {
    const bandwidth = x.bandwidth();
    barWidth = Math.min(bandwidth, maxBarWidth);
    xOffset = (bandwidth - barWidth) / 2;
  }
  const tooltipFormatter = isDateScale ? getSmartTooltipFormat(data, xField) : null;
  if (mode === "stacked") {
    const stack = d3.stack().keys(fields).order(d3.stackOrderNone).offset(d3.stackOffsetNone);
    const series = stack(data);
    const groups = g.selectAll("g.stack").data(series).join("g").attr("class", (d) => `stack bar-${sanitizeClassName(d.key)}`).attr("fill", (d, i) => barRows[i].color || colors[i]);
    groups.selectAll("rect").data((d) => d).join("rect").attr("class", function() {
      const key = d3.select(this.parentNode).datum().key;
      return `bar bar-${sanitizeClassName(key)}`;
    }).attr("x", (d) => {
      if (isDateScale) {
        return x(d.data[xField]) - barWidth / 2;
      } else {
        return x(d.data[xField]) + xOffset;
      }
    }).attr("width", barWidth).attr("opacity", 0.9).style("cursor", "pointer").on("mouseenter", function(event, d) {
      const key = d3.select(this.parentNode).datum().key;
      const value = d.data[key];
      const barColor = d3.select(this.parentNode).attr("fill");
      const row = barRows.find((r) => r.field === key);
      d3.select(this).transition().duration(200).attr("opacity", 1).attr("stroke", d3.color(barColor).darker(0.5)).attr("stroke-width", 2);
      const xValue = isDateScale ? tooltipFormatter(d.data[xField]) : d.data[xField];
      tooltip.style("opacity", 1).html(`<strong>${xValue}</strong><br/>${row.label || key}: ${value.toLocaleString()}`);
    }).on("mousemove", function(event) {
      positionTooltip(tooltip, event);
    }).on("mouseleave", function() {
      d3.select(this).transition().duration(200).attr("opacity", 0.9).attr("stroke", "none");
      tooltip.style("opacity", 0);
    });
    const stackedRects = groups.selectAll("rect");
    if (animation) {
      stackedRects.style("pointer-events", "none").attr("y", chartHeight).attr("height", 0).transition().delay((d, i) => i * 10).duration(400).attr("y", (d) => yScale(d[1])).attr("height", (d) => yScale(d[0]) - yScale(d[1])).on("end", function() {
        d3.select(this).style("pointer-events", "auto");
      });
    } else {
      stackedRects.attr("y", (d) => yScale(d[1])).attr("height", (d) => yScale(d[0]) - yScale(d[1]));
    }
  } else {
    const x1 = d3.scaleBand().domain(fields).rangeRound([0, barWidth]).padding(0.05);
    const groups = g.selectAll("g.group").data(data).join("g").attr("class", "group").attr("transform", (d) => {
      if (isDateScale) {
        return `translate(${x(d[xField]) - barWidth / 2}, 0)`;
      } else {
        return `translate(${x(d[xField]) + xOffset}, 0)`;
      }
    });
    groups.selectAll("rect").data((d) => fields.map((key, i) => ({ key, value: d[key], xValue: d[xField], row: barRows[i] }))).join("rect").attr("class", (d) => `bar bar-${sanitizeClassName(d.key)}`).attr("x", (d) => x1(d.key)).attr("width", x1.bandwidth()).attr("fill", (d) => d.row.color || colors[fields.indexOf(d.key)]).attr("opacity", 0.9).style("cursor", "pointer").on("mouseenter", function(event, d) {
      const barColor = d3.select(this).attr("fill");
      d3.select(this).transition().duration(200).attr("opacity", 1).attr("stroke", d3.color(barColor).darker(0.5)).attr("stroke-width", 2);
      const xValue = isDateScale ? tooltipFormatter(d.xValue) : d.xValue;
      tooltip.style("opacity", 1).html(`<strong>${xValue}</strong><br/>${d.row.label || d.key}: ${d.value.toLocaleString()}`);
    }).on("mousemove", function(event) {
      positionTooltip(tooltip, event);
    }).on("mouseleave", function() {
      d3.select(this).transition().duration(200).attr("opacity", 0.9).attr("stroke", "none");
      tooltip.style("opacity", 0);
    });
    const groupedRects = groups.selectAll("rect");
    if (animation) {
      groupedRects.style("pointer-events", "none").attr("y", chartHeight).attr("height", 0).transition().delay((d, i) => i * 10).duration(400).attr("y", (d) => yScale(d.value)).attr("height", (d) => chartHeight - yScale(d.value)).on("end", function() {
        d3.select(this).style("pointer-events", "auto");
      });
    } else {
      groupedRects.attr("y", (d) => yScale(d.value)).attr("height", (d) => chartHeight - yScale(d.value));
    }
  }
  barRows.forEach((row) => {
    var _a;
    if ((_a = row.dataLabels) == null ? void 0 : _a.show) {
      addDataLabels(g, data, row, x, yScale, chartHeight, xField, isDateScale, animation);
    }
  });
}
function renderLineMark(g, data, row, x, yScale, chartHeight, color, tooltip, container, xField, isDateScale, curveType, showDots, animation) {
  const validData = data.filter((d) => {
    const value = d[row.field];
    return value != null && !isNaN(value) && isFinite(value);
  });
  if (validData.length === 0) {
    console.warn(`[renderLineMark] No valid data for field '${row.field}'`);
    return;
  }
  const curve = d3[curveType] || d3.curveLinear;
  const line = d3.line().curve(curve).x((d) => isDateScale ? x(d[xField]) : x(d[xField]) + x.bandwidth() / 2).y((d) => yScale(d[row.field] || 0));
  const sanitizedField = sanitizeClassName(row.field);
  const path = g.append("path").datum(validData).attr("class", `line line-${sanitizedField}`).attr("fill", "none").attr("stroke", color).attr("stroke-width", 3).attr("d", line);
  const lineStyleDash = row.lineStyle ? LINE_STYLE_DASH_PATTERNS[row.lineStyle] || null : null;
  if (animation) {
    const totalLength = path.node().getTotalLength();
    path.attr("stroke-dasharray", totalLength + " " + totalLength).attr("stroke-dashoffset", totalLength).transition().duration(500).ease(d3.easeLinear).attr("stroke-dashoffset", 0).on("end", function() {
      if (lineStyleDash) {
        d3.select(this).attr("stroke-dasharray", lineStyleDash);
      } else {
        d3.select(this).attr("stroke-dasharray", null);
      }
    });
  } else {
    if (lineStyleDash) {
      path.attr("stroke-dasharray", lineStyleDash);
    }
  }
  const tooltipFormatter = isDateScale ? getSmartTooltipFormat(data, xField) : null;
  const hoverTargets = g.selectAll(`.hover-target-${sanitizedField}`).data(validData).join("circle").attr("class", `hover-target hover-target-${sanitizedField}`).attr("cx", (d) => isDateScale ? x(d[xField]) : x(d[xField]) + x.bandwidth() / 2).attr("cy", (d) => yScale(d[row.field])).attr("r", 8).attr("fill", "transparent").attr("opacity", 0).style("cursor", "pointer").on("mouseenter", function(event, d) {
    if (showDots) {
      const correspondingDot = g.select(`.dot-${sanitizedField}[data-index="${d3.select(this).attr("data-index")}"]`);
      correspondingDot.transition().duration(getAnimationDuration(200, animation)).attr("r", 7).attr("opacity", 1);
    } else {
      d3.select(this).transition().duration(getAnimationDuration(200, animation)).attr("r", 5).attr("fill", color).style("stroke", "var(--chartml-separator)").attr("stroke-width", 2).attr("opacity", 1);
    }
    const xValue = isDateScale ? tooltipFormatter(d[xField]) : d[xField];
    tooltip.style("opacity", 1).html(`<strong>${xValue}</strong><br/>${row.label || row.field}: ${d[row.field].toLocaleString()}`);
  }).on("mousemove", function(event) {
    positionTooltip(tooltip, event);
  }).on("mouseleave", function() {
    if (showDots) {
      const correspondingDot = g.select(`.dot-${sanitizedField}[data-index="${d3.select(this).attr("data-index")}"]`);
      correspondingDot.transition().duration(getAnimationDuration(200, animation)).attr("r", 5).attr("opacity", 0.9);
    } else {
      d3.select(this).transition().duration(getAnimationDuration(200, animation)).attr("r", 8).attr("fill", "transparent").attr("stroke", "none").attr("opacity", 0);
    }
    tooltip.style("opacity", 0);
  });
  hoverTargets.each(function(d, i) {
    d3.select(this).attr("data-index", i);
  });
  if (showDots) {
    const dots = g.selectAll(`.dot-${sanitizedField}`).data(validData).join("circle").attr("class", `dot dot-${sanitizedField}`).attr("cx", (d) => isDateScale ? x(d[xField]) : x(d[xField]) + x.bandwidth() / 2).attr("cy", (d) => yScale(d[row.field])).attr("r", animation ? 0 : 5).attr("fill", color).style("stroke", "var(--chartml-separator)").attr("stroke-width", 2).attr("opacity", 0.9).style("pointer-events", "none").each(function(d, i) {
      d3.select(this).attr("data-index", i);
    });
    if (animation) {
      const dotDelay = validData.length > 50 ? () => 500 : (_, i) => 500 + i * 20;
      dots.transition().delay(dotDelay).duration(200).attr("r", 5);
    }
  }
  addDataLabels(g, validData, row, x, yScale, chartHeight, xField, isDateScale, animation);
}
function renderAreaMarks(g, data, areaRows, x, yScale, chartHeight, colors, xField, isDateScale, curveType, fillOpacity, mode, animation) {
  const curve = d3[curveType] || d3.curveLinear;
  const fields = areaRows.map((r) => r.field);
  if (mode === "stacked" && fields.length > 1) {
    const stack = d3.stack().keys(fields).order(d3.stackOrderNone).offset(d3.stackOffsetNone);
    const series = stack(data);
    const area = d3.area().curve(curve).x((d) => isDateScale ? x(d.data[xField]) : x(d.data[xField]) + x.bandwidth() / 2).y0((d) => yScale(d[0])).y1((d) => yScale(d[1]));
    series.forEach((s, index) => {
      const sanitizedField = sanitizeClassName(areaRows[index].field);
      const path = g.append("path").datum(s).attr("class", `area area-${sanitizedField}`).attr("fill", areaRows[index].color || colors[index]).attr("opacity", fillOpacity).attr("d", area).on("mouseenter", function() {
        d3.select(this).transition().duration(200).attr("opacity", fillOpacity + 0.2);
      }).on("mouseleave", function() {
        d3.select(this).transition().duration(200).attr("opacity", fillOpacity);
      });
      if (animation) {
        path.attr("opacity", 0).transition().delay(index * 100).duration(400).attr("opacity", fillOpacity);
      }
    });
  } else if (mode === "normalized" && fields.length > 1) {
    const stack = d3.stack().keys(fields).order(d3.stackOrderNone).offset(d3.stackOffsetExpand);
    const series = stack(data);
    const area = d3.area().curve(curve).x((d) => isDateScale ? x(d.data[xField]) : x(d.data[xField]) + x.bandwidth() / 2).y0((d) => yScale(d[0])).y1((d) => yScale(d[1]));
    series.forEach((s, index) => {
      const sanitizedField = sanitizeClassName(areaRows[index].field);
      const path = g.append("path").datum(s).attr("class", `area area-${sanitizedField}`).attr("fill", areaRows[index].color || colors[index]).attr("opacity", fillOpacity).attr("d", area).on("mouseenter", function() {
        d3.select(this).transition().duration(200).attr("opacity", fillOpacity + 0.2);
      }).on("mouseleave", function() {
        d3.select(this).transition().duration(200).attr("opacity", fillOpacity);
      });
      if (animation) {
        path.attr("opacity", 0).transition().delay(index * 100).duration(400).attr("opacity", fillOpacity);
      }
    });
  } else {
    areaRows.forEach((row, index) => {
      const sanitizedField = sanitizeClassName(row.field);
      const area = d3.area().curve(curve).x((d) => isDateScale ? x(d[xField]) : x(d[xField]) + x.bandwidth() / 2).y0(chartHeight).y1((d) => yScale(d[row.field] || 0));
      const path = g.append("path").datum(data).attr("class", `area area-${sanitizedField}`).attr("fill", row.color || colors[index]).attr("opacity", fillOpacity).attr("d", area).on("mouseenter", function() {
        d3.select(this).transition().duration(200).attr("opacity", fillOpacity + 0.2);
      }).on("mouseleave", function() {
        d3.select(this).transition().duration(200).attr("opacity", fillOpacity);
      });
      if (animation) {
        path.attr("opacity", 0).transition().delay(index * 100).duration(400).attr("opacity", fillOpacity);
      }
    });
  }
}
function renderRangeMark(g, data, rangeRow, x, yScale, xField, isDateScale, curveType, color, animation) {
  const curve = d3[curveType] || d3.curveLinear;
  const fillOpacity = rangeRow.opacity || 0.15;
  const area = d3.area().curve(curve).x((d) => isDateScale ? x(d[xField]) : x(d[xField]) + x.bandwidth() / 2).y0((d) => {
    let val = d[rangeRow.lower] || 0;
    if (rangeRow.floor != null) val = Math.max(val, rangeRow.floor);
    return yScale(val);
  }).y1((d) => {
    let val = d[rangeRow.upper] || 0;
    if (rangeRow.ceiling != null) val = Math.min(val, rangeRow.ceiling);
    return yScale(val);
  });
  const validData = data.filter((d) => {
    const upper = d[rangeRow.upper];
    const lower = d[rangeRow.lower];
    return upper != null && !isNaN(upper) && lower != null && !isNaN(lower);
  });
  if (validData.length === 0) return;
  const sanitizedLabel = sanitizeClassName(rangeRow.label || "unlabeled");
  const path = g.append("path").datum(validData).attr("class", `range-area range-${sanitizedLabel}`).attr("fill", color).attr("fill-opacity", fillOpacity).attr("stroke", "none").attr("d", area).style("pointer-events", "none");
  if (animation) {
    path.attr("fill-opacity", 0).transition().duration(400).attr("fill-opacity", fillOpacity);
  }
}
function addLegend(svg, rows, colors, marginLeft, height, marginBottom, chartWidth, legendSpace, axes, animation) {
  var _a;
  if (rows.length <= 1) return null;
  const hasXAxisLabel = (_a = axes == null ? void 0 : axes.x) == null ? void 0 : _a.label;
  const xAxisLabelHeight = hasXAxisLabel ? 20 : 0;
  const gap = 5;
  const legendY = height - legendSpace - xAxisLabelHeight - gap;
  const legendItems = rows.map((row, idx) => ({
    label: row.label || row.field,
    color: row.color || colors[idx],
    mark: row.mark || "bar",
    field: row.mark === "range" ? row.label || "unlabeled" : row.field,
    lineStyle: row.lineStyle || null,
    opacity: row.opacity || null,
    index: idx
  })).filter((item) => !(item.mark === "range" && !rows[item.index].label));
  const getSeriesElements = (field) => {
    const sanitized = sanitizeClassName(field);
    const selector = `.bar-${sanitized}, .line-${sanitized}, .area-${sanitized}, .dots-${sanitized}, .range-${sanitized}`;
    return svg.selectAll(selector);
  };
  return createLegend(svg, legendItems, {
    x: marginLeft,
    y: legendY,
    width: chartWidth,
    align: "center",
    maxRows: 3,
    onItemHover: (item) => {
      legendItems.forEach((otherItem) => {
        const elements = getSeriesElements(otherItem.field);
        if (otherItem.field === item.field) {
          elements.transition().duration(getAnimationDuration(200, animation)).style("opacity", 1);
        } else {
          elements.each(function() {
            const el = d3.select(this);
            if (!el.attr("data-original-opacity")) {
              const currentOpacity = parseFloat(el.attr("opacity") || el.style("opacity")) || 0.9;
              el.attr("data-original-opacity", currentOpacity);
            }
            el.transition().duration(getAnimationDuration(200, animation)).style("opacity", 0.3);
          });
        }
      });
    },
    onItemLeave: () => {
      legendItems.forEach((item) => {
        const elements = getSeriesElements(item.field);
        elements.each(function() {
          const el = d3.select(this);
          const originalOpacity = parseFloat(el.attr("data-original-opacity")) || 0.9;
          el.transition().duration(getAnimationDuration(200, animation)).style("opacity", originalOpacity);
        });
      });
    }
  });
}
function renderAnnotationLine(g, annotation, scales, chartWidth, chartHeight, marginLeft, marginTop, isDateScale) {
  const { axis, value, label, labelPosition = "end", color = "var(--chartml-annotation)", strokeWidth = 1, dashArray, opacity = 1 } = annotation;
  let x1, y1, x2, y2;
  let scale;
  if (axis === "x") {
    scale = scales.x;
    let scaledValue = value;
    if (isDateScale && typeof value === "string") {
      scaledValue = new Date(value);
    }
    const xPos = scale(scaledValue);
    if (xPos === void 0) return;
    x1 = x2 = xPos;
    y1 = 0;
    y2 = chartHeight;
  } else if (axis === "left" || axis === "right") {
    scale = axis === "left" ? scales.yLeft : scales.yRight;
    if (!scale) return;
    const yPos = scale(value);
    if (yPos === void 0) return;
    x1 = 0;
    x2 = chartWidth;
    y1 = y2 = yPos;
  } else {
    return;
  }
  const line = g.append("line").attr("x1", x1).attr("y1", y1).attr("x2", x2).attr("y2", y2).attr("stroke", color).attr("stroke-width", strokeWidth).attr("opacity", opacity).style("pointer-events", "none");
  if (dashArray) {
    line.attr("stroke-dasharray", dashArray);
  }
  if (label) {
    let textX, textY, textAnchor = "middle";
    if (axis === "x") {
      textX = x1;
      if (labelPosition === "start") {
        textY = 0 - 5;
        textAnchor = "middle";
      } else if (labelPosition === "center") {
        textY = chartHeight / 2;
        textAnchor = "middle";
      } else {
        textY = chartHeight + 15;
        textAnchor = "middle";
      }
    } else {
      textY = y1 - 5;
      if (labelPosition === "start") {
        textX = 5;
        textAnchor = "start";
      } else if (labelPosition === "center") {
        textX = chartWidth / 2;
        textAnchor = "middle";
      } else {
        textX = chartWidth - 5;
        textAnchor = "end";
      }
    }
    g.append("text").attr("x", textX).attr("y", textY).attr("text-anchor", textAnchor).attr("font-size", AXIS_LABEL_FONT_SIZE).attr("font-family", "system-ui, -apple-system, sans-serif").attr("fill", color).style("pointer-events", "none").text(label);
  }
}
function renderAnnotationBand(g, annotation, scales, chartWidth, chartHeight, marginLeft, marginTop, isDateScale) {
  const { axis, from, to, label, color = "var(--chartml-annotation)", opacity = 0.2, strokeColor, strokeWidth = 0 } = annotation;
  let x, y, width, height;
  let scale;
  if (axis === "x") {
    scale = scales.x;
    let scaledFrom = from;
    let scaledTo = to;
    if (isDateScale) {
      if (typeof from === "string") scaledFrom = new Date(from);
      if (typeof to === "string") scaledTo = new Date(to);
    }
    const xPosFrom = scale(scaledFrom);
    const xPosTo = scale(scaledTo);
    if (xPosFrom === void 0 || xPosTo === void 0) return;
    x = Math.min(xPosFrom, xPosTo);
    width = Math.abs(xPosTo - xPosFrom);
    y = 0;
    height = chartHeight;
  } else if (axis === "left" || axis === "right") {
    scale = axis === "left" ? scales.yLeft : scales.yRight;
    if (!scale) return;
    const yPosFrom = scale(from);
    const yPosTo = scale(to);
    if (yPosFrom === void 0 || yPosTo === void 0) return;
    x = 0;
    width = chartWidth;
    y = Math.min(yPosFrom, yPosTo);
    height = Math.abs(yPosTo - yPosFrom);
  } else {
    return;
  }
  const rect = g.append("rect").attr("x", x).attr("y", y).attr("width", width).attr("height", height).attr("fill", color).attr("opacity", opacity).style("pointer-events", "none");
  if (strokeColor && strokeWidth > 0) {
    rect.attr("stroke", strokeColor).attr("stroke-width", strokeWidth);
  }
  if (label) {
    let textX, textY;
    if (axis === "x") {
      textX = x + width / 2;
      textY = 15;
    } else {
      textX = 10;
      textY = y + height / 2;
    }
    g.append("text").attr("x", textX).attr("y", textY).attr("text-anchor", axis === "x" ? "middle" : "start").attr("font-size", AXIS_LABEL_FONT_SIZE).attr("font-family", "system-ui, -apple-system, sans-serif").attr("fill", color).attr("opacity", Math.min(opacity * 3, 1)).style("pointer-events", "none").text(label);
  }
}
function renderHorizontalBarChart(container, data, config) {
  var _a, _b, _c, _d;
  const {
    xField = "x",
    rows = [],
    mode = "stacked",
    width = 600,
    height = 400,
    marginTop = 20,
    marginRight = 30,
    axes = {},
    colors,
    // REQUIRED - no default
    animation = true
    // Enable animations by default (backward compatible)
  } = config;
  if (!colors || !Array.isArray(colors)) {
    throw new Error("Horizontal bar chart config missing colors array. Ensure style resolution includes palette colors.");
  }
  let marginLeft;
  if (config.marginLeft !== void 0) {
    marginLeft = config.marginLeft;
  } else {
    const labels = data.map((d) => String(d[xField]));
    const labelWidths = measureLabelWidths$1(labels, "12px", "system-ui");
    const maxLabelWidth = Math.max(...labelWidths);
    marginLeft = Math.min(Math.ceil(maxLabelWidth) + 15, 250);
  }
  const hasLegend = rows.length > 1;
  let marginBottom;
  if (config.marginBottom !== void 0) {
    marginBottom = config.marginBottom;
  } else {
    const hasXAxisLabel = ((_a = axes.x) == null ? void 0 : _a.label) || ((_b = axes.left) == null ? void 0 : _b.label);
    const baseMargin = hasXAxisLabel ? 50 : 30;
    marginBottom = hasLegend ? baseMargin + 50 : baseMargin;
  }
  container.innerHTML = "";
  const chartWidth = width - marginLeft - marginRight;
  const chartHeight = height - marginTop - marginBottom;
  const svg = d3.select(container).append("svg").attr("width", "100%").attr("height", height).attr("viewBox", [0, 0, width, height]).style("max-width", "100%").style("display", "block");
  const g = svg.append("g").attr("transform", `translate(${marginLeft},${marginTop})`);
  const normalizedRows = rows.map((row, idx) => ({
    ...row,
    color: row.color || colors[idx % colors.length],
    label: row.label || row.field
  }));
  const leftRows = normalizedRows.filter((r) => !r.axis || r.axis === "left");
  const leftFields = leftRows.map((r) => r.field);
  let xMin, xMax;
  if (mode === "stacked" && leftRows.length > 1) {
    xMin = 0;
    xMax = d3.max(data, (d) => d3.sum(leftFields, (field) => d[field] || 0));
  } else {
    const allValues = data.flatMap((d) => leftFields.map((field) => d[field] || 0));
    xMin = Math.min(0, d3.min(allValues) || 0);
    xMax = d3.max(allValues) || 1;
  }
  const x = d3.scaleLinear().domain([xMin, xMax]).nice().range([0, chartWidth]);
  const barCount = data.length;
  let padding;
  if (barCount <= 10) {
    padding = 0.2;
  } else if (barCount <= 20) {
    padding = 0.15;
  } else if (barCount <= 40) {
    padding = 0.1;
  } else {
    padding = 0.05;
  }
  const y = d3.scaleBand().domain(data.map((d) => d[xField])).range([0, chartHeight]).padding(padding);
  const xAxisFormat = (_c = axes.x) == null ? void 0 : _c.format;
  const xFormatter = xAxisFormat ? createFormatter(xAxisFormat, "auto") : null;
  const xDomain = x.domain();
  const maxValue = xDomain[1];
  const sampleLabel = xFormatter ? xFormatter(maxValue) : maxValue.toLocaleString();
  const estimatedLabelWidth = measureLabelWidths$1([sampleLabel], AXIS_LABEL_FONT_SIZE, AXIS_LABEL_FONT_FAMILY)[0];
  const minLabelSpacing = estimatedLabelWidth + 20;
  const maxFittableTicks = Math.max(2, Math.floor(chartWidth / minLabelSpacing));
  const tickCount = Math.min(5, maxFittableTicks);
  const hBarXAxis = g.append("g").attr("transform", `translate(0,${chartHeight})`).style("color", "var(--chartml-axis-line)").call(d3.axisBottom(x).ticks(tickCount).tickFormat(xFormatter || ((d) => d)));
  hBarXAxis.selectAll("text").style("font-size", AXIS_LABEL_FONT_SIZE).style("font-family", AXIS_LABEL_FONT_FAMILY);
  const xLabel = (_d = axes.x) == null ? void 0 : _d.label;
  if (xLabel) {
    g.append("text").attr("x", chartWidth / 2).attr("y", chartHeight + marginBottom - 5).attr("text-anchor", "middle").style("font-size", "14px").style("font-family", AXIS_LABEL_FONT_FAMILY).style("fill", "var(--chartml-text)").text(xLabel);
  }
  const hBarYAxis = g.append("g").style("color", "var(--chartml-axis-line)").call(d3.axisLeft(y));
  hBarYAxis.selectAll("text").style("font-size", AXIS_LABEL_FONT_SIZE).style("font-family", AXIS_LABEL_FONT_FAMILY);
  const hBarGrid = g.append("g").attr("class", "grid").call(d3.axisBottom(x).tickSize(chartHeight).tickFormat(""));
  hBarGrid.selectAll("line").style("stroke", "var(--chartml-grid)").style("stroke-opacity", 0.5);
  hBarGrid.select(".domain").style("opacity", 0);
  const tooltip = createChartTooltip(container);
  const bandwidth = y.bandwidth();
  const maxBarHeight = 40;
  const barHeight = Math.min(bandwidth, maxBarHeight);
  const yOffset = (bandwidth - barHeight) / 2;
  if (leftRows.length > 1 && mode === "stacked") {
    const series = d3.stack().keys(leftFields).value((d, key) => d[key] || 0)(data);
    series.forEach((serie, i) => {
      const row = leftRows[i];
      g.selectAll(`.bar-${sanitizeClassName(row.field)}`).data(serie).join("rect").attr("class", `bar bar-${sanitizeClassName(row.field)}`).attr("y", (d) => y(d.data[xField]) + yOffset).attr("x", (d) => x(d[0])).attr("width", (d) => x(d[1]) - x(d[0])).attr("height", barHeight).attr("fill", row.color).style("opacity", 0.9).style("cursor", "pointer").on("mouseenter", function(event, d) {
        d3.select(this).style("opacity", 1);
        tooltip.style("opacity", 1).html(`<strong>${row.label}</strong><br/>${d.data[row.field].toLocaleString()}`);
      }).on("mousemove", function(event) {
        positionTooltip(tooltip, event);
      }).on("mouseleave", function() {
        d3.select(this).style("opacity", 0.9);
        tooltip.style("opacity", 0);
      });
    });
  } else if (leftRows.length > 1 && mode === "grouped") {
    const ySubgroup = d3.scaleBand().domain(leftFields).range([0, barHeight]).padding(0.05);
    leftRows.forEach((row) => {
      g.selectAll(`.bar-${sanitizeClassName(row.field)}`).data(data).join("rect").attr("class", `bar bar-${sanitizeClassName(row.field)}`).attr("y", (d) => y(d[xField]) + yOffset + ySubgroup(row.field)).attr("x", 0).attr("width", (d) => x(d[row.field] || 0)).attr("height", ySubgroup.bandwidth()).attr("fill", row.color).style("opacity", 0.9).style("cursor", "pointer").on("mouseenter", function(event, d) {
        d3.select(this).style("opacity", 1);
        tooltip.style("opacity", 1).html(`<strong>${row.label}</strong><br/>${d[row.field].toLocaleString()}`);
      }).on("mousemove", function(event) {
        positionTooltip(tooltip, event);
      }).on("mouseleave", function() {
        d3.select(this).style("opacity", 0.9);
        tooltip.style("opacity", 0);
      });
    });
  } else if (leftRows.length === 1) {
    const row = leftRows[0];
    g.selectAll(".bar").data(data).join("rect").attr("y", (d) => y(d[xField]) + yOffset).attr("x", 0).attr("width", (d) => x(d[row.field] || 0)).attr("height", barHeight).attr("fill", row.color).style("opacity", 0.9).style("cursor", "pointer").on("mouseenter", function(event, d) {
      d3.select(this).style("opacity", 1);
      tooltip.style("opacity", 1).html(`<strong>${d[xField]}</strong><br/>${d[row.field].toLocaleString()}`);
    }).on("mousemove", function(event) {
      positionTooltip(tooltip, event);
    }).on("mouseleave", function() {
      d3.select(this).style("opacity", 0.9);
      tooltip.style("opacity", 0);
    });
  }
  if (normalizedRows.length > 1) {
    const legendY = marginTop + chartHeight + 35;
    const legendItems = normalizedRows.map((row, idx) => ({
      label: row.label,
      color: row.color,
      mark: "bar",
      field: row.field,
      index: idx
    }));
    const getBarsByField = (field) => {
      const sanitized = sanitizeClassName(field);
      return svg.selectAll(`.bar-${sanitized}`);
    };
    createLegend(svg, legendItems, {
      x: marginLeft,
      y: legendY,
      width: chartWidth,
      align: "center",
      maxRows: 2,
      onItemHover: (item) => {
        legendItems.forEach((otherItem) => {
          const bars = getBarsByField(otherItem.field);
          if (otherItem.field === item.field) {
            bars.transition().duration(200).style("opacity", 1);
          } else {
            bars.each(function() {
              const el = d3.select(this);
              if (!el.attr("data-original-opacity")) {
                const currentOpacity = parseFloat(el.attr("opacity") || el.style("opacity")) || 0.9;
                el.attr("data-original-opacity", currentOpacity);
              }
              el.transition().duration(getAnimationDuration(200, animation)).style("opacity", 0.3);
            });
          }
        });
      },
      onItemLeave: () => {
        legendItems.forEach((item) => {
          const bars = getBarsByField(item.field);
          bars.each(function() {
            const el = d3.select(this);
            const originalOpacity = parseFloat(el.attr("data-original-opacity")) || 0.9;
            el.transition().duration(getAnimationDuration(200, animation)).style("opacity", originalOpacity);
          });
        });
      }
    });
  }
}
function renderD3CartesianChart(container, data, config) {
  var _a, _b, _c, _d, _e, _f, _g, _h, _i, _j, _k, _l;
  const orientation = config.orientation || "vertical";
  const type = config.type || "bar";
  if (orientation === "horizontal" && type === "bar") {
    return renderHorizontalBarChart(container, data, config);
  }
  const {
    xField = "x",
    rows = [],
    mode = "stacked",
    // 'stacked' or 'grouped' for multiple bars/areas
    width = 600,
    height = 400,
    marginTop = 20,
    marginRight = 30,
    marginLeft: configMarginLeft,
    axes = {},
    colors,
    // REQUIRED - no default
    curveType = "curveMonotoneX",
    showDots = true,
    fillOpacity = 0.6,
    annotations = [],
    animation = true
    // Enable animations by default (backward compatible)
  } = config;
  if (!colors || !Array.isArray(colors)) {
    throw new Error("Cartesian chart config missing colors array. Ensure style resolution includes palette colors.");
  }
  const marginLeft = configMarginLeft !== void 0 ? configMarginLeft : 70;
  data.length;
  const hasLegend = rows.length > 1;
  const legendSpace = hasLegend ? 30 : 0;
  const hasDateValues = data.length > 0 && data[0][xField] instanceof Date;
  let marginBottom;
  let labelRotationDegrees = 0;
  if (config.marginBottom !== void 0) {
    marginBottom = config.marginBottom;
  } else {
    const hasXAxisLabel = (_a = axes.x) == null ? void 0 : _a.label;
    const axisLabelSpace = hasXAxisLabel ? 20 : 0;
    const tempChartWidth = width - marginLeft - marginRight;
    let tempXScale;
    if (hasDateValues) {
      const uniqueDates = new Set(data.map((d) => d[xField].getTime()));
      const dataPointCount = uniqueDates.size;
      let inset = 30;
      if (dataPointCount >= 2) {
        inset = tempChartWidth / (2 * dataPointCount);
      }
      tempXScale = d3.scaleUtc().domain(d3.extent(data, (d) => d[xField])).range([inset, tempChartWidth - inset]);
    } else {
      tempXScale = d3.scaleBand().domain(data.map((d) => d[xField])).range([0, tempChartWidth]).padding(0.2);
    }
    const { marginNeeded, rotationDegrees } = calculateXAxisMargin(
      data,
      xField,
      tempXScale,
      tempChartWidth
    );
    labelRotationDegrees = rotationDegrees;
    marginBottom = marginNeeded + legendSpace + axisLabelSpace;
  }
  const normalizedRows = rows.map((row, idx) => ({
    ...row,
    mark: row.mark || type,
    // Default to chart type
    axis: row.axis || "left",
    // Default to left axis
    color: row.color || colors[idx % colors.length],
    label: row.label || row.field
  }));
  const leftRows = normalizedRows.filter((r) => r.axis === "left" && r.mark !== "range");
  const rightRows = normalizedRows.filter((r) => r.axis === "right" && r.mark !== "range");
  const leftRangeRows = normalizedRows.filter((r) => r.axis === "left" && r.mark === "range");
  const rightRangeRows = normalizedRows.filter((r) => r.axis === "right" && r.mark === "range");
  let finalMarginLeft = marginLeft;
  if (leftRows.length > 0) {
    const tempChartHeight = height - marginTop - marginBottom;
    const leftFields = leftRows.map((r) => r.field);
    const leftMarks = leftRows.map((r) => r.mark || type);
    const areaCount = leftMarks.filter((m) => m === "area").length;
    const barCount = leftMarks.filter((m) => m === "bar").length;
    const hasNormalizedAreas = areaCount > 1 && mode === "normalized";
    const hasStackedBars = barCount > 1 && mode === "stacked";
    const hasStackedAreas = areaCount > 1 && mode === "stacked";
    let yLeftMin, yLeftMax;
    if (hasNormalizedAreas) {
      yLeftMin = 0;
      yLeftMax = 1;
    } else if (hasStackedBars || hasStackedAreas) {
      yLeftMin = 0;
      yLeftMax = d3.max(data, (d) => d3.sum(leftFields, (field) => d[field] || 0));
    } else {
      const allLeftValues = data.flatMap((d) => leftFields.map((field) => d[field] || 0));
      yLeftMin = Math.min(0, d3.min(allLeftValues) || 0);
      yLeftMax = d3.max(allLeftValues) || 1;
    }
    if (leftRangeRows.length > 0) {
      const rangeValues = data.flatMap(
        (d) => leftRangeRows.flatMap((r) => [d[r.upper] || 0, d[r.lower] || 0])
      );
      const rangeMin = d3.min(rangeValues) || 0;
      const rangeMax = d3.max(rangeValues) || 0;
      yLeftMin = Math.min(yLeftMin, rangeMin);
      yLeftMax = Math.max(yLeftMax, rangeMax);
    }
    if (((_b = axes.left) == null ? void 0 : _b.min) !== void 0) yLeftMin = axes.left.min;
    if (((_c = axes.left) == null ? void 0 : _c.max) !== void 0) yLeftMax = axes.left.max;
    const tempYLeft = d3.scaleLinear().domain([yLeftMin, yLeftMax]).range([tempChartHeight, 0]);
    if (((_d = axes.left) == null ? void 0 : _d.nice) !== false) {
      tempYLeft.nice();
    }
    const ticks = tempYLeft.ticks(5);
    const formatter = hasNormalizedAreas ? d3.format(".0%") : ((_e = axes.left) == null ? void 0 : _e.format) ? createFormatter(axes.left.format) : d3.format(",");
    const tickLabels = ticks.map((t) => formatter(t));
    const labelWidths = measureLabelWidths$1(tickLabels, AXIS_LABEL_FONT_SIZE, AXIS_LABEL_FONT_FAMILY);
    const maxLabelWidth = Math.max(...labelWidths);
    const hasLeftLabel = !!((_f = axes.left) == null ? void 0 : _f.label);
    if (hasLeftLabel) {
      const gap = 10;
      const axisLabelSpace = 30;
      finalMarginLeft = Math.min(maxLabelWidth + gap + axisLabelSpace, 250);
    } else {
      const buffer = 20;
      finalMarginLeft = Math.min(maxLabelWidth + buffer, 250);
    }
  }
  const hasRightAxis = normalizedRows.some((r) => r.axis === "right");
  let finalMarginRight = marginRight;
  if (hasRightAxis) {
    const tempChartHeight = height - marginTop - marginBottom;
    const rightFields = rightRows.map((r) => r.field);
    const allRightValues = data.flatMap((d) => rightFields.map((field) => d[field] || 0));
    let yRightMin = 0;
    let yRightMax = d3.max(allRightValues) || 1;
    if (rightRangeRows.length > 0) {
      const rangeValues = data.flatMap(
        (d) => rightRangeRows.flatMap((r) => [d[r.upper] || 0, d[r.lower] || 0])
      );
      const rangeMin = d3.min(rangeValues) || 0;
      const rangeMax = d3.max(rangeValues) || 0;
      yRightMin = Math.min(yRightMin, rangeMin);
      yRightMax = Math.max(yRightMax, rangeMax);
    }
    if (((_g = axes.right) == null ? void 0 : _g.min) !== void 0) yRightMin = axes.right.min;
    if (((_h = axes.right) == null ? void 0 : _h.max) !== void 0) yRightMax = axes.right.max;
    const tempYRight = d3.scaleLinear().domain([yRightMin, yRightMax]).range([tempChartHeight, 0]);
    if (((_i = axes.right) == null ? void 0 : _i.nice) !== false) {
      tempYRight.nice();
    }
    const ticks = tempYRight.ticks(5);
    const formatter = ((_j = axes.right) == null ? void 0 : _j.format) ? createFormatter(axes.right.format) : d3.format(",");
    const tickLabels = ticks.map((t) => formatter(t));
    const labelWidths = measureLabelWidths$1(tickLabels, AXIS_LABEL_FONT_SIZE, AXIS_LABEL_FONT_FAMILY);
    const maxLabelWidth = Math.max(...labelWidths);
    const hasRightLabel = (_k = axes.right) == null ? void 0 : _k.label;
    const axisLabelSpace = hasRightLabel ? 30 : 0;
    finalMarginRight = Math.min(Math.ceil(maxLabelWidth) + 9 + 15 + axisLabelSpace, 250);
  }
  const { svg, g, chartWidth, chartHeight } = setupSvgContainer(
    container,
    width,
    height,
    marginTop,
    finalMarginRight,
    marginBottom,
    finalMarginLeft
  );
  const { isDateScale } = determineScaleTypes(data, xField);
  const scales = createScales(data, normalizedRows, xField, chartWidth, chartHeight, isDateScale, mode, axes);
  const tooltip = createChartTooltip(container);
  addAxesAndLabels(g, svg, scales, axes, chartWidth, chartHeight, finalMarginLeft, finalMarginRight, marginBottom, isDateScale, mode, container, data, xField, width, labelRotationDegrees);
  addGridLines(g, scales, chartWidth, chartHeight, (_l = config.style) == null ? void 0 : _l.grid);
  leftRangeRows.forEach((rangeRow, idx) => {
    const rangeColor = rangeRow.color || colors[normalizedRows.indexOf(rangeRow) % colors.length];
    renderRangeMark(g, data, rangeRow, scales.x, scales.yLeft, xField, isDateScale, curveType, rangeColor, animation);
  });
  if (scales.yRight) {
    rightRangeRows.forEach((rangeRow, idx) => {
      const rangeColor = rangeRow.color || colors[normalizedRows.indexOf(rangeRow) % colors.length];
      renderRangeMark(g, data, rangeRow, scales.x, scales.yRight, xField, isDateScale, curveType, rangeColor, animation);
    });
  }
  const leftBars = leftRows.filter((r) => r.mark === "bar");
  const leftLines = leftRows.filter((r) => r.mark === "line");
  const leftAreas = leftRows.filter((r) => r.mark === "area");
  if (leftBars.length > 1 && mode) {
    renderStackedBars(g, data, leftBars, scales.x, scales.yLeft, chartHeight, colors, tooltip, container, xField, isDateScale, mode, chartWidth, animation);
  } else if (leftBars.length === 1) {
    renderBarMark(g, data, leftBars[0], scales.x, scales.yLeft, chartHeight, leftBars[0].color, tooltip, container, xField, isDateScale, chartWidth, animation);
  }
  if (leftAreas.length > 0) {
    renderAreaMarks(g, data, leftAreas, scales.x, scales.yLeft, chartHeight, colors, xField, isDateScale, curveType, fillOpacity, mode, animation);
  }
  leftLines.forEach((row) => {
    renderLineMark(g, data, row, scales.x, scales.yLeft, chartHeight, row.color, tooltip, container, xField, isDateScale, curveType, showDots, animation);
  });
  if (scales.yRight) {
    const rightBars = rightRows.filter((r) => r.mark === "bar");
    const rightLines = rightRows.filter((r) => r.mark === "line");
    const rightAreas = rightRows.filter((r) => r.mark === "area");
    rightBars.forEach((row) => {
      renderBarMark(g, data, row, scales.x, scales.yRight, chartHeight, row.color, tooltip, container, xField, isDateScale, chartWidth, animation);
    });
    if (rightAreas.length > 0) {
      renderAreaMarks(g, data, rightAreas, scales.x, scales.yRight, chartHeight, colors, xField, isDateScale, curveType, fillOpacity, mode, animation);
    }
    rightLines.forEach((row) => {
      renderLineMark(g, data, row, scales.x, scales.yRight, chartHeight, row.color, tooltip, container, xField, isDateScale, curveType, showDots, animation);
    });
  }
  if (annotations && annotations.length > 0) {
    const annotationGroup = g.append("g").attr("class", "annotations");
    annotations.forEach((annotation) => {
      if (annotation.type === "line") {
        renderAnnotationLine(annotationGroup, annotation, scales, chartWidth, chartHeight, marginLeft, marginTop, isDateScale);
      } else if (annotation.type === "band") {
        renderAnnotationBand(annotationGroup, annotation, scales, chartWidth, chartHeight, marginLeft, marginTop, isDateScale);
      }
    });
    annotationGroup.lower();
  }
  addLegend(svg, normalizedRows, colors, marginLeft, height, marginBottom, chartWidth, legendSpace, axes, animation);
}
function generateFallbackColor(hex) {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  const rNorm = r / 255;
  const gNorm = g / 255;
  const bNorm = b / 255;
  const max = Math.max(rNorm, gNorm, bNorm);
  const min = Math.min(rNorm, gNorm, bNorm);
  let h, s, l = (max + min) / 2;
  if (max === min) {
    h = s = 0;
  } else {
    const d = max - min;
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    switch (max) {
      case rNorm:
        h = ((gNorm - bNorm) / d + (gNorm < bNorm ? 6 : 0)) / 6;
        break;
      case gNorm:
        h = ((bNorm - rNorm) / d + 2) / 6;
        break;
      case bNorm:
        h = ((rNorm - gNorm) / d + 4) / 6;
        break;
    }
  }
  s = s * 0.6;
  l = l * 0.7 + 0.15;
  const hue2rgb = (p, q, t) => {
    if (t < 0) t += 1;
    if (t > 1) t -= 1;
    if (t < 1 / 6) return p + (q - p) * 6 * t;
    if (t < 1 / 2) return q;
    if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
    return p;
  };
  let newR, newG, newB;
  if (s === 0) {
    newR = newG = newB = l;
  } else {
    const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
    const p = 2 * l - q;
    newR = hue2rgb(p, q, h + 1 / 3);
    newG = hue2rgb(p, q, h);
    newB = hue2rgb(p, q, h - 1 / 3);
  }
  const toHex = (n) => {
    const hex2 = Math.round(n * 255).toString(16);
    return hex2.length === 1 ? "0" + hex2 : hex2;
  };
  return `#${toHex(newR)}${toHex(newG)}${toHex(newB)}`;
}
function generateFallbackColors(baseColors) {
  if (!baseColors || baseColors.length === 0) {
    console.warn("[colorUtils] No base colors provided for fallback generation");
    return [];
  }
  return baseColors.map((color) => generateFallbackColor(color));
}
function getChartColors(seriesCount, basePalette) {
  if (!basePalette || basePalette.length === 0) {
    console.warn("[colorUtils] No base palette provided");
    return [];
  }
  if (seriesCount <= 12) {
    return basePalette.slice(0, seriesCount);
  }
  if (seriesCount <= 24) {
    const fallbacks2 = generateFallbackColors(basePalette);
    return [...basePalette, ...fallbacks2].slice(0, seriesCount);
  }
  const fallbacks = generateFallbackColors(basePalette);
  const fullPalette = [...basePalette, ...fallbacks];
  const colors = [];
  for (let i = 0; i < seriesCount; i++) {
    colors.push(fullPalette[i % fullPalette.length]);
  }
  console.warn(`[colorUtils] Chart has ${seriesCount} series. Consider filtering data, using small multiples, or grouping smaller categories.`);
  return colors;
}
function normalizeField(field, defaultMark) {
  if (typeof field === "string") {
    return {
      field,
      mark: defaultMark,
      axis: "left",
      color: null,
      label: field,
      dataLabels: null
    };
  }
  if (field.mark === "range") {
    if (!field.upper || !field.lower) {
      throw new Error(`Range mark requires both 'upper' and 'lower' fields`);
    }
    return {
      mark: "range",
      upper: field.upper,
      lower: field.lower,
      opacity: field.opacity || 0.15,
      color: field.color || null,
      label: field.label || null,
      axis: field.axis || "left",
      floor: field.floor != null ? field.floor : null,
      ceiling: field.ceiling != null ? field.ceiling : null
    };
  }
  return {
    field: field.field,
    mark: field.mark || defaultMark,
    axis: field.axis || "left",
    color: field.color || null,
    label: field.label || field.field,
    dataLabels: field.dataLabels || null,
    lineStyle: field.lineStyle || null
  };
}
function parseDataDates(data, xField) {
  if (!data || data.length === 0) return data;
  const sampleValue = data[0][xField];
  if (typeof sampleValue === "string" && /^\d{4}-\d{2}-\d{2}/.test(sampleValue)) {
    return data.map((d) => ({
      ...d,
      [xField]: new Date(d[xField])
    }));
  }
  return data;
}
function pivotDataByColor(data, xField, yField, colorField) {
  const sampleValue = data.length > 0 ? data[0][xField] : null;
  const isDateField = sampleValue instanceof Date;
  let xValues;
  if (isDateField) {
    const timestampSet = new Set(data.map((d) => d[xField].getTime()));
    xValues = [...timestampSet].map((ts) => new Date(ts));
  } else {
    xValues = [...new Set(data.map((d) => d[xField]))];
  }
  const colorValues = [...new Set(data.map((d) => d[colorField]))];
  const pivotedData = xValues.map((xValue) => {
    const row = { [xField]: xValue };
    colorValues.forEach((colorValue) => {
      const match = data.find((d) => {
        const xMatch = isDateField ? d[xField].getTime() === xValue.getTime() : d[xField] === xValue;
        return xMatch && d[colorField] === colorValue;
      });
      row[colorValue] = match ? match[yField] : 0;
    });
    return row;
  });
  return { pivotedData, colorValues };
}
function mapToCartesianChart(visualizeSpec, data, instanceConfig = {}) {
  var _a, _b, _c, _d, _e, _f, _g, _h, _i, _j, _k, _l, _m, _n;
  const { type, mode = "stacked", orientation = "vertical", rows, columns, marks = {}, axes = {}, style = {}, annotations = [] } = visualizeSpec;
  const columnField = Array.isArray(columns) ? columns[0] : columns;
  const xField = (typeof columnField === "string" ? columnField : columnField == null ? void 0 : columnField.field) || "x";
  let processedData = parseDataDates(data, xField);
  const rowFields = Array.isArray(rows) ? rows : [rows];
  let normalizedRows;
  if (marks.color && rowFields.length === 1) {
    const yField = typeof rowFields[0] === "string" ? rowFields[0] : rowFields[0].field;
    const { pivotedData, colorValues } = pivotDataByColor(processedData, xField, yField, marks.color);
    processedData = pivotedData;
    normalizedRows = colorValues.map(
      (colorValue, idx) => normalizeField({ field: colorValue, label: colorValue }, type)
    );
  } else {
    normalizedRows = rowFields.map((row, idx) => normalizeField(row, type));
  }
  const basePalette = style.colors || instanceConfig.defaultPalette || [
    "#2E7D9A",
    "#D4A445",
    "#4A7C59",
    "#D66B5B",
    "#8B6BA8",
    "#9BB85A",
    "#A85A6B",
    "#5A6BA8",
    "#B87D5A",
    "#5A9B9B",
    "#759B75",
    "#A8758B"
  ];
  const seriesCount = normalizedRows.length;
  const chartColors = getChartColors(seriesCount, basePalette);
  const isHorizontal = orientation === "horizontal" && type === "bar";
  const resolvedAxes = { ...axes };
  if (axes.columns) {
    const target = isHorizontal ? "left" : "x";
    resolvedAxes[target] = { ...axes.columns, ...resolvedAxes[target] };
  }
  if (axes.rows) {
    const target = isHorizontal ? "x" : "left";
    resolvedAxes[target] = { ...axes.rows, ...resolvedAxes[target] };
  }
  return {
    xField,
    rows: normalizedRows,
    type,
    // Default mark type
    mode,
    // 'stacked' or 'grouped'
    orientation,
    // 'vertical' or 'horizontal' - only applies to bar charts
    width: style.width || ((_a = instanceConfig.dimensions) == null ? void 0 : _a.width) || 600,
    height: style.height || ((_b = instanceConfig.dimensions) == null ? void 0 : _b.height) || 400,
    axes: {
      x: {
        label: ((_c = resolvedAxes.x) == null ? void 0 : _c.label) || "",
        format: (_d = resolvedAxes.x) == null ? void 0 : _d.format
      },
      left: {
        label: ((_e = resolvedAxes.left) == null ? void 0 : _e.label) || "",
        format: (_f = resolvedAxes.left) == null ? void 0 : _f.format,
        min: (_g = resolvedAxes.left) == null ? void 0 : _g.min,
        max: (_h = resolvedAxes.left) == null ? void 0 : _h.max,
        nice: ((_i = resolvedAxes.left) == null ? void 0 : _i.nice) !== false
        // Default true
      },
      right: {
        label: ((_j = resolvedAxes.right) == null ? void 0 : _j.label) || "",
        format: (_k = resolvedAxes.right) == null ? void 0 : _k.format,
        min: (_l = resolvedAxes.right) == null ? void 0 : _l.min,
        max: (_m = resolvedAxes.right) == null ? void 0 : _m.max,
        nice: ((_n = resolvedAxes.right) == null ? void 0 : _n.nice) !== false
        // Default true
      }
    },
    colors: chartColors,
    curveType: style.curveType || "curveMonotoneX",
    showDots: style.showDots !== false,
    // Default true
    fillOpacity: style.fillOpacity || 0.6,
    style: {
      grid: style.grid
      // Pass through grid configuration
    },
    annotations: annotations || [],
    // Pass through annotations (reference lines/bands)
    animation: instanceConfig.animation !== false,
    // Pass through animation setting (default true)
    data: processedData
  };
}
function mapToScatterPlot(visualizeSpec, instanceConfig = {}) {
  var _a, _b;
  const { rows, columns, marks = {}, axes = {}, style = {} } = visualizeSpec;
  const rowField = Array.isArray(rows) ? rows[0] : rows;
  const columnField = Array.isArray(columns) ? columns[0] : columns;
  const yField = (typeof rowField === "string" ? rowField : rowField == null ? void 0 : rowField.field) || "y";
  const xField = (typeof columnField === "string" ? columnField : columnField == null ? void 0 : columnField.field) || "x";
  const bottomAxis = axes.bottom || axes.x || axes.columns || {};
  const leftAxis = axes.left || axes.rows || {};
  const colors = style.colors || instanceConfig.defaultPalette || [
    "#2E7D9A",
    "#D4A445",
    "#4A7C59",
    "#D66B5B",
    "#8B6BA8",
    "#9BB85A",
    "#A85A6B",
    "#5A6BA8",
    "#B87D5A",
    "#5A9B9B",
    "#759B75",
    "#A8758B"
  ];
  return {
    xField,
    yField,
    sizeField: marks.size || null,
    colorField: marks.color || null,
    groupField: marks.group || null,
    labelField: marks.label || null,
    width: style.width || ((_a = instanceConfig.dimensions) == null ? void 0 : _a.width) || 600,
    height: style.height || ((_b = instanceConfig.dimensions) == null ? void 0 : _b.height) || 400,
    xAxisLabel: bottomAxis.label || "",
    yAxisLabel: leftAxis.label || "",
    xMin: bottomAxis.min,
    xMax: bottomAxis.max,
    xNice: bottomAxis.nice !== false,
    yMin: leftAxis.min,
    yMax: leftAxis.max,
    yNice: leftAxis.nice !== false,
    colors,
    radiusRange: [5, 20]
  };
}
function mapToPieChart(visualizeSpec, data, chartType = "pie", instanceConfig = {}) {
  var _a, _b;
  const { rows, columns, style = {} } = visualizeSpec;
  const rowField = Array.isArray(rows) ? rows[0] : rows;
  const columnField = Array.isArray(columns) ? columns[0] : columns;
  const valueField = (typeof rowField === "string" ? rowField : rowField == null ? void 0 : rowField.field) || "value";
  const categoryField = (typeof columnField === "string" ? columnField : columnField == null ? void 0 : columnField.field) || "category";
  const basePalette = style.colors || instanceConfig.defaultPalette || [
    "#2E7D9A",
    "#D4A445",
    "#4A7C59",
    "#D66B5B",
    "#8B6BA8",
    "#9BB85A",
    "#A85A6B",
    "#5A6BA8",
    "#B87D5A",
    "#5A9B9B",
    "#759B75",
    "#A8758B"
  ];
  const sliceCount = data ? data.length : 12;
  const chartColors = getChartColors(sliceCount, basePalette);
  return {
    categoryField,
    valueField,
    type: chartType,
    width: style.width || ((_a = instanceConfig.dimensions) == null ? void 0 : _a.width) || 600,
    height: style.height || ((_b = instanceConfig.dimensions) == null ? void 0 : _b.height) || 400,
    colors: chartColors
  };
}
function mapToMetricCard(visualizeSpec, data) {
  const { value, label, format, compareWith, invertTrend = false, style = {} } = visualizeSpec;
  const dataRow = data[0] || {};
  const currentValue = dataRow[value];
  const previousValue = compareWith ? dataRow[compareWith] : null;
  let comparison = null;
  if (previousValue != null && currentValue != null) {
    const change = currentValue - previousValue;
    const percentChange = change / previousValue * 100;
    let direction = "neutral";
    if (percentChange > 0) {
      direction = "up";
    } else if (percentChange < 0) {
      direction = "down";
    }
    let isGood = null;
    if (direction !== "neutral") {
      if (invertTrend) {
        isGood = direction === "down";
      } else {
        isGood = direction === "up";
      }
    }
    comparison = {
      change,
      percentChange,
      direction,
      // Actual direction: 'up', 'down', or 'neutral'
      isGood
      // Whether this change is good (true), bad (false), or neutral (null)
    };
  }
  return {
    value: currentValue,
    label: label || null,
    // Only show label if explicitly provided
    format: format || null,
    comparison,
    align: style.align || "center",
    // Default to center alignment
    showLabel: !!label
    // Only show label if explicitly provided (ignore style.showLabel legacy)
  };
}
function mapChartMLToD3Config(visualizeSpec, data, title = null, instanceConfig = {}) {
  const { type } = visualizeSpec;
  switch (type) {
    case "bar":
    case "line":
    case "area": {
      const mapped = mapToCartesianChart(visualizeSpec, data, instanceConfig);
      return {
        chartType: "cartesian",
        config: { ...mapped, title },
        data: mapped.data
      };
    }
    case "scatter":
      return {
        chartType: "scatter",
        config: { ...mapToScatterPlot(visualizeSpec, instanceConfig), title },
        data
      };
    case "pie":
    case "doughnut":
      return {
        chartType: type,
        config: { ...mapToPieChart(visualizeSpec, data, type, instanceConfig), title },
        data
      };
    case "table":
      return {
        chartType: "table",
        config: { spec: visualizeSpec, title },
        data
      };
    case "metric":
      return {
        chartType: "metric",
        config: { ...mapToMetricCard(visualizeSpec, data), title },
        data
      };
    default:
      throw new Error(`Unknown chart type: ${type}`);
  }
}
function deepMerge(target, source) {
  if (!source || typeof source !== "object") {
    return target;
  }
  const result = { ...target };
  for (const key in source) {
    if (source.hasOwnProperty(key)) {
      const sourceValue = source[key];
      const targetValue = result[key];
      if (Array.isArray(sourceValue)) {
        result[key] = [...sourceValue];
      } else if (sourceValue !== null && typeof sourceValue === "object" && !Array.isArray(sourceValue) && targetValue !== null && typeof targetValue === "object" && !Array.isArray(targetValue)) {
        result[key] = deepMerge(targetValue, sourceValue);
      } else {
        result[key] = sourceValue;
      }
    }
  }
  return result;
}
const SYSTEM_DEFAULTS = {
  theme: {
    colors: [
      "#E67E22",
      // Orange
      "#3498DB",
      // Blue
      "#2ECC71",
      // Green
      "#9B59B6",
      // Purple
      "#E74C3C",
      // Red
      "#1ABC9C",
      // Turquoise
      "#F39C12",
      // Yellow
      "#34495E"
      // Dark gray
    ],
    background: "#FFFFFF",
    fonts: {
      title: {
        size: 16,
        weight: 600,
        family: "system-ui, -apple-system, sans-serif",
        color: "#374151"
      },
      axis: {
        size: 12,
        family: "system-ui, -apple-system, sans-serif",
        color: "#6B7280"
      },
      legend: {
        size: 12,
        family: "system-ui, -apple-system, sans-serif",
        color: "#374151"
      }
    },
    grid: {
      color: "#E5E7EB",
      opacity: 0.5
    },
    padding: {
      top: 20,
      right: 20,
      bottom: 60,
      left: 60
    }
  }
};
let developerConfig = {};
function configure(config) {
  if (!config) {
    developerConfig = {};
    return;
  }
  if (typeof config === "string") {
    try {
      developerConfig = yaml__default.load(config);
    } catch (error) {
      console.error("Failed to parse YAML configuration:", error);
      developerConfig = {};
    }
  } else {
    developerConfig = config;
  }
}
function resetConfig() {
  developerConfig = {};
}
function getSystemDefaults() {
  return { ...SYSTEM_DEFAULTS };
}
const COMPONENT_TYPES = {
  SOURCE: "source",
  STYLE: "style",
  CONFIG: "config",
  PARAMS: "params",
  CHART: "chart"
};
const SUPPORTED_VERSIONS = ["1", "1.0"];
function parseComponent(yamlString) {
  if (!yamlString || typeof yamlString !== "string") {
    throw new Error("ChartML component must be a non-empty string");
  }
  let parsed;
  try {
    parsed = yaml__default.load(yamlString);
  } catch (error) {
    throw new Error(`Failed to parse ChartML YAML: ${error.message}`);
  }
  if (!parsed || typeof parsed !== "object") {
    throw new Error("ChartML component must be a valid YAML object");
  }
  const componentType = determineComponentType(parsed);
  if (parsed.version) {
    validateVersion(parsed.version);
  }
  validateComponent(componentType, parsed);
  return {
    type: componentType,
    spec: parsed,
    raw: yamlString
  };
}
function determineComponentType(spec) {
  if (!spec.type || typeof spec.type !== "string") {
    throw new Error('ChartML component must specify a "type" field (source, style, config, or chart)');
  }
  const type = spec.type.toLowerCase();
  if (!Object.values(COMPONENT_TYPES).includes(type)) {
    throw new Error(
      `Invalid component type: "${spec.type}". Must be one of: ${Object.values(COMPONENT_TYPES).join(", ")}`
    );
  }
  return type;
}
function validateVersion(version) {
  const versionStr = String(version);
  if (!SUPPORTED_VERSIONS.includes(versionStr)) {
    throw new Error(
      `Unsupported ChartML version: ${version}. Supported versions: ${SUPPORTED_VERSIONS.join(", ")}`
    );
  }
}
function validateComponent(type, spec) {
  switch (type) {
    case COMPONENT_TYPES.SOURCE:
      validateSourceComponent(spec);
      break;
    case COMPONENT_TYPES.STYLE:
      validateStyleComponent(spec);
      break;
    case COMPONENT_TYPES.CONFIG:
      validateConfigComponent(spec);
      break;
    case COMPONENT_TYPES.PARAMS:
      validateParamsComponent(spec);
      break;
    case COMPONENT_TYPES.CHART:
      validateChartComponent(spec);
      break;
    default:
      throw new Error(`Unknown component type: ${type}`);
  }
}
function validateSourceComponent(spec) {
  if (!spec.name || typeof spec.name !== "string") {
    throw new Error('Source component must have a "name" field (string)');
  }
  if (!spec.provider || typeof spec.provider !== "string") {
    throw new Error(
      `Source "${spec.name}" must specify a "provider" field (e.g., inline, http, or a custom plugin provider)`
    );
  }
  switch (spec.provider.toLowerCase()) {
    case "inline":
      if (!spec.rows || !Array.isArray(spec.rows)) {
        throw new Error(`Source "${spec.name}" with provider "inline" must have a "rows" field (array)`);
      }
      break;
    case "http":
      if (!spec.url || typeof spec.url !== "string") {
        throw new Error(`Source "${spec.name}" with provider "http" must have a "url" field (string)`);
      }
      break;
    case "api":
      if (!spec.endpoint || typeof spec.endpoint !== "string") {
        throw new Error(`Source "${spec.name}" with provider "api" must have an "endpoint" field (string)`);
      }
      break;
  }
}
function validateStyleComponent(spec) {
  if (!spec.name || typeof spec.name !== "string") {
    throw new Error('Style component must have a "name" field (string)');
  }
  const styleProperties = ["colors", "background", "fonts", "grid", "padding", "width", "height"];
  const hasStyleProperty = styleProperties.some((prop) => spec.hasOwnProperty(prop));
  if (!hasStyleProperty) {
    throw new Error(
      `Style "${spec.name}" must define at least one style property: ${styleProperties.join(", ")}`
    );
  }
  if (spec.colors && !Array.isArray(spec.colors)) {
    throw new Error(`Style "${spec.name}" colors must be an array of color strings`);
  }
}
function validateConfigComponent(spec) {
  if (!spec.theme || typeof spec.theme !== "object") {
    console.warn('Config component should typically have a "theme" object with configuration properties');
  }
}
function validateParamsComponent(spec) {
  if (spec.name && typeof spec.name !== "string") {
    throw new Error('Params component "name" must be a string if provided');
  }
  if (!spec.params || !Array.isArray(spec.params)) {
    throw new Error('Params component must have a "params" field (array of parameter definitions)');
  }
  if (spec.params.length === 0) {
    throw new Error("Params component must have at least one parameter definition");
  }
  spec.params.forEach((param, index) => {
    if (!param.id || typeof param.id !== "string") {
      throw new Error(`Parameter at index ${index} must have an "id" property (string)`);
    }
    if (!param.type || typeof param.type !== "string") {
      throw new Error(`Parameter "${param.id}" must have a "type" property (select, multiselect, number, date, etc.)`);
    }
    const validTypes = ["select", "multiselect", "number", "number_range", "date", "daterange", "text"];
    if (!validTypes.includes(param.type)) {
      throw new Error(
        `Parameter "${param.id}" has invalid type "${param.type}". Must be one of: ${validTypes.join(", ")}`
      );
    }
    if ((param.type === "select" || param.type === "multiselect") && !Array.isArray(param.options)) {
      throw new Error(`Parameter "${param.id}" with type "${param.type}" must have "options" array`);
    }
  });
}
function validateChartComponent(spec) {
  const hasInlineData = spec.data !== void 0;
  const hasDataSource = spec.dataSource && typeof spec.dataSource === "string";
  if (!hasInlineData && !hasDataSource) {
    throw new Error(
      'Chart component must have either "data" (inline data/URL) or "dataSource" (reference to named source)'
    );
  }
  if (hasInlineData && hasDataSource) {
    throw new Error(
      'Chart component cannot have both "data" and "dataSource". Use one or the other.'
    );
  }
  if (!spec.visualize || typeof spec.visualize !== "object") {
    throw new Error('Chart component must have a "visualize" block with chart configuration');
  }
  if (!spec.visualize.type || typeof spec.visualize.type !== "string") {
    throw new Error('Chart "visualize" block must specify a chart "type" (bar, line, pie, etc.)');
  }
}
function parseMultipleComponents(yamlBlocks) {
  const components = {
    sources: [],
    styles: [],
    configs: [],
    params: [],
    charts: []
  };
  const errors = [];
  for (let i = 0; i < yamlBlocks.length; i++) {
    try {
      const component = parseComponent(yamlBlocks[i]);
      switch (component.type) {
        case COMPONENT_TYPES.SOURCE:
          components.sources.push(component);
          break;
        case COMPONENT_TYPES.STYLE:
          components.styles.push(component);
          break;
        case COMPONENT_TYPES.CONFIG:
          components.configs.push(component);
          break;
        case COMPONENT_TYPES.PARAMS:
          components.params.push(component);
          break;
        case COMPONENT_TYPES.CHART:
          components.charts.push(component);
          break;
      }
    } catch (error) {
      errors.push({
        blockIndex: i,
        error: error.message,
        content: yamlBlocks[i]
      });
    }
  }
  return {
    components,
    errors
  };
}
function extractReferences(spec) {
  var _a;
  const references = {
    dataSource: null,
    style: null,
    params: null
  };
  if (spec.dataSource && typeof spec.dataSource === "string") {
    references.dataSource = spec.dataSource;
  }
  if (((_a = spec.visualize) == null ? void 0 : _a.style) && typeof spec.visualize.style === "string") {
    references.style = spec.visualize.style;
  }
  if (spec.params && typeof spec.params === "string") {
    references.params = spec.params;
  }
  return references;
}
class ComponentRegistry {
  constructor(paramChangeRegistry = null) {
    this.sources = /* @__PURE__ */ new Map();
    this.styles = /* @__PURE__ */ new Map();
    this.configs = /* @__PURE__ */ new Map();
    this.params = /* @__PURE__ */ new Map();
    this.paramChangeRegistry = paramChangeRegistry;
  }
  /**
   * Register a source component
   * @param {string} name - Unique identifier
   * @param {Object} definition - Source definition
   * @throws {Error} If name already exists
   */
  registerSource(name, definition) {
    if (!name || typeof name !== "string") {
      throw new Error("Source name must be a non-empty string");
    }
    if (this.sources.has(name)) {
      throw new Error(`Source "${name}" is already registered. Use a unique name.`);
    }
    if (!definition.provider) {
      throw new Error(`Source "${name}" must specify a provider (e.g., inline, http, or custom plugin provider)`);
    }
    this.sources.set(name, { ...definition });
  }
  /**
   * Register a style component
   * @param {string} name - Unique identifier
   * @param {Object} definition - Style definition
   * @throws {Error} If name already exists
   */
  registerStyle(name, definition) {
    if (!name || typeof name !== "string") {
      throw new Error("Style name must be a non-empty string");
    }
    if (this.styles.has(name)) {
      throw new Error(`Style "${name}" is already registered. Use a unique name.`);
    }
    this.styles.set(name, { ...definition });
  }
  /**
   * Register a config component
   * @param {Object} definition - Config definition
   */
  registerConfig(definition) {
    const id = this.configs.size;
    this.configs.set(id, { ...definition });
  }
  /**
   * Register a params component
   * @param {string} name - Unique identifier (required for named params blocks)
   * @param {Object} definition - Params definition
   * @throws {Error} If name already exists
   */
  registerParams(name, definition) {
    if (!name || typeof name !== "string") {
      throw new Error("Params name must be a non-empty string");
    }
    if (this.params.has(name)) {
      throw new Error(`Params "${name}" is already registered. Use a unique name.`);
    }
    if (!definition.params || !Array.isArray(definition.params)) {
      throw new Error(`Params "${name}" must have a params array`);
    }
    const values = {};
    definition.params.forEach((param) => {
      if (!param.id) {
        throw new Error(`Param in "${name}" must have an id field`);
      }
      values[param.id] = param.default;
    });
    this.params.set(name, {
      definition: { ...definition },
      values
    });
  }
  /**
   * Resolve a source reference
   * @param {string} name - Source name to resolve
   * @returns {Object|null} Source definition or null if not found
   */
  resolveSource(name) {
    if (!name || typeof name !== "string") {
      return null;
    }
    return this.sources.get(name) || null;
  }
  /**
   * Resolve a style reference
   * @param {string} name - Style name to resolve
   * @returns {Object|null} Style definition or null if not found
   */
  resolveStyle(name) {
    if (!name || typeof name !== "string") {
      return null;
    }
    return this.styles.get(name) || null;
  }
  /**
   * Resolve a params reference
   * @param {string} name - Params name to resolve
   * @returns {Object|null} Params definition or null if not found
   */
  resolveParams(name) {
    if (!name || typeof name !== "string") {
      return null;
    }
    const params = this.params.get(name);
    return params ? params.definition : null;
  }
  /**
   * Get current parameter values for a named params block
   * @param {string} name - Params block name
   * @returns {Object} Current parameter values { param_id: value }
   */
  getParamValues(name) {
    if (!name || typeof name !== "string") {
      return {};
    }
    const params = this.params.get(name);
    return params ? { ...params.values } : {};
  }
  /**
   * Set a parameter value
   * @param {string} name - Params block name
   * @param {string} paramId - Parameter id
   * @param {*} value - New value
   */
  setParamValue(name, paramId, value) {
    var _a;
    if (!name || typeof name !== "string") {
      throw new Error("Params name must be a non-empty string");
    }
    const params = this.params.get(name);
    if (!params) {
      throw new Error(`Params "${name}" not found. Register it first.`);
    }
    const paramDef = (_a = params.definition.params) == null ? void 0 : _a.find((p) => p.id === paramId);
    if (!paramDef) {
      throw new Error(`Parameter "${paramId}" not found in params "${name}"`);
    }
    const oldValue = params.values[paramId];
    const valueChanged = oldValue !== value;
    params.values[paramId] = value;
    if (valueChanged && this.paramChangeRegistry) {
      this.paramChangeRegistry.notifyChange(name, paramId, value);
    }
  }
  /**
   * Get merged config
   * Merges all registered configs in registration order
   * @returns {Object} Merged configuration
   */
  getMergedConfig() {
    let merged = {};
    for (const [, config] of this.configs) {
      merged = deepMerge(merged, config);
    }
    return merged;
  }
  /**
   * Clear all registered components
   * Useful for testing or page navigation
   */
  clear() {
    this.sources.clear();
    this.styles.clear();
    this.configs.clear();
    this.params.clear();
  }
  /**
   * Get registry statistics
   * @returns {Object} Count of each component type
   */
  getStats() {
    return {
      sources: this.sources.size,
      styles: this.styles.size,
      configs: this.configs.size,
      params: this.params.size
    };
  }
  /**
   * Check if a source exists
   * @param {string} name - Source name
   * @returns {boolean}
   */
  hasSource(name) {
    return this.sources.has(name);
  }
  /**
   * Check if a style exists
   * @param {string} name - Style name
   * @returns {boolean}
   */
  hasStyle(name) {
    return this.styles.has(name);
  }
  /**
   * Check if a params block exists
   * @param {string} name - Params name
   * @returns {boolean}
   */
  hasParams(name) {
    if (!name || typeof name !== "string") {
      return false;
    }
    return this.params.has(name);
  }
}
let globalRegistry$1 = new ComponentRegistry();
function getGlobalRegistry() {
  return globalRegistry$1;
}
function createRegistry(paramChangeRegistry = null) {
  return new ComponentRegistry(paramChangeRegistry);
}
function resetGlobalRegistry() {
  globalRegistry$1 = new ComponentRegistry();
}
class GlobalPluginRegistry {
  constructor() {
    this.chartRenderers = /* @__PURE__ */ new Map();
    this.dataSources = /* @__PURE__ */ new Map();
    this.transformMiddleware = [];
    this.datasourceResolver = null;
  }
  registerChartRenderer(type, renderer) {
    if (this.chartRenderers.has(type)) {
      console.warn(
        `⚠️  ChartML: Renderer "${type}" is already registered and will be overwritten.
   Consider using a namespaced type (e.g., "@yourorg/${type}") to avoid conflicts.`
      );
    }
    this.chartRenderers.set(type, renderer);
  }
  registerDataSource(name, handler) {
    this.dataSources.set(name, handler);
  }
  registerTransformMiddleware(middleware) {
    this.transformMiddleware.push(middleware);
  }
  setDatasourceResolver(resolver) {
    this.datasourceResolver = resolver;
  }
  getDatasourceResolver() {
    return this.datasourceResolver;
  }
  getChartRenderer(type) {
    return this.chartRenderers.get(type);
  }
  getDataSource(name) {
    return this.dataSources.get(name);
  }
  getAllChartRenderers() {
    return this.chartRenderers;
  }
  getAllDataSources() {
    return this.dataSources;
  }
  getAllTransformMiddleware() {
    return this.transformMiddleware;
  }
}
const globalRegistry = new GlobalPluginRegistry();
const aggregateCache = /* @__PURE__ */ new Map();
const CACHE_TTL_MS = 5 * 60 * 1e3;
const inFlightRequests = /* @__PURE__ */ new Map();
function generateCacheKey(spec, context) {
  const dataSpec = spec.data || {};
  const transformSpec = spec.transform || {};
  return JSON.stringify({
    data: dataSpec,
    transform: transformSpec
  });
}
async function d3Transform(data, spec, context = {}) {
  var _a;
  if (!spec) {
    return {
      data,
      metadata: {}
    };
  }
  const aggregateSpec = ((_a = spec.transform) == null ? void 0 : _a.aggregate) || {};
  const { dimensions = [], measures = [], sort, limit, filters } = aggregateSpec;
  if (dimensions.length === 0 && measures.length === 0) {
    if (!data && context.fetchData) {
      data = await context.fetchData();
    }
    const actualData = (data == null ? void 0 : data.data) !== void 0 ? data.data : data;
    const resultData = filters ? applyFilters(actualData, filters) : actualData;
    return {
      data: resultData,
      metadata: {
        refreshedAt: Date.now(),
        cacheHit: false
      }
    };
  }
  const cacheKey = !context.bypassCache ? generateCacheKey(spec) : null;
  if (!context.bypassCache && cacheKey) {
    const cached = aggregateCache.get(cacheKey);
    if (cached) {
      const age = Date.now() - cached.timestamp;
      if (age < CACHE_TTL_MS) {
        return {
          data: cached.result,
          metadata: {
            refreshedAt: cached.timestamp,
            cacheHit: true
          }
        };
      } else {
        aggregateCache.delete(cacheKey);
      }
    }
    const inFlight = inFlightRequests.get(cacheKey);
    if (inFlight) {
      return await inFlight;
    }
  }
  const aggregationPromise = (async () => {
    try {
      if (!data && context.fetchData) {
        data = await context.fetchData();
      }
      const actualData = (data == null ? void 0 : data.data) !== void 0 ? data.data : data;
      const sourceMetadata = (data == null ? void 0 : data.metadata) || {};
      const { preAggFilters, postAggFilters } = splitFilters(filters, dimensions, measures);
      const filteredData = preAggFilters ? applyFilters(actualData, preAggFilters) : actualData;
      let result;
      if (dimensions.length === 0) {
        const aggregated = {};
        measures.forEach((measure) => {
          if (!measure.expression) {
            aggregated[measure.name] = computeMeasure(filteredData, measure);
          }
        });
        result = [aggregated];
      } else if (dimensions.length === 1) {
        const grouped = d3$1.rollup(
          filteredData,
          (group) => {
            const aggregated = {};
            aggregated[dimensions[0]] = group[0][dimensions[0]];
            measures.forEach((measure) => {
              if (!measure.expression) {
                aggregated[measure.name] = computeMeasure(group, measure);
              }
            });
            return aggregated;
          },
          (d) => d[dimensions[0]]
        );
        result = Array.from(grouped.values());
      } else {
        const grouped = d3$1.rollup(
          filteredData,
          (group) => {
            const aggregated = {};
            dimensions.forEach((dim) => {
              aggregated[dim] = group[0][dim];
            });
            measures.forEach((measure) => {
              if (!measure.expression) {
                aggregated[measure.name] = computeMeasure(group, measure);
              }
            });
            return aggregated;
          },
          ...dimensions.map((dim) => (d) => d[dim])
        );
        result = flattenNestedMap(grouped, dimensions);
      }
      const expressionMeasures = measures.filter((m) => m.expression);
      if (expressionMeasures.length > 0) {
        result = result.map((row) => {
          const extended = { ...row };
          expressionMeasures.forEach((measure) => {
            try {
              extended[measure.name] = evaluateExpression(measure.expression, extended);
            } catch (error) {
              console.warn(`Failed to evaluate expression "${measure.expression}":`, error.message);
              extended[measure.name] = null;
            }
          });
          return extended;
        });
      }
      if (postAggFilters) {
        result = applyFilters(result, postAggFilters);
      }
      if ((sort == null ? void 0 : sort.length) > 0) {
        result.sort((a, b) => {
          for (const s of sort) {
            const aVal = a[s.field];
            const bVal = b[s.field];
            if (aVal == null && bVal == null) continue;
            if (aVal == null) return 1;
            if (bVal == null) return -1;
            if (aVal === bVal) continue;
            const cmp = aVal < bVal ? -1 : 1;
            return s.direction === "desc" ? -cmp : cmp;
          }
          return 0;
        });
      }
      if (limit && limit > 0) {
        result = result.slice(0, limit);
      }
      const timestamp = sourceMetadata.refreshedAt || Date.now();
      if (!context.bypassCache && cacheKey) {
        aggregateCache.set(cacheKey, {
          result,
          timestamp
        });
      }
      return {
        data: result,
        metadata: {
          refreshedAt: timestamp,
          cacheHit: false,
          sourceWasCached: !!sourceMetadata.refreshedAt
          // True if data came from DuckDB cache
        }
      };
    } finally {
      if (cacheKey) {
        inFlightRequests.delete(cacheKey);
      }
    }
  })();
  if (cacheKey) {
    inFlightRequests.set(cacheKey, aggregationPromise);
  }
  return await aggregationPromise;
}
function computeMeasure(group, measure) {
  var _a, _b;
  if (measure.expression) {
    return null;
  }
  const { column, aggregation } = measure;
  switch (aggregation == null ? void 0 : aggregation.toLowerCase()) {
    case "sum":
      return d3$1.sum(group, (d) => Number(d[column]) || 0);
    case "avg":
    case "mean":
      return d3$1.mean(group, (d) => Number(d[column]) || 0) || 0;
    case "count":
      return group.length;
    case "min":
      return d3$1.min(group, (d) => d[column]);
    case "max":
      return d3$1.max(group, (d) => d[column]);
    case "first":
      return (_a = group[0]) == null ? void 0 : _a[column];
    case "last":
      return (_b = group[group.length - 1]) == null ? void 0 : _b[column];
    default:
      throw new Error(`Unknown aggregation: ${aggregation}`);
  }
}
function flattenNestedMap(map, dimensions, depth = 0) {
  const result = [];
  if (depth >= dimensions.length) {
    return [map];
  }
  for (const [key, value] of map.entries()) {
    if (value instanceof Map) {
      const nested = flattenNestedMap(value, dimensions, depth + 1);
      result.push(...nested);
    } else {
      result.push(value);
    }
  }
  return result;
}
function evaluateExpression(expression, context) {
  try {
    if (expression.length > 500) {
      throw new Error("Expression too long (max 500 characters)");
    }
    const allowedPattern = /^[a-zA-Z0-9_+\-*/().\s]+$/;
    if (!allowedPattern.test(expression)) {
      throw new Error("Expression contains disallowed characters");
    }
    const dangerousPatterns = [
      /\.\./,
      // Prototype pollution attempts
      /__proto__/,
      // Prototype pollution
      /constructor/,
      // Constructor access
      /eval/i,
      // Eval attempts
      /function/i,
      // Function declarations
      /import/i,
      // Import statements
      /require/i
      // Require statements
    ];
    for (const pattern of dangerousPatterns) {
      if (pattern.test(expression)) {
        throw new Error("Expression contains disallowed pattern");
      }
    }
    const safeContext = { ...context };
    let code = expression;
    const identifiers = expression.match(/\b[a-zA-Z_][a-zA-Z0-9_]*\b/g) || [];
    identifiers.forEach((identifier) => {
      const keywords = ["true", "false", "null", "undefined", "NaN", "Infinity"];
      if (keywords.includes(identifier)) return;
      const regex = new RegExp(`\\b${identifier}\\b`, "g");
      code = code.replace(regex, `context.${identifier}`);
    });
    const fn = new Function("context", `'use strict'; return (${code});`);
    const timeout = 100;
    const startTime = Date.now();
    const result = fn(safeContext);
    if (Date.now() - startTime > timeout) {
      throw new Error("Expression execution timeout");
    }
    return Number(result) || 0;
  } catch (error) {
    throw new Error(`Expression evaluation failed: ${expression} - ${error.message}`);
  }
}
function splitFilters(filters, dimensions, measures) {
  if (!filters || !filters.rules || filters.rules.length === 0) {
    return { preAggFilters: null, postAggFilters: null };
  }
  const measureFields = new Set(measures.map((m) => m.name));
  const preAggRules = [];
  const postAggRules = [];
  filters.rules.forEach((rule) => {
    if (measureFields.has(rule.field)) {
      postAggRules.push(rule);
    } else if (dimensions.includes(rule.field)) {
      preAggRules.push(rule);
    } else {
      console.warn(`[d3Transform] Filter field "${rule.field}" not found in dimensions or measures, applying pre-aggregation`);
      preAggRules.push(rule);
    }
  });
  return {
    preAggFilters: preAggRules.length > 0 ? { ...filters, rules: preAggRules } : null,
    postAggFilters: postAggRules.length > 0 ? { ...filters, rules: postAggRules } : null
  };
}
function applyFilters(data, filters) {
  if (!filters || !filters.rules || filters.rules.length === 0) {
    return data;
  }
  const { combinator = "and", rules } = filters;
  return data.filter((row) => {
    const results = rules.map((rule) => applyRule(row, rule));
    if (combinator === "or") {
      return results.some((r) => r);
    } else {
      return results.every((r) => r);
    }
  });
}
function applyRule(row, rule) {
  const { field, operator, value } = rule;
  const fieldValue = row[field];
  switch (operator) {
    case "=":
    case "==":
      return fieldValue == value;
    case "!=":
      return fieldValue != value;
    case ">":
      return fieldValue > value;
    case ">=":
      return fieldValue >= value;
    case "<":
      return fieldValue < value;
    case "<=":
      return fieldValue <= value;
    case "in":
      return Array.isArray(value) && value.includes(fieldValue);
    case "not in":
      return Array.isArray(value) && !value.includes(fieldValue);
    case "contains":
      return String(fieldValue).includes(String(value));
    case "not contains":
      return !String(fieldValue).includes(String(value));
    case "starts with":
      return String(fieldValue).startsWith(String(value));
    case "ends with":
      return String(fieldValue).endsWith(String(value));
    case "is null":
      return fieldValue === null || fieldValue === void 0;
    case "is not null":
      return fieldValue !== null && fieldValue !== void 0;
    default:
      console.warn(`[d3Transform] Unknown operator: ${operator}`);
      return true;
  }
}
function renderParams(paramDefs, currentValues, onChange, container, className = "") {
  container.innerHTML = "";
  if (!paramDefs || paramDefs.length === 0) {
    return container;
  }
  const wrapper = document.createElement("div");
  wrapper.className = className ? `chartml-params ${className}` : "chartml-params";
  paramDefs.forEach((param) => {
    const paramGroup = document.createElement("div");
    paramGroup.className = "chartml-param-group";
    const paramControl = renderParamControl(param, currentValues[param.id], (newValue) => {
      onChange(param.id, newValue);
    });
    paramGroup.appendChild(paramControl);
    wrapper.appendChild(paramGroup);
  });
  container.appendChild(wrapper);
  return wrapper;
}
function renderParamControl(param, currentValue, onChange) {
  const { type, label, id, options, placeholder, default: defaultValue } = param;
  const value = currentValue !== void 0 ? currentValue : defaultValue;
  switch (type) {
    case "multiselect":
      return renderMultiSelectControl(param, value, onChange);
    case "select":
      return renderSelectControl(param, value, onChange);
    case "number":
      return renderNumberControl(param, value, onChange);
    case "text":
      return renderTextControl(param, value, onChange);
    case "daterange":
      return renderDateRangeControl(param, value, onChange);
    default:
      console.warn(`[ChartML] Unknown parameter type: ${type}`);
      const errorDiv = document.createElement("div");
      errorDiv.className = "chartml-param-error";
      errorDiv.textContent = `Unknown parameter type: ${type}`;
      return errorDiv;
  }
}
function renderMultiSelectControl(param, currentValue, onChange) {
  const { id, label, options } = param;
  const selectedValues = Array.isArray(currentValue) ? currentValue : currentValue ? [currentValue] : [];
  const container = document.createElement("div");
  container.className = "chartml-param-multiselect";
  const labelEl = document.createElement("label");
  labelEl.className = "chartml-param-label";
  labelEl.textContent = label;
  container.appendChild(labelEl);
  const button = document.createElement("button");
  button.type = "button";
  button.className = "chartml-param-multiselect-button";
  const buttonText = document.createElement("span");
  const updateButtonText = () => {
    const count = selectedValues.length;
    buttonText.textContent = count === 0 ? "Select..." : count === 1 ? `${selectedValues[0]}` : `${count} selected`;
  };
  updateButtonText();
  button.appendChild(buttonText);
  const arrow = document.createElement("span");
  arrow.className = "chartml-param-multiselect-arrow";
  arrow.innerHTML = "▼";
  button.appendChild(arrow);
  const dropdown = document.createElement("div");
  dropdown.className = "chartml-param-multiselect-dropdown";
  dropdown.style.display = "none";
  options.forEach((option) => {
    const optionLabel = document.createElement("label");
    optionLabel.className = "chartml-param-option";
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.checked = selectedValues.includes(option);
    checkbox.addEventListener("change", (e) => {
      const newValue = e.target.checked ? [...selectedValues, option] : selectedValues.filter((v) => v !== option);
      onChange(newValue);
      selectedValues.length = 0;
      selectedValues.push(...newValue);
      updateButtonText();
    });
    const span = document.createElement("span");
    span.textContent = option;
    optionLabel.appendChild(checkbox);
    optionLabel.appendChild(span);
    dropdown.appendChild(optionLabel);
  });
  button.addEventListener("click", (e) => {
    e.stopPropagation();
    const isOpen = dropdown.style.display !== "none";
    dropdown.style.display = isOpen ? "none" : "block";
    arrow.style.transform = isOpen ? "rotate(0deg)" : "rotate(180deg)";
  });
  const closeDropdown = (e) => {
    if (!container.contains(e.target)) {
      dropdown.style.display = "none";
      arrow.style.transform = "rotate(0deg)";
    }
  };
  document.addEventListener("click", closeDropdown);
  container.appendChild(button);
  container.appendChild(dropdown);
  return container;
}
function renderSelectControl(param, currentValue, onChange) {
  const { id, label, options } = param;
  const container = document.createElement("div");
  container.className = "chartml-param-select";
  const labelEl = document.createElement("label");
  labelEl.className = "chartml-param-label";
  labelEl.htmlFor = `param-${id}`;
  labelEl.textContent = label;
  container.appendChild(labelEl);
  const select = document.createElement("select");
  select.id = `param-${id}`;
  select.value = currentValue || "";
  options.forEach((option) => {
    const optEl = document.createElement("option");
    optEl.value = option;
    optEl.textContent = option;
    optEl.selected = option === currentValue;
    select.appendChild(optEl);
  });
  select.addEventListener("change", (e) => {
    onChange(e.target.value);
  });
  container.appendChild(select);
  return container;
}
function renderNumberControl(param, currentValue, onChange) {
  const { id, label, min, max } = param;
  const container = document.createElement("div");
  container.className = "chartml-param-number";
  const labelEl = document.createElement("label");
  labelEl.className = "chartml-param-label";
  labelEl.htmlFor = `param-${id}`;
  labelEl.textContent = label;
  container.appendChild(labelEl);
  const input = document.createElement("input");
  input.id = `param-${id}`;
  input.type = "number";
  input.value = currentValue !== void 0 ? currentValue : 0;
  if (min !== void 0) input.min = min;
  if (max !== void 0) input.max = max;
  input.addEventListener("input", (e) => {
    onChange(Number(e.target.value));
  });
  container.appendChild(input);
  return container;
}
function renderTextControl(param, currentValue, onChange) {
  const { id, label, placeholder } = param;
  const container = document.createElement("div");
  container.className = "chartml-param-text";
  const labelEl = document.createElement("label");
  labelEl.className = "chartml-param-label";
  labelEl.htmlFor = `param-${id}`;
  labelEl.textContent = label;
  container.appendChild(labelEl);
  const input = document.createElement("input");
  input.id = `param-${id}`;
  input.type = "text";
  input.value = currentValue || "";
  if (placeholder) input.placeholder = placeholder;
  input.addEventListener("input", (e) => {
    onChange(e.target.value);
  });
  container.appendChild(input);
  return container;
}
function renderDateRangeControl(param, currentValue, onChange) {
  const { id, label } = param;
  const value = currentValue || {};
  const container = document.createElement("div");
  container.className = "chartml-param-daterange";
  const labelEl = document.createElement("label");
  labelEl.className = "chartml-param-label";
  labelEl.textContent = label;
  container.appendChild(labelEl);
  const inputsContainer = document.createElement("div");
  inputsContainer.className = "chartml-param-daterange-inputs";
  const startInput = document.createElement("input");
  startInput.type = "date";
  startInput.value = value.start || "";
  startInput.addEventListener("input", (e) => {
    onChange({
      ...value,
      start: e.target.value
    });
  });
  const separator = document.createElement("span");
  separator.className = "chartml-param-daterange-separator";
  separator.textContent = "to";
  const endInput = document.createElement("input");
  endInput.type = "date";
  endInput.value = value.end || "";
  endInput.addEventListener("input", (e) => {
    onChange({
      ...value,
      end: e.target.value
    });
  });
  inputsContainer.appendChild(startInput);
  inputsContainer.appendChild(separator);
  inputsContainer.appendChild(endInput);
  container.appendChild(inputsContainer);
  return container;
}
function resolveParamReferences(spec, paramValues = {}, chartParams = null) {
  if (!spec) return spec;
  let specString = JSON.stringify(spec);
  const paramReferenceRegex = /"\$([a-zA-Z0-9_]+(?:\.[a-zA-Z0-9_]+)*)"/g;
  specString = specString.replace(paramReferenceRegex, (match, path) => {
    const hasDot = path.includes(".");
    let value;
    if (hasDot) {
      value = paramValues[path];
      if (value === void 0) {
        console.warn(`[ChartML] Named parameter reference not found: $${path}`);
        return match;
      }
    } else {
      value = paramValues[path];
      if (value === void 0 && chartParams && Array.isArray(chartParams)) {
        const paramDef = chartParams.find((p) => p.id === path);
        if (paramDef) {
          value = paramDef.default;
        }
      }
      if (value === void 0) {
        console.warn(`[ChartML] Chart-level parameter reference not found: $${path}`);
        return match;
      }
    }
    return JSON.stringify(value);
  });
  return JSON.parse(specString);
}
function getNestedValue(obj, path) {
  const parts = path.split(".");
  let current = obj;
  for (const part of parts) {
    if (current === null || current === void 0) {
      return void 0;
    }
    current = current[part];
  }
  return current;
}
function extractParamReferences(spec) {
  if (!spec) return /* @__PURE__ */ new Set();
  const specString = JSON.stringify(spec);
  const paramReferenceRegex = /"\$([a-zA-Z0-9_]+(?:\.[a-zA-Z0-9_]+)*)"/g;
  const references = /* @__PURE__ */ new Set();
  let match;
  while ((match = paramReferenceRegex.exec(specString)) !== null) {
    references.add(match[1]);
  }
  return references;
}
function validateParamReferences(spec, paramValues = {}, chartParams = null) {
  const references = extractParamReferences(spec);
  const missing = [];
  for (const ref of references) {
    const hasDot = ref.includes(".");
    if (hasDot) {
      if (getNestedValue(paramValues, ref) === void 0) {
        missing.push(ref);
      }
    } else {
      if (!(ref in paramValues)) {
        const paramDef = chartParams == null ? void 0 : chartParams.find((p) => p.id === ref);
        if (!paramDef || paramDef.default === void 0) {
          missing.push(ref);
        }
      }
    }
  }
  return {
    valid: missing.length === 0,
    missing
  };
}
class SourceRefreshRegistry {
  constructor() {
    this.sources = /* @__PURE__ */ new Map();
  }
  /**
   * Subscribe a Chart instance to source refresh notifications
   * @param {string} sourceName - Name of the data source (e.g., 'search_trends')
   * @param {Chart} chart - Chart instance to subscribe
   */
  subscribe(sourceName, chart) {
    if (!this.sources.has(sourceName)) {
      this.sources.set(sourceName, {
        subscribers: /* @__PURE__ */ new Set(),
        isRefreshing: false,
        lastFetched: null
      });
    }
    const source = this.sources.get(sourceName);
    source.subscribers.add(chart);
  }
  /**
   * Unsubscribe a Chart instance from source refresh notifications
   * @param {string} sourceName - Name of the data source
   * @param {Chart} chart - Chart instance to unsubscribe
   */
  unsubscribe(sourceName, chart) {
    const source = this.sources.get(sourceName);
    if (source) {
      source.subscribers.delete(chart);
      if (source.subscribers.size === 0) {
        this.sources.delete(sourceName);
      }
    }
  }
  /**
   * Refresh a source - coordinates notifications to all subscribers
   * @param {string} sourceName - Name of the data source
   * @param {Function} refreshCallback - Async function that does the actual refresh (from initiating chart)
   * @param {Chart} initiatingChart - The chart that triggered the refresh (to skip re-rendering it)
   * @returns {Promise<void>}
   */
  async refreshSource(sourceName, refreshCallback, initiatingChart = null) {
    const source = this.sources.get(sourceName);
    if (!source) {
      await refreshCallback();
      return;
    }
    try {
      source.isRefreshing = true;
      for (const chart of source.subscribers) {
        if (chart.onRefreshStateChange) {
          chart.onRefreshStateChange(true);
        }
      }
      await refreshCallback();
      source.lastFetched = Date.now();
    } finally {
      source.isRefreshing = false;
      for (const chart of source.subscribers) {
        if (chart.metadata) {
          chart.metadata.last_updated = source.lastFetched;
        }
        if (chart.onRefreshStateChange) {
          chart.onRefreshStateChange(false);
        }
        if (chart !== initiatingChart && chart.rerender) {
          chart.rerender().catch((error) => {
            console.error("[SourceRefreshRegistry] Chart rerender failed:", error);
          });
        }
      }
    }
  }
  /**
   * Get the last fetched timestamp for a source
   * @param {string} sourceName - Name of the data source
   * @returns {number|null} Timestamp in milliseconds, or null if never fetched
   */
  getLastFetched(sourceName) {
    const source = this.sources.get(sourceName);
    return (source == null ? void 0 : source.lastFetched) || null;
  }
  /**
   * Check if a source is currently refreshing
   * @param {string} sourceName - Name of the data source
   * @returns {boolean}
   */
  isRefreshing(sourceName) {
    const source = this.sources.get(sourceName);
    return (source == null ? void 0 : source.isRefreshing) || false;
  }
}
class ParamChangeRegistry {
  constructor() {
    this.scopes = /* @__PURE__ */ new Map();
  }
  /**
   * Subscribe a Chart instance to parameter change notifications for a scope
   * @param {string} scopeName - Name of the params block (e.g., 'dashboard_filters')
   * @param {Chart} chart - Chart instance to subscribe
   */
  subscribe(scopeName, chart) {
    if (!scopeName || typeof scopeName !== "string") {
      console.warn("[ParamChangeRegistry] Invalid scope name:", scopeName);
      return;
    }
    if (!this.scopes.has(scopeName)) {
      this.scopes.set(scopeName, /* @__PURE__ */ new Set());
    }
    const subscribers = this.scopes.get(scopeName);
    subscribers.add(chart);
  }
  /**
   * Unsubscribe a Chart instance from parameter change notifications
   * @param {string} scopeName - Name of the params block
   * @param {Chart} chart - Chart instance to unsubscribe
   */
  unsubscribe(scopeName, chart) {
    const subscribers = this.scopes.get(scopeName);
    if (subscribers) {
      subscribers.delete(chart);
      if (subscribers.size === 0) {
        this.scopes.delete(scopeName);
      }
    }
  }
  /**
   * Notify all charts subscribed to a scope that a parameter changed
   * Called by registry.setParamValue() after value comparison
   *
   * @param {string} scopeName - Name of the params block
   * @param {string} paramId - Parameter ID that changed
   * @param {*} newValue - New parameter value
   */
  notifyChange(scopeName, paramId, newValue) {
    const subscribers = this.scopes.get(scopeName);
    if (!subscribers || subscribers.size === 0) {
      return;
    }
    for (const chart of subscribers) {
      if (chart.rerender) {
        chart.rerender().catch((error) => {
          console.error("[ParamChangeRegistry] Chart rerender failed:", error);
        });
      }
    }
  }
  /**
   * Get subscriber count for a scope (useful for debugging)
   * @param {string} scopeName - Name of the params block
   * @returns {number} Number of subscribed charts
   */
  getSubscriberCount(scopeName) {
    const subscribers = this.scopes.get(scopeName);
    return subscribers ? subscribers.size : 0;
  }
  /**
   * Get all registered scopes (useful for debugging)
   * @returns {string[]} Array of scope names
   */
  getScopes() {
    return Array.from(this.scopes.keys());
  }
}
const DEFAULT_LABEL_FONT_SIZE = "12px";
const DEFAULT_LABEL_FONT_FAMILY = "system-ui";
function measureLabelWidths(labels, fontSize = DEFAULT_LABEL_FONT_SIZE, fontFamily = DEFAULT_LABEL_FONT_FAMILY) {
  const svg = d3.select("body").append("svg").style("position", "absolute").style("visibility", "hidden").style("width", "0").style("height", "0");
  const measurements = labels.map((label) => {
    const text = svg.append("text").style("font-size", fontSize).style("font-family", fontFamily).text(label);
    const width = text.node().getComputedTextLength();
    text.remove();
    return width;
  });
  svg.remove();
  return measurements;
}
function determineLabelStrategy(labels, chartWidth, config = {}) {
  const {
    labelStrategy = "auto",
    maxLabelWidth = 120,
    rotationAngle = -45,
    // Fixed at -45° for consistency
    minLabelSpacing = 10,
    maxLabelsBeforeSampling = 50
  } = config;
  if (labelStrategy !== "auto") {
    return {
      strategy: labelStrategy,
      metadata: { forced: true, rotationAngle, maxLabelWidth }
    };
  }
  const labelCount = labels.length;
  const availableSpacePerLabel = chartWidth / labelCount;
  const labelWidths = measureLabelWidths(labels);
  const maxWidth = Math.max(...labelWidths);
  const avgWidth = labelWidths.reduce((a, b) => a + b, 0) / labelWidths.length;
  if (avgWidth + minLabelSpacing <= availableSpacePerLabel) {
    return {
      strategy: "horizontal",
      metadata: { avgWidth, availableSpacePerLabel, labelWidths }
    };
  }
  if (labelCount <= 40) {
    const radians = 45 * Math.PI / 180;
    const requiredVerticalSpace = maxWidth * Math.sin(radians);
    const requiredMargin = Math.min(Math.ceil(requiredVerticalSpace) + 15, 150);
    return {
      strategy: "rotated",
      metadata: {
        rotationAngle,
        avgWidth,
        maxWidth,
        requiredMargin,
        labelWidths
      }
    };
  }
  if (maxLabelWidth + minLabelSpacing <= availableSpacePerLabel && labelCount <= 50) {
    return {
      strategy: "truncated",
      metadata: { maxLabelWidth, originalMaxWidth: maxWidth, labelWidths }
    };
  }
  if (labelCount >= 30) {
    return {
      strategy: "sampled",
      metadata: {
        totalLabels: labelCount,
        maxLabelsToShow: Math.max(Math.floor(chartWidth / 120), 5),
        reason: "too_many_categories"
      }
    };
  }
  return {
    strategy: "rotated",
    metadata: { rotationAngle, maxWidth, fallback: true, labelWidths }
  };
}
function applyHorizontalLabels(axisSelection, fontSize = DEFAULT_LABEL_FONT_SIZE, fontFamily = DEFAULT_LABEL_FONT_FAMILY) {
  axisSelection.selectAll("text").style("font-size", fontSize).style("font-family", fontFamily).style("text-anchor", "middle");
  return 0;
}
function applyRotatedLabels(axisSelection, angle = -45, labelWidths = [], fontSize = DEFAULT_LABEL_FONT_SIZE, fontFamily = DEFAULT_LABEL_FONT_FAMILY) {
  const angleRad = angle * Math.PI / 180;
  const maxWidth = labelWidths.length > 0 ? Math.max(...labelWidths) : 100;
  const additionalMargin = maxWidth * Math.abs(Math.sin(angleRad));
  axisSelection.selectAll("text").style("font-size", fontSize).style("font-family", fontFamily).style("text-anchor", "end").attr("dx", "-0.8em").attr("dy", "0.15em").attr("transform", `rotate(${angle})`);
  return Math.ceil(additionalMargin);
}
function applyTruncatedLabels(axisSelection, maxWidth, options = {}) {
  const {
    tooltip = null,
    container = null,
    fontSize = DEFAULT_LABEL_FONT_SIZE,
    fontFamily = DEFAULT_LABEL_FONT_FAMILY
  } = options;
  axisSelection.selectAll("text").style("font-size", fontSize).style("font-family", fontFamily).style("text-anchor", "middle").each(function(d) {
    const text = d3.select(this);
    const fullText = String(d);
    text.attr("data-full-text", fullText);
    let truncated = fullText;
    text.text(truncated);
    while (text.node().getComputedTextLength() > maxWidth && truncated.length > 0) {
      truncated = truncated.slice(0, -1);
      text.text(truncated + "…");
    }
  });
  if (tooltip && container) {
    axisSelection.selectAll("text").style("cursor", "help").on("mouseenter", function(event) {
      const fullText = d3.select(this).attr("data-full-text");
      const currentText = d3.select(this).text();
      if (currentText.endsWith("…")) {
        tooltip.style("opacity", 1).html(fullText);
      }
    }).on("mousemove", function(event) {
      positionTooltip(tooltip, event);
    }).on("mouseleave", function() {
      tooltip.style("opacity", 0);
    });
  }
  return 0;
}
function getStrategicIndices(totalCount, targetCount) {
  if (totalCount <= targetCount) {
    return Array.from({ length: totalCount }, (_, i) => i);
  }
  const indices = /* @__PURE__ */ new Set();
  indices.add(0);
  indices.add(totalCount - 1);
  const step = totalCount / (targetCount - 1);
  for (let i = 1; i < targetCount - 1; i++) {
    indices.add(Math.round(i * step));
  }
  return Array.from(indices).sort((a, b) => a - b);
}
function applySampledLabels(axisGenerator, domain, maxLabels) {
  const indices = getStrategicIndices(domain.length, maxLabels);
  const tickValues = indices.map((i) => domain[i]);
  return axisGenerator.tickValues(tickValues);
}
function applyLabelStrategy(axisSelection, strategy, metadata = {}, options = {}) {
  const {
    fontSize = DEFAULT_LABEL_FONT_SIZE,
    fontFamily = DEFAULT_LABEL_FONT_FAMILY,
    tooltip = null,
    container = null
  } = options;
  switch (strategy) {
    case "horizontal":
      return applyHorizontalLabels(axisSelection, fontSize, fontFamily);
    case "rotated":
      return applyRotatedLabels(
        axisSelection,
        metadata.rotationAngle || -45,
        metadata.labelWidths || [],
        fontSize,
        fontFamily
      );
    case "truncated":
      return applyTruncatedLabels(
        axisSelection,
        metadata.maxLabelWidth || 120,
        { tooltip, container, fontSize, fontFamily }
      );
    case "sampled":
      console.warn("[labelUtils] Sampled strategy should be applied via applySampledLabels() before axis rendering");
      return 0;
    default:
      console.warn(`[labelUtils] Unknown strategy: ${strategy}, using horizontal`);
      return applyHorizontalLabels(axisSelection, fontSize, fontFamily);
  }
}
const RESERVED_DATA_KEYS = /* @__PURE__ */ new Set(["datasource", "provider", "query", "rows", "url", "cache"]);
function isNamedSources(dataSpec) {
  if (typeof dataSpec === "string") return false;
  if (!dataSpec || typeof dataSpec !== "object") return false;
  if (Array.isArray(dataSpec)) return false;
  return !Object.keys(dataSpec).some((key) => RESERVED_DATA_KEYS.has(key));
}
class Chart {
  constructor(chartml, spec, container, options, middlewareMetadata = {}) {
    this.chartml = chartml;
    this.spec = spec;
    this.container = container;
    this.options = options;
    this.onRefreshStateChange = null;
    let dimensions = middlewareMetadata.dimensions;
    if (!dimensions) {
      dimensions = chartml.getExpectedDimensions(spec);
    }
    this.sourceName = middlewareMetadata.sourceName || null;
    this.metadata = {
      // Use refreshedAt from middleware metadata if available, otherwise current time
      last_updated: middlewareMetadata.refreshedAt || Date.now(),
      dimensions,
      // { width, height }
      // Include any other metadata from middleware
      ...middlewareMetadata
    };
    if (this.sourceName) {
      chartml.sourceRefreshRegistry.subscribe(this.sourceName, this);
    }
    const paramRefs = extractParamReferences(spec);
    this.paramScopes = /* @__PURE__ */ new Set();
    for (const ref of paramRefs) {
      if (ref.includes(".")) {
        const scopeName = ref.split(".")[0];
        if (!this.paramScopes.has(scopeName)) {
          this.paramScopes.add(scopeName);
          chartml.paramChangeRegistry.subscribe(scopeName, this);
        }
      }
    }
  }
  /**
   * Set callback for refresh state changes (for animating refresh button, etc.)
   * @param {Function} callback - Called with (isRefreshing: boolean)
   */
  setRefreshStateCallback(callback) {
    this.onRefreshStateChange = callback;
  }
  /**
   * Refresh the chart by re-fetching data from source (bypassing cache) and re-rendering
   * @returns {Promise<void>}
   *
   * If chart uses a named data source, notifies all other charts using the same source
   * that a refresh is happening (they show spinners). Middleware deduplicates the actual fetch.
   */
  async refresh() {
    if (this.sourceName) {
      await this.chartml.sourceRefreshRegistry.refreshSource(this.sourceName, async () => {
        await this.chartml._renderChartWithParams(this.container, {
          ...this.options,
          bypassCache: true,
          spec: this.spec
          // Use THIS chart's spec
        });
        this.metadata.last_updated = Date.now();
      }, this);
      return;
    }
    try {
      if (this.onRefreshStateChange) {
        this.onRefreshStateChange(true);
      }
      await this.chartml._renderChartWithParams(this.container, {
        ...this.options,
        bypassCache: true,
        spec: this.spec
        // Use THIS chart's spec
      });
      this.metadata.last_updated = Date.now();
    } finally {
      if (this.onRefreshStateChange) {
        this.onRefreshStateChange(false);
      }
    }
  }
  /**
   * Re-render the chart using cached data (no fetch)
   * Used when another chart refreshed the shared data source
   * @returns {Promise<void>}
   */
  async rerender() {
    await this.chartml.render(this.spec, this.container, {
      ...this.options,
      bypassCache: false
      // Use cached data
    });
  }
  /**
   * Destroy the chart and clean up
   */
  destroy() {
    if (this.sourceName) {
      this.chartml.sourceRefreshRegistry.unsubscribe(this.sourceName, this);
    }
    if (this.paramScopes) {
      for (const scopeName of this.paramScopes) {
        this.chartml.paramChangeRegistry.unsubscribe(scopeName, this);
      }
    }
    if (this.container) {
      this.container.innerHTML = "";
    }
  }
  /**
   * Get chart metadata (e.g., last refresh timestamp)
   * @returns {Object} Metadata object with last_updated timestamp
   */
  getMetadata() {
    return { ...this.metadata };
  }
}
class ChartML {
  constructor(options = {}) {
    this.dataSources = /* @__PURE__ */ new Map();
    this.transformMiddleware = [];
    this.chartRenderers = /* @__PURE__ */ new Map();
    this.defaultPalette = options.defaultPalette || null;
    this.loadingIndicator = options.loadingIndicator || null;
    this.animation = options.animation !== false;
    this.datasourceResolver = null;
    this.paramChangeRegistry = new ParamChangeRegistry();
    this.registry = options.registry || createRegistry(this.paramChangeRegistry);
    this.sourceRefreshRegistry = new SourceRefreshRegistry();
    this.hooks = {
      onProgress: options.onProgress || null,
      // Progress callback for streaming
      onCacheHit: options.onCacheHit || null,
      // Cache hit notification
      onCacheMiss: options.onCacheMiss || null,
      // Cache miss notification
      onError: options.onError || null,
      // Error callback
      onLoadingChange: options.onLoadingChange || null
      // Loading state change callback (isLoading: boolean)
    };
    this.paramValues = {};
    this.paramsDefinition = null;
    this.filterContainer = null;
    this.chartContainer = null;
    this._registerBuiltInDataSources();
    this._registerBuiltInTransform();
    this._registerBuiltInChartRenderers();
  }
  /**
   * Set the default palette to use for charts
   * @param {Array} palette - Array of color strings (e.g., ['#ff0000', '#00ff00', '#0000ff'])
   */
  setDefaultPalette(palette) {
    if (!Array.isArray(palette)) {
      throw new Error("Invalid palette: must be an array of color strings");
    }
    this.defaultPalette = palette;
  }
  /**
   * Set the datasource resolver function for resolving slugs to internal configs.
   *
   * The resolver enables user-friendly datasource references in ChartML specs:
   *   data:
   *     datasource: "production-postgres"  # User-defined slug
   *     query: SELECT * FROM users
   *
   * Instead of hard-to-remember UUIDs:
   *   data:
   *     datasource_id: "ds-2a1b3f5d8e5e4434a609084b2b4233d0"
   *     query: SELECT * FROM users
   *
   * @param {Function} resolver - Async function that resolves slugs to datasource configs
   *   Signature: async (slug, context) => {
   *     provider: string,        // e.g., "postgres", "bigquery"
   *     datasource_id: string,   // Internal UUID (e.g., "ds-abc123")
   *     slug: string,            // The slug that was resolved
   *     connection_config?: object  // Optional connection config
   *   }
   *
   * @example
   * chartml.setDatasourceResolver(async (slug, context) => {
   *   const response = await apiClient.get(`/api/v1/datasources/${slug}`);
   *   const ds = response.data;
   *   return {
   *     provider: ds.datasource_type,
   *     datasource_id: ds.id,
   *     slug: ds.slug
   *   };
   * });
   */
  setDatasourceResolver(resolver) {
    if (typeof resolver !== "function") {
      throw new Error("Datasource resolver must be a function");
    }
    this.datasourceResolver = resolver;
  }
  /**
   * Register built-in transform middleware (d3-array aggregate)
   */
  _registerBuiltInTransform() {
    this.registerTransformMiddleware(d3Transform);
  }
  /**
   * Register built-in data sources (inline and HTTP)
   */
  _registerBuiltInDataSources() {
    this.registerDataSource("inline", async (spec) => {
      if (Array.isArray(spec.rows)) {
        return spec.rows;
      }
      throw new Error("Inline data source requires rows to be an array");
    });
    this.registerDataSource("http", async (spec) => {
      if (typeof spec.data === "string" && (spec.data.startsWith("http://") || spec.data.startsWith("https://"))) {
        const response = await fetch(spec.data);
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
        const data = await response.json();
        if (!Array.isArray(data)) {
          throw new Error("HTTP data source must return a JSON array");
        }
        return data;
      }
      throw new Error("HTTP data source requires data to be a URL string");
    });
  }
  /**
   * Register a custom data source plugin
   *
   * @param {string} name - Data source name (e.g., 'bigquery', 'postgres')
   * @param {Function} handler - Async function that returns data array
   *
   * @example
   * chartml.registerDataSource('bigquery', async (spec) => {
   *   // Execute BigQuery and return rows
   *   return rows;
   * });
   */
  registerDataSource(name, handler) {
    this.dataSources.set(name, handler);
  }
  /**
   * Emit a hook event to registered callbacks
   * @private
   */
  _emitHook(hookName, ...args) {
    if (this.hooks[hookName] && typeof this.hooks[hookName] === "function") {
      try {
        this.hooks[hookName](...args);
      } catch (error) {
        console.error(`[ChartML] Hook ${hookName} error:`, error);
      }
    }
  }
  /**
   * Register transform middleware plugin
   *
   * @param {Function} middleware - Async function that transforms data
   *
   * @example
   * chartml.registerTransformMiddleware(async (data, spec) => {
   *   // Transform data using DuckDB or other engine
   *   return transformedData;
   * });
   */
  registerTransformMiddleware(middleware) {
    this.transformMiddleware.push(middleware);
  }
  /**
   * Set transform middleware, replacing any existing middleware (including defaults)
   *
   * This is the preferred method when you want to replace the default d3Transform
   * middleware with a custom implementation like DuckDB.
   *
   * @param {Function} middleware - Async function that transforms data
   *
   * @example
   * chartml.setTransformMiddleware(async (data, spec) => {
   *   // Replace default d3 aggregation with DuckDB
   *   return duckDbTransform(data, spec);
   * });
   */
  setTransformMiddleware(middleware) {
    this.transformMiddleware = [middleware];
  }
  /**
   * Register a chart renderer plugin
   *
   * @param {string} type - Chart type (e.g., 'bar', 'line', 'pie')
   * @param {Function} renderer - Renderer function (container, data, config) => void
   *
   * Optional Plugin Interface:
   * Renderers can provide custom default dimensions by implementing:
   * renderer.getDefaultDimensions = (spec, container) => ({ height: number, width?: number })
   *
   * This allows plugins to override the default 400px height with chart-type-specific defaults.
   * For example, metric cards might return { height: 150 } for a more compact display.
   *
   * @example
   * // Basic renderer
   * chartml.registerChartRenderer('bar', (container, data, config) => {
   *   // Render bar chart using D3, Canvas, or any library
   * });
   *
   * @example
   * // Renderer with custom default dimensions
   * const renderer = (container, data, config) => {
   *   // Render metric card
   * };
   * renderer.getDefaultDimensions = () => ({ height: 150 });
   * chartml.registerChartRenderer('metric', renderer);
   */
  registerChartRenderer(type, renderer) {
    if (this.chartRenderers.has(type)) {
      console.warn(
        `⚠️  ChartML: Renderer "${type}" is already registered and will be overwritten.
   Consider using a namespaced type (e.g., "@yourorg/${type}") to avoid conflicts.`
      );
    }
    this.chartRenderers.set(type, renderer);
  }
  /**
   * Resolve data source - determine which handler to use
   */
  async _resolveDataSource(spec, options = {}) {
    if (Array.isArray(spec.data)) {
      const handler = this.dataSources.get("inline");
      return await handler(spec, options);
    }
    if (typeof spec.data === "string" && (spec.data.startsWith("http://") || spec.data.startsWith("https://"))) {
      const handler = this.dataSources.get("http");
      return await handler(spec, options);
    }
    if (spec.data && typeof spec.data === "object" && spec.data.type) {
      const handler = this.dataSources.get(spec.data.type);
      if (!handler) {
        throw new Error(`Unknown data source type: ${spec.data.type}`);
      }
      return await handler(spec, options);
    }
    throw new Error("Unable to resolve data source. Data must be an array, URL string, or object with type property.");
  }
  /**
   * Apply transform middleware (includes filter + aggregate stages)
   *
   * MIDDLEWARE-CONTROLLED CACHING:
   * The middleware receives a lazy data fetch callback in context.fetchData.
   * The middleware decides whether to:
   * - Return cached results (never calls fetchData)
   * - Call fetchData() to get fresh data
   *
   * ChartML core is completely unaware of caching - only middleware knows.
   *
   * @param {Function} fetchData - Lazy data fetch callback (only called by middleware if needed)
   * @param {Object} spec - Full chart spec (for cache key generation)
   * @param {Object} context - Context with hooks, options, etc.
   * @returns {Promise<Array>} Processed data
   */
  async _applyTransform(fetchData, spec, context = {}) {
    if (this.transformMiddleware.length === 0) {
      return await fetchData();
    }
    const middlewareContext = {
      ...context,
      fetchData,
      // Lazy data source callback
      spec
      // Full spec for cache key generation
    };
    let result = null;
    for (const middleware of this.transformMiddleware) {
      result = await middleware(result, spec, middlewareContext);
    }
    return result;
  }
  /**
   * Register a component (source, style, config, or params)
   *
   * @param {string|object} spec - ChartML YAML string or parsed object
   * @returns {Object} Parsed component with type information
   * @throws {Error} If component is invalid or a chart type
   *
   * @example
   * chartml.registerComponent(`
   *   type: source
   *   name: sales_data
   *   provider: inline
   *   data:
   *     - month: Jan
   *       revenue: 45000
   * `);
   */
  registerComponent(spec) {
    const component = parseComponent(typeof spec === "string" ? spec : yaml.dump(spec));
    if (component.type === COMPONENT_TYPES.CHART) {
      throw new Error("Cannot register chart components. Use render() method instead.");
    }
    switch (component.type) {
      case COMPONENT_TYPES.SOURCE:
        this.registry.registerSource(component.spec.name, component.spec);
        break;
      case COMPONENT_TYPES.STYLE:
        this.registry.registerStyle(component.spec.name, component.spec);
        break;
      case COMPONENT_TYPES.CONFIG:
        this.registry.registerConfig(component.spec);
        break;
      case COMPONENT_TYPES.PARAMS:
        this.registry.registerParams(component.spec.name, component.spec);
        break;
      default:
        throw new Error(`Unknown component type: ${component.type}`);
    }
    return component;
  }
  /**
   * Resolve data source with registry support
   * Supports both inline data and references to registered sources
   */
  async _resolveDataSource(spec, options = {}) {
    if (typeof spec.data === "string" && !spec.data.startsWith("http://") && !spec.data.startsWith("https://")) {
      const source = this.registry.resolveSource(spec.data);
      if (!source) {
        throw new Error(`Data source "${spec.data}" not found. Did you register it first?`);
      }
      if (source.provider === "inline") {
        return source.rows || source.data;
      } else if (source.provider === "http") {
        const response = await fetch(source.endpoint);
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
        const data = await response.json();
        if (!Array.isArray(data)) {
          throw new Error("HTTP data source must return a JSON array");
        }
        return data;
      } else {
        let handler = this.dataSources.get(source.provider);
        if (!handler) {
          handler = globalRegistry.getDataSource(source.provider);
        }
        if (!handler) {
          throw new Error(`Unknown data source provider: ${source.provider}`);
        }
        return await handler(source, { hooks: this.hooks, ...options });
      }
    }
    if (spec.dataSource && typeof spec.dataSource === "string") {
      const source = this.registry.resolveSource(spec.dataSource);
      if (!source) {
        throw new Error(`Data source "${spec.dataSource}" not found. Did you register it first?`);
      }
      if (source.provider === "inline") {
        return source.rows || source.data;
      } else if (source.provider === "http") {
        const response = await fetch(source.endpoint);
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
        const data = await response.json();
        if (!Array.isArray(data)) {
          throw new Error("HTTP data source must return a JSON array");
        }
        return data;
      } else {
        let handler = this.dataSources.get(source.provider);
        if (!handler) {
          handler = globalRegistry.getDataSource(source.provider);
        }
        if (!handler) {
          throw new Error(`Unknown data source provider: ${source.provider}`);
        }
        return await handler(source, { hooks: this.hooks, ...options });
      }
    }
    if (Array.isArray(spec.data)) {
      const handler = this.dataSources.get("inline");
      return await handler(spec);
    }
    if (typeof spec.data === "string" && (spec.data.startsWith("http://") || spec.data.startsWith("https://"))) {
      const handler = this.dataSources.get("http");
      return await handler(spec);
    }
    if (spec.data && typeof spec.data === "object" && spec.data.datasource) {
      const resolver = this.datasourceResolver || globalRegistry.getDatasourceResolver();
      if (!resolver) {
        throw new Error(
          'Datasource resolver not configured. Call globalRegistry.setDatasourceResolver() or chartml.setDatasourceResolver() before using datasource slugs, or use provider: "postgres" instead of datasource: "slug".'
        );
      }
      const resolved = await resolver(spec.data.datasource, options);
      let handler = this.dataSources.get(resolved.provider);
      if (!handler) {
        handler = globalRegistry.getDataSource(resolved.provider);
      }
      if (!handler) {
        throw new Error(`Unknown data source provider: ${resolved.provider}`);
      }
      const enrichedSpec = {
        ...spec.data,
        datasource_id: resolved.datasource_id,
        // Add internal ID for plugin
        provider: resolved.provider,
        // Ensure provider is set
        // Keep original slug for logging/debugging
        _resolved_slug: spec.data.datasource
      };
      return await handler(enrichedSpec, {
        hooks: this.hooks,
        ...options,
        resolvedDatasource: resolved
      });
    }
    if (spec.data && typeof spec.data === "object" && spec.data.provider) {
      let handler = this.dataSources.get(spec.data.provider);
      if (!handler) {
        handler = globalRegistry.getDataSource(spec.data.provider);
      }
      if (!handler) {
        throw new Error(`Unknown data source provider: ${spec.data.provider}`);
      }
      return await handler(spec.data, { hooks: this.hooks });
    }
    throw new Error('Unable to resolve data source. Provide "data:" as either a string (source reference), array (inline rows), object with "datasource" slug, or object with "provider" property.');
  }
  /**
   * Calculate dimensions for chart rendering
   * DOES NOT modify or return style - only calculates width/height
   *
   * @param {Object} spec - Chart specification
   * @param {HTMLElement} [container] - Optional container to read width from for responsive sizing
   * @returns {Object} - {width, height} calculated dimensions
   */
  _calculateDimensions(spec, container = null) {
    var _a, _b;
    let style = ((_a = spec.visualize) == null ? void 0 : _a.style) || {};
    if (typeof style === "string") {
      const registeredStyle = this.registry.resolveStyle(style);
      if (!registeredStyle) {
        throw new Error(`Style "${style}" not found. Did you register it first?`);
      }
      style = registeredStyle;
    }
    const containerWidth = (container == null ? void 0 : container.offsetWidth) || 600;
    let defaultHeight = 400;
    const chartType = (_b = spec.visualize) == null ? void 0 : _b.type;
    if (chartType) {
      const renderer = this.chartRenderers.get(chartType);
      if (renderer == null ? void 0 : renderer.getDefaultDimensions) {
        try {
          const pluginDefaults = renderer.getDefaultDimensions(spec, container);
          if (pluginDefaults == null ? void 0 : pluginDefaults.height) {
            defaultHeight = pluginDefaults.height;
          }
        } catch (error) {
          console.warn(`[ChartML] Plugin dimension provider for "${chartType}" failed:`, error);
        }
      }
    }
    return {
      width: style.width || containerWidth,
      height: style.height || defaultHeight
    };
  }
  /**
   * Render ChartML specification into a DOM container
   *
   * Supports both ChartML v1.0 components (with type field) and legacy format.
   * For v1.0 components:
   * - source/style/config components are registered, not rendered
   * - chart components are rendered with reference resolution
   *
   * @param {string|object} spec - ChartML YAML string or parsed object
   * @param {HTMLElement} container - DOM element to render into
   * @param {Object} options - Rendering options
   * @param {HTMLElement} [options.filterContainer] - Optional separate container for filter controls
   * @param {Object} [options.filterValues] - Initial filter values to override defaults
   * @param {string} [options.paramsClassName] - Optional CSS classes for params container (e.g., Tailwind classes)
   * @returns {Object|null} Component info if registered, null if rendered
   *
   * @example
   * // Register a source
   * chartml.render(`
   *   type: source
   *   name: sales_data
   *   provider: inline
   *   data: [...]
   * `, container); // Returns component info, doesn't render
   *
   * // Render a chart that references the source
   * await chartml.render(`
   *   type: chart
   *   dataSource: sales_data
   *   visualize:
   *     type: bar
   *     columns: month
   *     rows: revenue
   * `, container); // Renders the chart
   *
   * // Render with filters
   * await chartml.render(spec, chartContainer, {
   *   filterContainer: document.getElementById('filters')
   * });
   */
  async render(spec, container, options = {}) {
    let parsedSpec = typeof spec === "string" ? yaml.load(spec) : spec;
    if (parsedSpec.type) {
      try {
        const component = parseComponent(typeof spec === "string" ? spec : yaml.dump(spec));
        if (component.type === COMPONENT_TYPES.SOURCE || component.type === COMPONENT_TYPES.STYLE || component.type === COMPONENT_TYPES.CONFIG) {
          return this.registerComponent(spec);
        }
        if (component.type === COMPONENT_TYPES.PARAMS) {
          this.registerComponent(spec);
          const paramsSpec = component.spec;
          const paramValues2 = this.registry.getParamValues(paramsSpec.name);
          renderParams(
            paramsSpec.params,
            paramValues2,
            (paramId, newValue) => {
              this.registry.setParamValue(paramsSpec.name, paramId, newValue);
            },
            container,
            options.paramsClassName || ""
          );
          return component;
        }
      } catch (error) {
        container.innerHTML = "";
        const errorDiv = document.createElement("div");
        errorDiv.style.cssText = "padding: 1rem; background: #fef2f2; color: #991b1b; border-left: 4px solid #dc2626; border-radius: 4px;";
        const errorLabel = document.createElement("strong");
        errorLabel.textContent = "ChartML Error: ";
        const errorText = document.createTextNode(error.message);
        errorDiv.appendChild(errorLabel);
        errorDiv.appendChild(errorText);
        container.appendChild(errorDiv);
        throw error;
      }
    }
    let paramsDefinition = null;
    let paramValues = {};
    const originalSpec = parsedSpec;
    if (parsedSpec.params && typeof parsedSpec.params === "string") {
      paramsDefinition = this.registry.resolveParams(parsedSpec.params);
      if (!paramsDefinition) {
        throw new Error(`Params "${parsedSpec.params}" not found. Register it first.`);
      }
      paramValues = this.registry.getParamValues(parsedSpec.params);
      parsedSpec = resolveParamReferences(parsedSpec, paramValues);
    } else if (parsedSpec.params && Array.isArray(parsedSpec.params)) {
      paramsDefinition = { params: parsedSpec.params };
      paramValues = {};
      parsedSpec.params.forEach((param) => {
        if (param.default !== void 0) {
          paramValues[param.id] = param.default;
        }
      });
      parsedSpec = resolveParamReferences(parsedSpec, paramValues, parsedSpec.params);
    }
    this.paramsDefinition = paramsDefinition;
    this.paramValues = paramValues;
    let chartContainer = container;
    let paramsContainer = options.filterContainer;
    if (paramsDefinition && paramsDefinition.params && !options.filterContainer) {
      container.innerHTML = "";
      paramsContainer = document.createElement("div");
      paramsContainer.className = "chartml-params-container";
      container.appendChild(paramsContainer);
      chartContainer = document.createElement("div");
      chartContainer.className = "chartml-chart-container chartml-chart";
      try {
        const { height } = this.getExpectedDimensions(parsedSpec);
        chartContainer.style.minHeight = `${height}px`;
      } catch (err) {
        chartContainer.style.minHeight = "400px";
      }
      container.appendChild(chartContainer);
    } else {
      chartContainer.classList.add("chartml-chart");
    }
    this.chartContainer = chartContainer;
    this.filterContainer = paramsContainer || container;
    if (paramsDefinition && paramsDefinition.params) {
      const targetContainer = paramsContainer || container;
      renderParams(
        paramsDefinition.params,
        paramValues,
        (paramId, newValue) => {
          paramValues[paramId] = newValue;
          if (originalSpec.params && typeof originalSpec.params === "string") {
            this.registry.setParamValue(originalSpec.params, paramId, newValue);
          }
          const resolvedSpec = Array.isArray(originalSpec.params) ? resolveParamReferences(originalSpec, paramValues, originalSpec.params) : resolveParamReferences(originalSpec, paramValues);
          this._renderChartWithParams(chartContainer, { ...options, spec: resolvedSpec });
        },
        targetContainer,
        options.paramsClassName || ""
      );
    }
    const onSpecChange = async (modifications) => {
      const modifiedSpec = {
        ...originalSpec,
        transform: {
          ...originalSpec.transform || {},
          ...modifications.transform || {}
        }
      };
      await this._renderChartWithParams(chartContainer, { ...options, spec: modifiedSpec, onSpecChange });
    };
    try {
      const chartMetadata = await this._renderChartWithParams(chartContainer, { ...options, spec: parsedSpec, onSpecChange });
      return new Chart(this, originalSpec, container, options, chartMetadata);
    } catch (error) {
      if (options.onError) {
        options.onError(error);
      }
      throw error;
    }
  }
  /**
   * Internal method to render the chart with parameter resolution
   * (separated to allow re-rendering on param changes)
   * @private
   *
   * ARCHITECTURE: Two-Layer System
   * Layer 1: Immutable Spec (this.currentSpec) - NEVER modified
   * Layer 2: Render Context (local variable) - Built fresh each render, discarded after
   */
  async _renderChartWithParams(container, options = {}) {
    var _a;
    const loadingIndicator = options.bypassCache ? null : showLoadingIndicator(container, this.loadingIndicator);
    try {
      const immutableSpec = options.spec;
      if (!immutableSpec) {
        throw new Error("[ChartML] _renderChartWithParams requires spec in options");
      }
      const context = {
        // Reference to immutable spec
        spec: immutableSpec,
        // Extract source name from immutable spec (for refresh coordination)
        sourceName: null,
        // Resolved spec with params applied (new object, doesn't mutate original)
        resolvedSpec: null,
        // Resolved data source definition (for cache keys)
        resolvedDataSource: null,
        // Processed data from middleware
        processedData: null,
        // Calculated dimensions
        dimensions: null,
        // Metadata from data source and middleware
        metadata: {}
      };
      if (typeof immutableSpec.data === "string" && !immutableSpec.data.startsWith("http://") && !immutableSpec.data.startsWith("https://")) {
        context.sourceName = immutableSpec.data;
      } else if (typeof immutableSpec.dataSource === "string") {
        context.sourceName = immutableSpec.dataSource;
      }
      const allParamValues = {};
      const registry = this.registry;
      if (registry && registry.params) {
        for (const [scopeName, paramsBlock] of registry.params) {
          const values = paramsBlock.values || {};
          for (const [paramId, value] of Object.entries(values)) {
            allParamValues[`${scopeName}.${paramId}`] = value;
          }
        }
      }
      context.resolvedSpec = resolveParamReferences(
        immutableSpec,
        allParamValues,
        immutableSpec.params
        // Chart-level inline params (if any)
      );
      if (typeof context.resolvedSpec.data === "string" && this.registry) {
        const sourceName = context.resolvedSpec.data;
        context.resolvedDataSource = this.registry.resolveSource(sourceName);
      }
      const fetchData = async (sourceName) => {
        if (sourceName && isNamedSources(context.resolvedSpec.data)) {
          const sourceSpec = context.resolvedSpec.data[sourceName];
          if (!sourceSpec) {
            throw new Error(`Data source "${sourceName}" not found in spec.data`);
          }
          const singleSourceSpec = { ...context.resolvedSpec, data: sourceSpec };
          return await this._resolveDataSource(singleSourceSpec, options);
        }
        return await this._resolveDataSource(context.resolvedSpec, options);
      };
      const result = await this._applyTransform(fetchData, context.resolvedSpec, {
        hooks: this.hooks,
        bypassCache: options.bypassCache,
        resolvedDataSource: context.resolvedDataSource
      });
      context.processedData = (result == null ? void 0 : result.data) !== void 0 ? result.data : result;
      context.metadata = (result == null ? void 0 : result.metadata) || {};
      if (Array.isArray(context.processedData) && context.processedData.length === 0) {
        hideLoadingIndicator(loadingIndicator);
        container.innerHTML = "";
        const emptyDimensions = this._calculateDimensions(context.resolvedSpec, container);
        const emptyState = document.createElement("div");
        emptyState.className = "chartml-empty-state";
        emptyState.style.cssText = `display: flex; flex-direction: column; align-items: center; justify-content: center; height: ${emptyDimensions.height}px; color: var(--chartml-text-secondary); font-family: system-ui, -apple-system, sans-serif;`;
        const icon = document.createElement("div");
        icon.style.cssText = "font-size: 32px; margin-bottom: 8px; opacity: 0.5;";
        icon.innerHTML = `<svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M3 3v18h18"/><path d="M7 16l4-8 4 4 4-6" opacity="0.3"/></svg>`;
        const message = document.createElement("div");
        message.style.cssText = "font-size: 14px; font-weight: 500;";
        message.textContent = "No data available";
        const hint = document.createElement("div");
        hint.style.cssText = "font-size: 12px; margin-top: 4px; opacity: 0.7;";
        hint.textContent = "The query returned no results";
        emptyState.appendChild(icon);
        emptyState.appendChild(message);
        emptyState.appendChild(hint);
        container.appendChild(emptyState);
        if (context.resolvedSpec.title) {
          const titleDiv = document.createElement("div");
          titleDiv.className = "chart-title";
          titleDiv.style.cssText = "font-size: 16px; font-weight: 600; color: var(--chartml-text-strong); margin-bottom: 8px;";
          titleDiv.textContent = context.resolvedSpec.title;
          container.insertBefore(titleDiv, container.firstChild);
        }
        const chartMetadata2 = {
          ...context.metadata,
          dimensions: this._calculateDimensions(context.resolvedSpec, container),
          sourceName: context.sourceName,
          isEmpty: true
        };
        return chartMetadata2;
      }
      context.dimensions = this._calculateDimensions(context.resolvedSpec, container);
      const animationEnabled = ((_a = context.resolvedSpec.style) == null ? void 0 : _a.animation) !== void 0 ? context.resolvedSpec.style.animation : this.animation;
      const instanceConfig = {
        defaultPalette: this.defaultPalette,
        dimensions: context.dimensions,
        animation: animationEnabled
      };
      const { chartType, config, data: chartData } = mapChartMLToD3Config(
        context.resolvedSpec.visualize,
        context.processedData,
        context.resolvedSpec.title,
        instanceConfig
      );
      const enhancedConfig = {
        ...config,
        spec: context.resolvedSpec,
        // Pass the resolved spec to renderer (for reading current state)
        onSpecChange: options.onSpecChange
        // Pass through callback from render()
      };
      let renderer = this.chartRenderers.get(chartType);
      if (!renderer) {
        renderer = globalRegistry.getChartRenderer(chartType);
      }
      if (!renderer) {
        throw new Error(
          `No renderer registered for chart type: ${chartType}. Register a renderer using: chartml.registerChartRenderer('${chartType}', rendererFunction) or import a plugin that auto-registers (e.g., import '@chartml/chart-pie')`
        );
      }
      renderer(container, chartData, enhancedConfig);
      if (context.resolvedSpec.title) {
        let titleDiv = container.querySelector(".chart-title");
        if (!titleDiv) {
          titleDiv = document.createElement("div");
          titleDiv.className = "chart-title";
          titleDiv.style.cssText = "font-size: 16px; font-weight: 600; color: var(--chartml-text-strong); margin-bottom: 8px;";
          container.insertBefore(titleDiv, container.firstChild);
        }
        titleDiv.textContent = context.resolvedSpec.title;
      }
      hideLoadingIndicator(loadingIndicator);
      const chartMetadata = {
        ...context.metadata,
        dimensions: context.dimensions,
        sourceName: context.sourceName
        // Pass extracted source name
      };
      return chartMetadata;
    } catch (error) {
      hideLoadingIndicator(loadingIndicator);
      this._emitHook("onError", error);
      throw error;
    }
  }
  /**
   * Register built-in chart renderers
   * Only cartesian charts (bar, line, area) are built-in.
   * All other chart types (pie, scatter, metric, table, etc.) are plugins.
   * @private
   */
  _registerBuiltInChartRenderers() {
    this.registerChartRenderer("cartesian", (container, data, config) => {
      renderD3CartesianChart(container, data, config);
    });
  }
  /**
   * Get expected dimensions from a ChartML spec without rendering
   *
   * This method calculates the dimensions a chart will have when rendered,
   * allowing wrappers to pre-allocate container space and prevent layout shift
   * during data loading.
   *
   * The dimension calculation follows this priority:
   * 1. Explicit dimensions in spec (visualize.style.width/height)
   * 2. Plugin-provided defaults (via renderer.getDefaultDimensions())
   * 3. ChartML defaults (width: 600px, height: 400px)
   *
   * Note: This is a static method and cannot access plugin defaults.
   * Use the instance method getExpectedDimensions() for full plugin support.
   *
   * @param {string|Object} spec - ChartML specification (YAML string or object)
   * @returns {{width: number|null, height: number}} Expected dimensions
   *          width is null (responsive) unless explicitly set in spec
   *
   * @example
   * // Static method (no plugin defaults)
   * const { width, height } = ChartML.getExpectedDimensions(spec);
   * container.style.minHeight = `${height}px`;
   *
   * @example
   * // Instance method (includes plugin defaults)
   * const chartml = new ChartML();
   * const { width, height } = chartml.getExpectedDimensions(spec);
   * container.style.minHeight = `${height}px`;
   */
  static getExpectedDimensions(spec) {
    var _a;
    try {
      const parsedSpec = typeof spec === "string" ? yaml.load(spec) : spec;
      const hasTitle = !!(parsedSpec == null ? void 0 : parsedSpec.title);
      const titleHeight = hasTitle ? 32 : 0;
      const style = ((_a = parsedSpec.visualize) == null ? void 0 : _a.style) || {};
      return {
        width: style.width || null,
        // Width is responsive unless explicitly set
        height: (style.height || 400) + titleHeight
        // Default height: 400px + title
      };
    } catch (error) {
      console.warn("[ChartML] Failed to parse dimensions from spec:", error);
      return { width: null, height: 400 };
    }
  }
  /**
   * Get expected dimensions from a ChartML spec (instance method with plugin support)
   *
   * This instance method provides the full dimension calculation including plugin defaults.
   * Prefer this over the static method when you have a ChartML instance available.
   *
   * The dimension calculation follows this priority:
   * 1. Explicit dimensions in spec (visualize.style.width/height)
   * 2. Plugin-provided defaults (via renderer.getDefaultDimensions())
   * 3. ChartML defaults (width: null/responsive, height: 400px)
   *
   * @param {string|Object} spec - ChartML specification (YAML string or object)
   * @returns {{width: number|null, height: number}} Expected dimensions
   *          width is null (responsive) unless explicitly set in spec
   *
   * @example
   * const chartml = new ChartML();
   * chartml.registerChartRenderer('metric', metricRenderer);
   * const { width, height } = chartml.getExpectedDimensions(metricSpec);
   * container.style.minHeight = `${height}px`;  // Uses plugin's getDefaultDimensions()
   */
  getExpectedDimensions(spec) {
    var _a, _b, _c, _d, _e;
    try {
      const parsedSpec = typeof spec === "string" ? yaml.load(spec) : spec;
      const hasTitle = !!(parsedSpec == null ? void 0 : parsedSpec.title);
      const titleHeight = hasTitle ? 32 : 0;
      const explicitHeight = (_b = (_a = parsedSpec == null ? void 0 : parsedSpec.visualize) == null ? void 0 : _a.style) == null ? void 0 : _b.height;
      const explicitWidth = (_d = (_c = parsedSpec == null ? void 0 : parsedSpec.visualize) == null ? void 0 : _c.style) == null ? void 0 : _d.width;
      if (explicitHeight) {
        return {
          width: explicitWidth || null,
          height: explicitHeight + titleHeight
        };
      }
      const chartType = (_e = parsedSpec == null ? void 0 : parsedSpec.visualize) == null ? void 0 : _e.type;
      if (chartType && this.chartRenderers) {
        const renderer = this.chartRenderers.get(chartType);
        if (renderer == null ? void 0 : renderer.getDefaultDimensions) {
          try {
            const pluginDefaults = renderer.getDefaultDimensions(parsedSpec, null);
            if (pluginDefaults == null ? void 0 : pluginDefaults.height) {
              const extraTitleHeight = pluginDefaults.includesTitle ? 0 : titleHeight;
              return {
                width: pluginDefaults.width || explicitWidth || null,
                height: pluginDefaults.height + extraTitleHeight
              };
            }
          } catch (error) {
            console.warn(`[ChartML] Plugin dimension provider for "${chartType}" failed:`, error);
          }
        }
      }
      return {
        width: explicitWidth || null,
        // Responsive unless explicit
        height: 400 + titleHeight
      };
    } catch (error) {
      console.warn("[ChartML] Failed to parse dimensions from spec:", error);
      return { width: null, height: 400 };
    }
  }
}
function createDefaultLoadingIndicator() {
  const loader = document.createElement("div");
  loader.className = "chartml-loading-indicator";
  loader.innerHTML = '<div class="chartml-spinner"></div>';
  return loader;
}
function showLoadingIndicator(container, customIndicator) {
  const indicator = customIndicator ? customIndicator() : createDefaultLoadingIndicator();
  const computedPosition = window.getComputedStyle(container).position;
  if (computedPosition === "static") {
    container.style.position = "relative";
  }
  container.appendChild(indicator);
  return indicator;
}
function hideLoadingIndicator(indicator) {
  if (indicator && indicator.parentNode) {
    indicator.parentNode.removeChild(indicator);
  }
}
async function renderChart(spec, container) {
  const chartml = new ChartML();
  await chartml.render(spec, container);
}
export {
  COMPONENT_TYPES,
  ChartML,
  ComponentRegistry,
  DEFAULT_LABEL_FONT_FAMILY,
  DEFAULT_LABEL_FONT_SIZE,
  applyHorizontalLabels,
  applyLabelStrategy,
  applyRotatedLabels,
  applySampledLabels,
  applyTruncatedLabels,
  calculateLegendHeight,
  calculateLegendLayout,
  configure,
  createChartTooltip,
  createFormatter,
  createLegend,
  createRegistry,
  d3Transform,
  determineLabelStrategy,
  extractParamReferences,
  extractReferences,
  getChartColors,
  getGlobalRegistry,
  getStrategicIndices,
  getSystemDefaults,
  globalRegistry,
  measureLabelWidths,
  parseComponent,
  parseMultipleComponents,
  positionTooltip,
  renderChart,
  resetConfig,
  resetGlobalRegistry,
  resolveParamReferences,
  validateParamReferences
};
//# sourceMappingURL=index.js.map
