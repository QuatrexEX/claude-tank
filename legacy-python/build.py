"""Claude Tank — PyInstaller build script.

Run: python build.py
Output: dist/claude-tank.exe
"""

import PyInstaller.__main__
import os

here = os.path.dirname(os.path.abspath(__file__))

PyInstaller.__main__.run([
    os.path.join(here, "main.py"),
    "--onefile",
    "--windowed",
    "--name=claude-tank",
    "--icon=assets/icon.ico" if os.path.exists(os.path.join(here, "assets", "icon.ico")) else "",
    f"--add-data={os.path.join(here, 'ui', 'web')};ui/web",
    f"--add-data={os.path.join(here, 'locales')};locales",
    "--hidden-import=pystray._win32",
    "--hidden-import=webview",
    "--hidden-import=clr",
    "--noconfirm",
    "--clean",
])
