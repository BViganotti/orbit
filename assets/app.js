/* Orbit Studio — original vanilla JavaScript. No external requests or assets. */
"use strict";

const G0 = 9.80665;
const TOKEN = new URLSearchParams(window.location.search).get("token") || "";

const $ = (id) => document.getElementById(id);
const state = {
  presets: [],
  scripts: [],
  current: null,
  presetId: "atlas",
  runId: null,
  result: null,
  eventSource: null,
  validateTimer: null,
};

function api(path, options = {}) {
  const sep = path.includes("?") ? "&" : "?";
  const url = `${path}${sep}token=${encodeURIComponent(TOKEN)}`;
  return fetch(url, {
    cache: "no-store",
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...(options.headers || {}),
    },
  }).then(async (response) => {
    const text = await response.text();
    let data = null;
    try { data = text ? JSON.parse(text) : null; } catch { data = { error: text }; }
    if (!response.ok) {
      throw new Error((data && (data.error || data.message)) || `HTTP ${response.status}`);
    }
    return data;
  });
}

function fmt(value, digits = 0) {
  if (value === null || value === undefined || !Number.isFinite(Number(value))) return "—";
  return Number(value).toLocaleString(undefined, {
    maximumFractionDigits: digits,
    minimumFractionDigits: digits,
  });
}

function sci(value) {
  if (!Number.isFinite(Number(value))) return "—";
  return Number(value).toExponential(2);
}

function setText(id, text, cls) {
  const node = $(id);
  if (!node) return;
  node.textContent = text;
  if (cls !== undefined) node.className = cls;
}

function clone(obj) { return JSON.parse(JSON.stringify(obj)); }

function sanitizeRocket(rocket) {
  return {
    name: rocket.name,
    payload_mass: Number(rocket.payload_mass),
    drag_area: Number(rocket.drag_area),
    drag_coefficient: Number(rocket.drag_coefficient),
    stages: rocket.stages.map((stage) => ({
      name: stage.name,
      structure_mass: Number(stage.structure_mass),
      engine_count: Number(stage.engine_count),
      reserve_fraction: Number(stage.reserve_fraction || 0),
      engine: {
        name: stage.engine.name,
        thrust_vacuum: Number(stage.engine.thrust_vacuum),
        thrust_sea_level: Number(stage.engine.thrust_sea_level),
        isp_vacuum: Number(stage.engine.isp_vacuum),
        isp_sea_level: Number(stage.engine.isp_sea_level),
        mass: Number(stage.engine.mass),
        min_throttle: Number(stage.engine.min_throttle || 0.4),
        gimbal_deg: Number(stage.engine.gimbal_deg || 5),
      },
      tanks: stage.tanks.map((tank) => ({
        name: tank.name,
        propellant_mass: Number(tank.propellant_mass),
        dry_mass: Number(tank.dry_mass),
        propellant: {
          name: tank.propellant.name,
          density: Number(tank.propellant.density),
        },
      })),
    })),
  };
}

function localSummary(rocket) {
  const stages = rocket.stages || [];
  const stageWet = (stage) => Number(stage.structure_mass) +
    stage.tanks.reduce((a, t) => a + Number(t.propellant_mass) + Number(t.dry_mass), 0) +
    Number(stage.engine_count) * Number(stage.engine.mass);
  const stageDry = (stage) => Number(stage.structure_mass) +
    stage.tanks.reduce((a, t) => a + Number(t.dry_mass), 0) +
    Number(stage.engine_count) * Number(stage.engine.mass);
  const propellant = stages.reduce((a, s) => a + s.tanks.reduce((b, t) => b + Number(t.propellant_mass), 0), 0);
  const liftoff = Number(rocket.payload_mass) + stages.reduce((a, s) => a + stageWet(s), 0);
  let dv = 0;
  stages.forEach((stage, i) => {
    const above = Number(rocket.payload_mass) + stages.slice(i + 1).reduce((a, s) => a + stageWet(s), 0);
    const m0 = above + stageWet(stage);
    const mf = above + stageDry(stage) + stage.tanks.reduce((a, t) => a + Number(t.propellant_mass), 0) * Number(stage.reserve_fraction || 0);
    if (m0 > 0 && mf > 0 && mf < m0) dv += Number(stage.engine.isp_vacuum) * G0 * Math.log(m0 / mf);
  });
  const first = stages[0];
  const twr = first && liftoff > 0 ? Number(first.engine_count) * Number(first.engine.thrust_sea_level) / (liftoff * G0) : 0;
  return { liftoff, propellant, dv, twr, stages: stages.length, payloadFraction: liftoff > 0 ? Number(rocket.payload_mass) / liftoff : 0 };
}

/* ---------------- tabs ---------------- */
document.querySelectorAll(".tab-btn").forEach((button) => {
  button.addEventListener("click", () => {
    document.querySelectorAll(".tab-btn").forEach((b) => {
      b.classList.toggle("active", b === button);
      b.setAttribute("aria-selected", b === button ? "true" : "false");
    });
    document.querySelectorAll(".tab-panel").forEach((panel) => {
      panel.classList.toggle("active", panel.id === `tab-${button.dataset.tab}`);
    });
    if (button.dataset.tab === "plots" && state.result) drawPlots(state.result);
  });
});

/* ---------------- garage ---------------- */
function renderPresets() {
  const holder = $("preset-list");
  holder.replaceChildren();
  state.presets.forEach((rocket) => {
    const button = document.createElement("button");
    button.className = `preset${rocket.name.toLowerCase() === state.presetId ? " active" : ""}`;
    button.type = "button";
    const name = document.createElement("div");
    name.className = "preset-name";
    name.textContent = rocket.name;
    const meta = document.createElement("div");
    meta.className = "preset-meta";
    const s = rocket.summary || {};
    const span = document.createElement("span");
    span.textContent = `${(s.total_delta_v / 1000).toFixed(1)} km/s · ${s.stage_count || rocket.stages.length} stages`;
    name.appendChild(span);
    meta.textContent = `${(s.liftoff_mass / 1000).toFixed(0)} t · TWR ${(s.liftoff_twr || 0).toFixed(2)}`;
    button.append(name, meta);
    button.addEventListener("click", () => selectPreset(rocket));
    holder.appendChild(button);
  });
}

function selectPreset(rocket) {
  state.presetId = rocket.name.toLowerCase();
  state.current = clone(rocket);
  renderRocket();
  renderPresets();
  validateSoon();
}

function updateField(path, value) {
  if (!state.current) return;
  const parts = path.split(".");
  let target = state.current;
  for (let i = 0; i < parts.length - 1; i += 1) target = target[parts[i]];
  target[parts[parts.length - 1]] = value;
  updateSummary();
  validateSoon();
}

function numberField(label, value, field, stage, step = "1", min = "0", scale = 1) {
  const wrapper = document.createElement("label");
  wrapper.textContent = label;
  const input = document.createElement("input");
  input.type = "number";
  input.step = step;
  input.min = min;
  input.value = Number(value) / scale;
  input.addEventListener("input", () => {
    const next = Number(input.value);
    if (Number.isFinite(next)) updateField(field, next * scale);
  });
  wrapper.appendChild(input);
  return wrapper;
}

function textField(label, value, field, maxLength = 64) {
  const wrapper = document.createElement("label");
  wrapper.textContent = label;
  const input = document.createElement("input");
  input.type = "text";
  input.maxLength = maxLength;
  input.value = value;
  input.addEventListener("input", () => updateField(field, input.value));
  wrapper.appendChild(input);
  return wrapper;
}

function renderStages() {
  const holder = $("stages");
  holder.replaceChildren();
  if (!state.current) return;
  state.current.stages.forEach((stage, index) => {
    const card = document.createElement("div");
    card.className = "stage-card";
    const head = document.createElement("div");
    head.className = "stage-head";
    const label = document.createElement("div");
    label.className = "stage-index";
    label.textContent = `STAGE ${index + 1}`;
    const actions = document.createElement("div");
    actions.className = "row-actions";
    const remove = document.createElement("button");
    remove.className = "btn ghost danger";
    remove.type = "button";
    remove.textContent = "Remove";
    remove.disabled = state.current.stages.length <= 1;
    remove.addEventListener("click", () => {
      state.current.stages.splice(index, 1);
      renderRocket();
      updateSummary();
      validateSoon();
    });
    actions.appendChild(remove);
    head.append(label, actions);

    const fields = document.createElement("div");
    fields.className = "stage-fields";
    const prefix = `stages.${index}`;
    fields.append(
      textField("Name", stage.name, `${prefix}.name`),
      numberField("Structure (kg)", stage.structure_mass, `${prefix}.structure_mass`, index, "100"),
      numberField("Engines", stage.engine_count, `${prefix}.engine_count`, index, "1", "1"),
      numberField("Engine mass (kg)", stage.engine.mass, `${prefix}.engine.mass`, index, "10"),
      numberField("Thrust vac (kN)", stage.engine.thrust_vacuum, `${prefix}.engine.thrust_vacuum`, index, "10", "0", 1000),
      numberField("Thrust SL (kN)", stage.engine.thrust_sea_level, `${prefix}.engine.thrust_sea_level`, index, "10", "0", 1000),
      numberField("Isp vac (s)", stage.engine.isp_vacuum, `${prefix}.engine.isp_vacuum`, index, "1"),
      numberField("Isp SL (s)", stage.engine.isp_sea_level, `${prefix}.engine.isp_sea_level`, index, "1"),
      numberField("Propellant (kg)", stage.tanks[0].propellant_mass, `${prefix}.tanks.0.propellant_mass`, index, "100"),
      numberField("Tank dry (kg)", stage.tanks[0].dry_mass, `${prefix}.tanks.0.dry_mass`, index, "10"),
      numberField("Prop. density", stage.tanks[0].propellant.density, `${prefix}.tanks.0.propellant.density`, index, "1", "1"),
      numberField("Reserve", stage.reserve_fraction || 0, `${prefix}.reserve_fraction`, index, "0.01"),
    );
    const foot = document.createElement("div");
    foot.className = "stage-foot";
    const prop = stage.tanks.reduce((a, t) => a + Number(t.propellant_mass), 0);
    foot.textContent = `${prop.toLocaleString()} kg propellant · ${stage.engine.name}`;

    card.append(head, fields, foot);
    holder.appendChild(card);
  });
}

function renderRocket() {
  if (!state.current) return;
  $("rocket-name").value = state.current.name;
  $("rocket-payload").value = Number(state.current.payload_mass);
  $("rocket-area").value = Number(state.current.drag_area);
  $("rocket-cd").value = Number(state.current.drag_coefficient);
  renderStages();
  const s = localSummary(state.current);
  setText("summary-liftoff", `${fmt(s.liftoff / 1000, 1)} t`);
  setText("summary-payload", `${fmt(s.payloadFraction * 100, 2)} %`);
  setText("summary-dv", `${fmt(s.dv / 1000, 2)} km/s`);
  setText("summary-twr", fmt(s.twr, 2));
  setText("summary-prop", `${fmt(s.propellant / 1000, 1)} t`);
  setText("summary-burn", "—");
  setText("mission-rocket-name", state.current.name);
  setText("mission-mass", `${fmt(s.liftoff / 1000, 1)} t`);
  setText("mission-dv", `${fmt(s.dv / 1000, 2)} km/s`);
  setText("mission-twr", fmt(s.twr, 2));
  setText("mission-stages", String(s.stages));
}

function updateSummary() {
  if (!state.current) return;
  const s = localSummary(state.current);
  setText("summary-liftoff", `${fmt(s.liftoff / 1000, 1)} t`);
  setText("summary-payload", `${fmt(s.payloadFraction * 100, 2)} %`);
  setText("summary-dv", `${fmt(s.dv / 1000, 2)} km/s`);
  setText("summary-twr", fmt(s.twr, 2));
  setText("summary-prop", `${fmt(s.propellant / 1000, 1)} t`);
  setText("mission-rocket-name", state.current.name);
  setText("mission-mass", `${fmt(s.liftoff / 1000, 1)} t`);
  setText("mission-dv", `${fmt(s.dv / 1000, 2)} km/s`);
  setText("mission-twr", fmt(s.twr, 2));
  setText("mission-stages", String(s.stages));
}

function validateSoon() {
  clearTimeout(state.validateTimer);
  state.validateTimer = setTimeout(validateRocket, 420);
}

async function validateRocket() {
  if (!state.current) return;
  setText("garage-status", "checking", "pill warn");
  try {
    const data = await api("/api/validate", {
      method: "POST",
      body: JSON.stringify(state.current),
    });
    const report = data.report;
    setText("garage-status", report.ok ? "nominal" : "errors", `pill ${report.ok ? "ok" : "bad"}`);
    const holder = $("checks");
    holder.replaceChildren();
    report.issues.forEach((issue) => {
      const row = document.createElement("div");
      row.className = `check ${issue.severity}`;
      const badge = document.createElement("b");
      badge.textContent = issue.severity;
      const text = document.createElement("span");
      text.textContent = issue.message;
      row.append(badge, text);
      holder.appendChild(row);
    });
    if (report.summary) {
      setText("summary-dv", `${fmt(report.summary.total_delta_v / 1000, 2)} km/s`);
      setText("summary-twr", fmt(report.summary.liftoff_twr, 2));
      setText("summary-liftoff", `${fmt(report.summary.liftoff_mass / 1000, 1)} t`);
    }
  } catch (error) {
    setText("garage-status", "invalid", "pill bad");
    const holder = $("checks");
    holder.replaceChildren();
    const row = document.createElement("div");
    row.className = "check error";
    row.textContent = error.message;
    holder.appendChild(row);
  }
}

/* ---------------- mission ---------------- */
function populateScripts() {
  const select = $("script-select");
  select.replaceChildren();
  state.scripts.forEach((script) => {
    const option = document.createElement("option");
    option.value = script.id;
    option.textContent = script.name;
    select.appendChild(option);
  });
  select.addEventListener("change", () => loadScript(select.value));
  if (state.scripts.length) loadScript(state.scripts[0].id);
}

function loadScript(id) {
  const script = state.scripts.find((s) => s.id === id);
  if (!script) return;
  $("lunar-altitude").value = script.lunar_altitude_km;
  $("epoch-days").value = script.epoch_days;
  $("moon-phase").value = script.moon_phase_deg;
  $("sun-phase").value = script.sun_phase_deg;
  setText("script-description", script.description || "");
}

function missionPayload() {
  const script = state.scripts.find((s) => s.id === $("script-select").value);
  return {
    name: script ? script.name : "Custom mission",
    rocket: sanitizeRocket(state.current),
    lunar_altitude_m: Number($("lunar-altitude").value) * 1000,
    parking_altitude_m: Number($("parking-altitude").value) * 1000,
    epoch_s: Number($("epoch-days").value) * 86400,
    moon_phase_deg: Number($("moon-phase").value),
    sun_phase_deg: Number($("sun-phase").value),
    tolerance: Number($("tolerance").value) / 100,
  };
}

async function runMission() {
  if (!state.current) return;
  if (state.eventSource) { state.eventSource.close(); state.eventSource = null; }
  setText("mission-status", "starting run…");
  setText("result-pill", "running", "pill warn");
  $("run-btn").disabled = true;
  $("progress-bar").style.width = "12%";
  try {
    const data = await api("/api/mission/run", {
      method: "POST",
      body: JSON.stringify(missionPayload()),
    });
    state.runId = data.run_id;
    setText("mission-status", `run #${data.run_id} computing…`);
    openEvents(data.run_id);
  } catch (error) {
    setText("mission-status", `error: ${error.message}`);
    setText("result-pill", "error", "pill bad");
    $("run-btn").disabled = false;
    $("progress-bar").style.width = "0%";
  }
}

function openEvents(runId) {
  const source = new EventSource(`/api/events?run=${runId}&token=${encodeURIComponent(TOKEN)}`);
  state.eventSource = source;
  $("progress-bar").style.width = "38%";
  const mini = $("mission-events");
  mini.replaceChildren();
  const append = (text, cls) => {
    const line = document.createElement("div");
    if (cls) line.className = cls;
    line.textContent = text;
    mini.appendChild(line);
    mini.scrollTop = mini.scrollHeight;
  };
  source.addEventListener("progress", (event) => {
    try {
      const payload = JSON.parse(event.data);
      append(`› ${payload.message || payload.phase || "working"}`, "phase");
    } catch { append(`› ${event.data}`, "phase"); }
  });
  source.addEventListener("mission", (event) => {
    try {
      const e = JSON.parse(event.data);
      append(`  ${Number(e.t).toFixed(1)}s ${e.phase} ${e.kind}: ${e.detail}`);
    } catch { append(`  ${event.data}`); }
  });
  source.addEventListener("done", async () => {
    source.close();
    state.eventSource = null;
    $("progress-bar").style.width = "100%";
    await finishRun(runId);
  });
  source.addEventListener("error", (event) => {
    if (event.data) append(`error: ${event.data}`, "bad");
    source.close();
    state.eventSource = null;
    $("run-btn").disabled = false;
    setText("result-pill", "stream closed", "pill bad");
  });
  source.onerror = () => {
    if (source.readyState === EventSource.CLOSED) {
      $("run-btn").disabled = false;
    }
  };
}

async function finishRun(runId) {
  try {
    const data = await api(`/api/runs/${runId}`);
    if (data.status === "failed") {
      setText("mission-status", `failed: ${data.message}`);
      setText("result-pill", "failed", "pill bad");
      $("run-btn").disabled = false;
      return;
    }
    state.result = data.result;
    renderResult(data.result);
    setText("mission-status", "complete");
    $("run-btn").disabled = false;
  } catch (error) {
    setText("mission-status", `fetch failed: ${error.message}`);
    $("run-btn").disabled = false;
  }
}

function renderResult(result) {
  const lunar = result.lunar || {};
  const metrics = result.metrics || {};
  const transfer = result.transfer || {};
  setText("result-pill", result.success ? "success" : "failed", `pill ${result.success ? "ok" : "bad"}`);
  setText("result-status", result.status + (result.success ? " ✓" : " ✗"), result.success ? "ok" : "bad");
  setText("result-peri", `${fmt((lunar.periapsis_altitude || 0) / 1000, 1)} km`);
  setText("result-apo", `${fmt((lunar.apoapsis_altitude || 0) / 1000, 1)} km`);
  setText("result-ecc", fmt(lunar.eccentricity || 0, 6));
  setText("result-dv", `${fmt((metrics.delta_v_ascent + metrics.delta_v_parking + metrics.delta_v_transfer + metrics.delta_v_capture) / 1000, 2)} km/s`);
  setText("result-time", `${fmt(metrics.total_time / 86400, 2)} d`);
  setText("result-residual", `${fmt(transfer.residual || 0, 1)} m`);
  setText("result-mass", `${fmt((metrics.final_mass || 0) / 1000, 2)} t`);
  const ascent = result.ascent || {};
  setText("result-maxq", `${fmt((ascent.max_q || 0) / 1000, 1)} kPa`);
  setText("result-maxg", `${fmt(ascent.max_g || 0, 2)} g`);
  setText("result-message", result.message || "");
  $("export-csv").disabled = false;
  $("export-report").disabled = false;
  renderEventTable(result.events || []);
  drawPlots(result);
}

function renderEventTable(events) {
  const body = $("event-table");
  body.replaceChildren();
  setText("event-count", `${events.length} events`);
  events.forEach((event) => {
    const row = document.createElement("tr");
    const cells = [fmt(event.t, 1), event.phase, event.kind, event.detail];
    cells.forEach((value) => {
      const cell = document.createElement("td");
      cell.textContent = value;
      row.appendChild(cell);
    });
    body.appendChild(row);
  });
}

async function downloadExport(kind, filename) {
  if (!state.runId) return;
  try {
    const response = await fetch(`/api/export/${kind}?run=${state.runId}&token=${encodeURIComponent(TOKEN)}`, { cache: "no-store" });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const blob = await response.blob();
    const link = document.createElement("a");
    link.href = URL.createObjectURL(blob);
    link.download = filename;
    document.body.appendChild(link);
    link.click();
    link.remove();
    setTimeout(() => URL.revokeObjectURL(link.href), 3000);
  } catch (error) {
    setText("mission-status", `export failed: ${error.message}`);
  }
}

/* ---------------- plotting ---------------- */
const phaseColors = ["#fbbf24", "#38bdf8", "#5eead4", "#f472b6"];

function plotFrame(canvas) {
  const ctx = canvas.getContext("2d");
  const w = canvas.width;
  const h = canvas.height;
  ctx.clearRect(0, 0, w, h);
  ctx.fillStyle = "#060d19";
  ctx.fillRect(0, 0, w, h);
  return { ctx, w, h, left: 64, right: w - 22, top: 22, bottom: h - 42 };
}

function axis(ctx, frame, xmin, xmax, ymin, ymax, xlabel, ylabel, logy = false) {
  const { left, right, top, bottom } = frame;
  const tx = (x) => left + ((x - xmin) / (xmax - xmin || 1)) * (right - left);
  const ty = (y) => {
    if (logy) {
      const lo = Math.log10(Math.max(1, ymin));
      const hi = Math.log10(Math.max(10, ymax));
      return bottom - ((Math.log10(Math.max(1, y)) - lo) / (hi - lo || 1)) * (bottom - top);
    }
    return bottom - ((y - ymin) / (ymax - ymin || 1)) * (bottom - top);
  };
  ctx.strokeStyle = "#16243a";
  ctx.lineWidth = 1;
  ctx.font = "12px ui-monospace, monospace";
  ctx.fillStyle = "#5d7191";
  for (let i = 0; i <= 5; i += 1) {
    const y = top + ((bottom - top) * i) / 5;
    ctx.beginPath(); ctx.moveTo(left, y); ctx.lineTo(right, y); ctx.stroke();
    const value = logy
      ? Math.pow(10, Math.log10(Math.max(1, ymin)) + ((Math.log10(Math.max(10, ymax)) - Math.log10(Math.max(1, ymin))) * (5 - i)) / 5)
      : ymin + ((ymax - ymin) * (5 - i)) / 5;
    ctx.fillText(value >= 10000 ? `${(value / 1000).toFixed(0)}k` : value.toFixed(value < 10 ? 1 : 0), 8, y + 4);
  }
  for (let i = 0; i <= 6; i += 1) {
    const x = left + ((right - left) * i) / 6;
    ctx.beginPath(); ctx.moveTo(x, top); ctx.lineTo(x, bottom); ctx.stroke();
    const value = xmin + ((xmax - xmin) * i) / 6;
    const label = Math.abs(value) >= 10000 ? `${(value / 1000).toFixed(0)}k` : value.toFixed(value < 10 ? 1 : 0);
    const width = ctx.measureText(label).width;
    ctx.fillText(label, x - width / 2, bottom + 20);
  }
  ctx.strokeStyle = "#2b4266";
  ctx.strokeRect(left, top, right - left, bottom - top);
  ctx.fillStyle = "#8ba0bf";
  ctx.fillText(xlabel, (left + right) / 2 - ctx.measureText(xlabel).width / 2, bottom + 36);
  ctx.save();
  ctx.translate(16, (top + bottom) / 2 + ctx.measureText(ylabel).width / 2);
  ctx.rotate(-Math.PI / 2);
  ctx.fillText(ylabel, 0, 0);
  ctx.restore();
  return { tx, ty };
}

function phaseSegments(points) {
  const segments = [];
  let current = null;
  points.forEach((p) => {
    if (!current || current.phase !== p[4]) {
      current = { phase: p[4], points: [] };
      segments.push(current);
    }
    current.points.push(p);
  });
  return segments;
}

function drawSeries(ctx, points, tx, ty, phaseAware = true) {
  if (!points.length) return;
  ctx.lineWidth = 1.8;
  ctx.lineJoin = "round";
  if (phaseAware) {
    phaseSegments(points).forEach((segment) => {
      ctx.strokeStyle = phaseColors[Math.min(3, Math.max(0, segment.phase))];
      ctx.beginPath();
      segment.points.forEach((p, i) => {
        const x = tx(p[0]);
        const y = ty(p[1]);
        if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
      });
      ctx.stroke();
    });
  } else {
    ctx.strokeStyle = "#5eead4";
    ctx.beginPath();
    points.forEach((p, i) => {
      const x = tx(p[0]);
      const y = ty(p[1]);
      if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
    });
    ctx.stroke();
  }
}

function plotTimeSeries(canvas, result, index, scale, label, logy = false) {
  const samples = (result.samples || []).filter((s) => Number.isFinite(s[index]) && s[index] >= 0);
  const frame = plotFrame(canvas);
  if (!samples.length) {
    frame.ctx.fillStyle = "#5d7191";
    frame.ctx.font = "14px ui-monospace, monospace";
    frame.ctx.fillText("no trajectory samples", frame.left, frame.top + 30);
    return;
  }
  const t0 = samples[0][0];
  const points = samples.map((s) => [(s[0] - t0) / 3600, s[index] * scale, 0, 0, s[4]]);
  const xmax = Math.max(0.1, ...points.map((p) => p[0]));
  let ymax = Math.max(1, ...points.map((p) => p[1]));
  if (logy) ymax = Math.max(10, ymax);
  const { tx, ty } = axis(frame.ctx, frame, 0, xmax, 0, ymax, "hours", label, logy);
  drawSeries(frame.ctx, points, tx, ty);
}

function drawPlots(result) {
  if (!result) return;
  plotTimeSeries($("plot-altitude"), result, 1, 0.001, "altitude (km)", true);
  plotTimeSeries($("plot-speed"), result, 2, 0.001, "speed (km/s)", false);
  plotTimeSeries($("plot-moon"), result, 3, 0.001, "distance (km)", true);
  drawTrajectory(result);
}

function drawTrajectory(result) {
  const canvas = $("plot-trajectory");
  const frame = plotFrame(canvas);
  const samples = result.samples || [];
  if (!samples.length) {
    frame.ctx.fillStyle = "#5d7191";
    frame.ctx.fillText("no trajectory samples", frame.left, frame.top + 30);
    return;
  }
  const xs = samples.map((s) => s[6] / 1000);
  const ys = samples.map((s) => s[7] / 1000);
  const extent = Math.max(9000, ...xs.map(Math.abs), ...ys.map(Math.abs)) * 1.04;
  const { tx, ty } = axis(frame.ctx, frame, -extent, extent, -extent, extent, "x (km)", "y (km)");
  frame.ctx.save();
  frame.ctx.setLineDash([5, 6]);
  frame.ctx.strokeStyle = "#2b4266";
  frame.ctx.beginPath();
  frame.ctx.arc(tx(0), ty(0), Math.abs(tx(384400) - tx(0)), 0, Math.PI * 2);
  frame.ctx.stroke();
  frame.ctx.restore();
  const earthRadius = 6371;
  frame.ctx.fillStyle = "#17395e";
  frame.ctx.strokeStyle = "#38bdf8";
  frame.ctx.beginPath();
  frame.ctx.arc(tx(0), ty(0), Math.max(3, Math.abs(tx(earthRadius) - tx(0))), 0, Math.PI * 2);
  frame.ctx.fill();
  frame.ctx.stroke();
  phaseSegments(samples).forEach((segment) => {
    frame.ctx.strokeStyle = phaseColors[Math.min(3, Math.max(0, segment.phase))];
    frame.ctx.lineWidth = 1.9;
    frame.ctx.beginPath();
    segment.points.forEach((s, i) => {
      const x = tx(s[6] / 1000);
      const y = ty(s[7] / 1000);
      if (i === 0) frame.ctx.moveTo(x, y); else frame.ctx.lineTo(x, y);
    });
    frame.ctx.stroke();
  });
  frame.ctx.fillStyle = "#e6eefc";
  frame.ctx.font = "12px ui-monospace, monospace";
  frame.ctx.fillText("Earth", tx(0) + 8, ty(0) - 8);
  frame.ctx.fillStyle = "#5d7191";
  frame.ctx.fillText("Moon orbit (384,400 km)", frame.left + 10, frame.top + 18);
}

/* ---------------- bootstrap ---------------- */
async function boot() {
  if (!TOKEN) {
    $("token-warning").hidden = false;
    setText("connection-text", "no token");
    $("connection").className = "connection bad";
    return;
  }
  $("footer-token").textContent = `token ${TOKEN.slice(0, 6)}…`;
  try {
    await api("/api/health");
    setText("connection-text", "loopback online");
    $("connection").className = "connection ok";
  } catch (error) {
    setText("connection-text", "offline");
    $("connection").className = "connection bad";
    return;
  }
  try {
    const [presets, scripts] = await Promise.all([api("/api/presets"), api("/api/scripts")]);
    state.presets = presets.rockets || [];
    state.scripts = scripts.scripts || [];
    if (state.presets.length) {
      selectPreset(state.presets.find((r) => r.name.toLowerCase() === "atlas") || state.presets[0]);
    }
    populateScripts();
    renderPresets();
  } catch (error) {
    setText("garage-status", "load failed", "pill bad");
    setText("mission-status", error.message);
  }
}

$("rocket-name").addEventListener("input", (e) => updateField("name", e.target.value));
$("rocket-payload").addEventListener("input", (e) => updateField("payload_mass", Number(e.target.value)));
$("rocket-area").addEventListener("input", (e) => updateField("drag_area", Number(e.target.value)));
$("rocket-cd").addEventListener("input", (e) => updateField("drag_coefficient", Number(e.target.value)));
$("validate-btn").addEventListener("click", validateRocket);
$("reset-rocket").addEventListener("click", () => {
  const rocket = state.presets.find((r) => r.name.toLowerCase() === state.presetId);
  if (rocket) selectPreset(rocket);
});
$("add-stage").addEventListener("click", () => {
  if (!state.current || state.current.stages.length >= 8) return;
  const copy = clone(state.current.stages[state.current.stages.length - 1]);
  copy.name = `${copy.name} B`;
  state.current.stages.push(copy);
  renderRocket();
  updateSummary();
  validateSoon();
});
$("run-btn").addEventListener("click", runMission);
$("demo-btn").addEventListener("click", () => {
  const script = state.scripts.find((s) => s.id === "lunar-atlas-100") || state.scripts[0];
  if (script) {
    $("script-select").value = script.id;
    loadScript(script.id);
  }
  runMission();
});
$("export-csv").addEventListener("click", () => downloadExport("csv", "orbit-trajectory.csv"));
$("export-report").addEventListener("click", () => downloadExport("report", "orbit-report.txt"));
$("conservation-btn").addEventListener("click", async () => {
  setText("conservation-result", "integrating 30 days…");
  try {
    const data = await api("/api/conservation");
    setText("conservation-result",
      `30 d · ${data.steps} steps · energy drift ${sci(data.relative_energy_error)} · momentum ${sci(data.relative_momentum_error)}`);
  } catch (error) {
    setText("conservation-result", `failed: ${error.message}`);
  }
});

document.addEventListener("keydown", (event) => {
  const tag = (event.target && event.target.tagName || "").toLowerCase();
  if (["input", "select", "textarea", "button"].includes(tag)) return;
  const tabs = { "1": "garage", "2": "mission", "3": "plots", "4": "events" };
  if (tabs[event.key]) {
    const button = document.querySelector(`.tab-btn[data-tab='${tabs[event.key]}']`);
    if (button) button.click();
  }
});
window.addEventListener("resize", () => { if (state.result) drawPlots(state.result); });
boot();
