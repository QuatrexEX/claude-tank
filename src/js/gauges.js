/* Claude Tank — Gauge rendering logic
 * Note: All data rendered via innerHTML originates from our own internal
 * API (numeric values + locale strings). No untrusted user input is rendered. */

function gaugeColorClass(pct, isRemaining) {
  const val = isRemaining ? pct : (100 - pct);
  if (val > 50) return 'gauge-safe';
  if (val > 20) return 'gauge-caution';
  if (val > 5) return 'gauge-warning';
  return 'gauge-danger';
}

function gaugeColor(pct, isRemaining) {
  const val = isRemaining ? pct : (100 - pct);
  if (val > 50) return '#22c55e';
  if (val > 20) return '#eab308';
  if (val > 5) return '#f97316';
  return '#ef4444';
}

function formatResetTime(isoStr) {
  if (!isoStr) return '';
  const reset = new Date(isoStr);
  const now = new Date();
  const diff = reset - now;
  if (diff <= 0) return 'now';
  const hours = Math.floor(diff / 3600000);
  const minutes = Math.floor((diff % 3600000) / 60000);
  if (hours >= 24) {
    const days = Math.floor(hours / 24);
    const rem = hours % 24;
    return days + 'd ' + rem + 'h';
  }
  return hours + 'h ' + minutes + 'm';
}

function renderGauge(container, opts) {
  var title = opts.title;
  var utilization = opts.utilization;
  var resetAt = opts.resetAt;
  var gaugeMode = opts.gaugeMode;
  var strings = opts.strings;

  var isRemaining = gaugeMode === 'remaining';
  var displayPct = isRemaining ? (100 - utilization) : utilization;
  var colorClass = gaugeColorClass(displayPct, isRemaining);
  var label = isRemaining ? (strings.left || 'left') : (strings.used || 'used');
  var resetLabel = strings.reset_in || 'Reset in';
  var color = gaugeColor(displayPct, isRemaining);
  var pctInt = Math.round(displayPct);

  // Build DOM safely
  container.textContent = '';

  var wrapper = document.createElement('div');
  wrapper.className = 'gauge-container';

  var header = document.createElement('div');
  header.className = 'gauge-header';

  var titleEl = document.createElement('span');
  titleEl.className = 'gauge-title';
  titleEl.textContent = title;
  header.appendChild(titleEl);

  var valueEl = document.createElement('span');
  valueEl.className = 'gauge-value';
  valueEl.style.color = color;
  valueEl.textContent = pctInt + '% ';
  var labelSpan = document.createElement('span');
  labelSpan.style.cssText = 'font-size:12px;font-weight:400;opacity:0.7';
  labelSpan.textContent = label;
  valueEl.appendChild(labelSpan);
  header.appendChild(valueEl);

  wrapper.appendChild(header);

  var track = document.createElement('div');
  track.className = 'gauge-track';
  var fill = document.createElement('div');
  fill.className = 'gauge-fill ' + gaugeMode + ' ' + colorClass;
  fill.style.width = displayPct + '%';
  track.appendChild(fill);
  wrapper.appendChild(track);

  if (resetAt) {
    var resetEl = document.createElement('div');
    resetEl.className = 'gauge-reset';
    resetEl.textContent = '\u23F1 ' + resetLabel + ' ' + formatResetTime(resetAt);
    wrapper.appendChild(resetEl);
  }

  container.appendChild(wrapper);
}

function renderMiniGauge(opts) {
  var label = opts.label;
  var utilization = opts.utilization;
  var gaugeMode = opts.gaugeMode;

  var isRemaining = gaugeMode === 'remaining';
  var displayPct = isRemaining ? (100 - utilization) : utilization;
  var color = gaugeColor(displayPct, isRemaining);
  var pctInt = Math.round(displayPct);

  var outer = document.createElement('div');
  outer.className = 'mini-gauge';

  var lbl = document.createElement('span');
  lbl.className = 'mini-gauge-label';
  lbl.textContent = label;
  outer.appendChild(lbl);

  var track = document.createElement('div');
  track.className = 'mini-gauge-track';
  var fill = document.createElement('div');
  fill.className = 'mini-gauge-fill';
  fill.style.width = displayPct + '%';
  fill.style.background = color;
  track.appendChild(fill);
  outer.appendChild(track);

  var val = document.createElement('span');
  val.className = 'mini-gauge-value';
  val.style.color = color;
  val.textContent = pctInt + '%';
  outer.appendChild(val);

  return outer;
}
