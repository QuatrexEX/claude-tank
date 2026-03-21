/* Claude Tank — Hover panel logic */

let hoverTimeout = null;

function notifyHover(entered) {
  if (entered) {
    if (hoverTimeout) { clearTimeout(hoverTimeout); hoverTimeout = null; }
    if (window.pywebview && window.pywebview.api) {
      window.pywebview.api.show_hover();
    }
  } else {
    hoverTimeout = setTimeout(() => {
      if (window.pywebview && window.pywebview.api) {
        window.pywebview.api.hide_hover();
      }
    }, 300);
  }
}

function initWidgetHover() {
  document.body.addEventListener('mouseenter', () => notifyHover(true));
  document.body.addEventListener('mouseleave', () => notifyHover(false));
}

function initHoverPanelHover() {
  document.body.addEventListener('mouseenter', () => {
    if (hoverTimeout) { clearTimeout(hoverTimeout); hoverTimeout = null; }
  });
  document.body.addEventListener('mouseleave', () => notifyHover(false));
}
