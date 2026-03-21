/* Claude Tank — Theme management */

function applyTheme(themeName) {
  document.body.className = document.body.className
    .replace(/theme-\w+/g, '')
    .trim();
  document.body.classList.add(`theme-${themeName}`);
}

function getCurrentTheme() {
  const match = document.body.className.match(/theme-(\w+)/);
  return match ? match[1] : 'cyber';
}
