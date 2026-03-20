/* Claude Tank — Dashboard / Widget / Hover panel main logic
 * All data rendered comes from our internal API (numbers + locale strings).
 * No untrusted user content is ever rendered. */

var currentData = null;
var currentConfig = null;
var strings = {};

function updateDashboard(data, config, i18nStrings) {
  currentData = data;
  currentConfig = config;
  if (i18nStrings) strings = i18nStrings;
  if (!data) return;

  var mode = config.gauge_mode || 'remaining';

  var g5h = document.getElementById('gauge-5h');
  if (g5h) {
    renderGauge(g5h, {
      title: strings.five_hour || '5-Hour Limit',
      utilization: data.five_hour,
      resetAt: data.five_hour_reset,
      gaugeMode: mode,
      strings: strings,
    });
  }

  var g7d = document.getElementById('gauge-7d');
  if (g7d) {
    renderGauge(g7d, {
      title: strings.seven_day || '7-Day Limit',
      utilization: data.seven_day,
      resetAt: data.seven_day_reset,
      gaugeMode: mode,
      strings: strings,
    });
  }

  // Model gauges
  var modelSection = document.getElementById('model-gauges');
  if (modelSection) {
    modelSection.textContent = '';
    if (data.opus > 0 || data.opus_reset) {
      var opusDiv = document.createElement('div');
      opusDiv.id = 'gauge-opus';
      modelSection.appendChild(opusDiv);
      renderGauge(opusDiv, { title: 'Opus', utilization: data.opus, resetAt: data.opus_reset, gaugeMode: mode, strings: strings });
    }
    if (data.sonnet > 0 || data.sonnet_reset) {
      var sonnetDiv = document.createElement('div');
      sonnetDiv.id = 'gauge-sonnet';
      modelSection.appendChild(sonnetDiv);
      renderGauge(sonnetDiv, { title: 'Sonnet', utilization: data.sonnet, resetAt: data.sonnet_reset, gaugeMode: mode, strings: strings });
    }
  }

  // Extra usage
  var extraSection = document.getElementById('extra-usage');
  if (extraSection) {
    if (data.extra_enabled) {
      extraSection.textContent = '';
      var lbl = document.createElement('div');
      lbl.className = 'label';
      lbl.textContent = strings.extra_usage || 'Extra Usage';
      extraSection.appendChild(lbl);
      var val = document.createElement('div');
      val.className = 'value-sm';
      var used = data.extra_used != null ? '$' + data.extra_used.toFixed(2) : '\u2014';
      var limit = data.extra_monthly_limit != null ? '$' + data.extra_monthly_limit.toFixed(2) : '\u2014';
      val.textContent = used + ' / ' + limit;
      extraSection.appendChild(val);
      extraSection.style.display = '';
    } else {
      extraSection.style.display = 'none';
    }
  }

  var ts = document.getElementById('last-updated');
  if (ts) {
    ts.textContent = (strings.last_updated || 'Last updated') + ': ' + new Date().toLocaleTimeString();
  }
}

function updateWidget(data, config, i18nStrings) {
  currentData = data;
  currentConfig = config;
  if (i18nStrings) strings = i18nStrings;
  if (!data) return;

  var mode = config.gauge_mode || 'remaining';
  var w = config.widget || {};
  var container = document.getElementById('widget-content');
  if (!container) return;

  container.textContent = '';
  var items = [];

  if (w.show_5h !== false) {
    items.push(renderMiniGauge({ label: '5h', utilization: data.five_hour, gaugeMode: mode }));
  }
  if (w.show_7d !== false) {
    items.push(renderMiniGauge({ label: '7d', utilization: data.seven_day, gaugeMode: mode }));
  }
  if (w.show_opus) {
    items.push(renderMiniGauge({ label: 'Op', utilization: data.opus, gaugeMode: mode }));
  }
  if (w.show_sonnet) {
    items.push(renderMiniGauge({ label: 'So', utilization: data.sonnet, gaugeMode: mode }));
  }
  if (w.show_reset_timer && data.five_hour_reset) {
    var timerSpan = document.createElement('span');
    timerSpan.className = 'mini-gauge-label';
    timerSpan.style.cssText = 'font-size:10px;color:var(--text-muted)';
    timerSpan.textContent = '\u23F1 ' + formatResetTime(data.five_hour_reset);
    items.push(timerSpan);
  }

  for (var i = 0; i < items.length; i++) {
    if (i > 0) {
      var sep = document.createElement('div');
      sep.className = 'widget-separator';
      container.appendChild(sep);
    }
    container.appendChild(items[i]);
  }
}

function updateHoverPanel(data, config, plan, i18nStrings) {
  currentData = data;
  currentConfig = config;
  if (i18nStrings) strings = i18nStrings;
  if (!data) return;

  var mode = config.gauge_mode || 'remaining';

  var planEl = document.getElementById('hover-plan-name');
  if (planEl) planEl.textContent = plan || '';

  var g5h = document.getElementById('hover-gauge-5h');
  if (g5h) {
    renderGauge(g5h, { title: strings.five_hour || '5-Hour Limit', utilization: data.five_hour, resetAt: data.five_hour_reset, gaugeMode: mode, strings: strings });
  }

  var g7d = document.getElementById('hover-gauge-7d');
  if (g7d) {
    renderGauge(g7d, { title: strings.seven_day || '7-Day Limit', utilization: data.seven_day, resetAt: data.seven_day_reset, gaugeMode: mode, strings: strings });
  }

  var models = document.getElementById('hover-models');
  if (models) {
    models.textContent = '';
    if (data.opus > 0) {
      var row = document.createElement('div');
      row.style.cssText = 'display:flex;justify-content:space-between';
      var rl = document.createElement('span');
      rl.className = 'label';
      rl.textContent = 'Opus';
      var rv = document.createElement('span');
      rv.className = 'value-sm';
      rv.textContent = Math.round(data.opus) + '%';
      row.appendChild(rl);
      row.appendChild(rv);
      models.appendChild(row);
    }
    if (data.sonnet > 0) {
      var row2 = document.createElement('div');
      row2.style.cssText = 'display:flex;justify-content:space-between';
      var rl2 = document.createElement('span');
      rl2.className = 'label';
      rl2.textContent = 'Sonnet';
      var rv2 = document.createElement('span');
      rv2.className = 'value-sm';
      rv2.textContent = Math.round(data.sonnet) + '%';
      row2.appendChild(rl2);
      row2.appendChild(rv2);
      models.appendChild(row2);
    }
    if (data.extra_enabled) {
      var row3 = document.createElement('div');
      row3.style.cssText = 'display:flex;justify-content:space-between';
      var rl3 = document.createElement('span');
      rl3.className = 'label';
      rl3.textContent = strings.extra_usage || 'Extra';
      var rv3 = document.createElement('span');
      rv3.className = 'value-sm';
      var eu = data.extra_used != null ? '$' + data.extra_used.toFixed(2) : '\u2014';
      var el = data.extra_monthly_limit != null ? '$' + data.extra_monthly_limit.toFixed(2) : '\u2014';
      rv3.textContent = eu + '/' + el;
      row3.appendChild(rl3);
      row3.appendChild(rv3);
      models.appendChild(row3);
    }
  }

  var ts = document.getElementById('hover-updated');
  if (ts) {
    ts.textContent = (strings.last_updated || 'Last updated') + ': ' + new Date().toLocaleTimeString();
  }
}
