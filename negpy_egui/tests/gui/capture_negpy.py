#!/usr/bin/env python3
"""Capture the Python app in a fixed state for the GUI comparison.

Runs against an isolated user directory, so a capture never reads or writes the
user's edits. The window must show the requested frame before it saves.
"""

from __future__ import annotations

import argparse
import os
import shutil
import sys
import time
from pathlib import Path


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--folder", required=True, type=Path)
    parser.add_argument("--frame", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--width", type=int, default=1920)
    parser.add_argument("--height", type=int, default=1024)
    parser.add_argument("--settle", type=float, default=2.0)
    parser.add_argument("--timeout", type=float, default=60.0)
    parser.add_argument(
        "--profile",
        type=Path,
        help="Copy this user directory, so the capture shows the saved edits and settings.",
    )
    args = parser.parse_args(argv)

    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    os.environ.setdefault("QT_ENABLE_HIGHDPI_SCALING", "0")
    os.environ["QT_SCALE_FACTOR"] = "1"
    user_dir = Path("/tmp/negpy-gui-capture")
    os.environ.setdefault("NEGPY_USER_DIR", str(user_dir))
    os.environ.setdefault("XDG_RUNTIME_DIR", "/tmp/negpy-gui-runtime")
    Path(os.environ["NEGPY_USER_DIR"]).mkdir(parents=True, exist_ok=True)
    Path(os.environ["XDG_RUNTIME_DIR"]).mkdir(parents=True, exist_ok=True)
    if args.profile:
        shutil.copytree(args.profile, user_dir, dirs_exist_ok=True)
    sys.path.insert(0, str(Path(__file__).resolve().parents[3]))

    from PyQt6.QtCore import QTimer
    from PyQt6.QtWidgets import QApplication

    from negpy.desktop.controller import AppController
    from negpy.desktop.main import _AppStyle
    from negpy.desktop.session import DesktopSessionManager
    from negpy.desktop.view.main_window import MainWindow
    from negpy.desktop.view.styles.templates import load_stylesheet, themed_palette
    from negpy.infrastructure.storage.repository import StorageRepository
    from negpy.kernel.system.config import APP_CONFIG
    from negpy.kernel.system.i18n import configure

    app = QApplication(sys.argv[:1])
    app.setStyle(_AppStyle("Fusion"))
    app.setStyleSheet(load_stylesheet())
    app.setPalette(themed_palette(app.palette()))
    repo = StorageRepository(APP_CONFIG.edits_db_path, APP_CONFIG.settings_db_path)
    repo.initialize()
    repo.save_global_setting("tutorial_seen", True)
    repo.save_global_setting("right_panel_group", "roll")
    repo.save_global_setting("histogram_log_scale", True)
    repo.save_global_setting("scan_source", "camera")
    repo.save_global_setting("scan_setup", {"capture": "camera", "light": "white"})
    configure("en")
    controller = AppController(DesktopSessionManager(repo))
    controller.apply_scan_setup("camera", "white")
    MainWindow._maybe_restore_session = lambda self: None
    window = MainWindow(controller)
    window.setFixedSize(args.width, args.height)
    window.show()

    target = str(args.folder / args.frame)
    state = {"settled": None, "saved": False, "loaded": False}

    def save() -> None:
        if state["saved"] or controller.state.current_file_path != target:
            return
        state["saved"] = True
        image = window.grab()
        if image.width() != args.width or image.height() != args.height:
            image = image.scaled(args.width, args.height)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        image.save(str(args.output))
        print(f"saved {args.output} ({image.width()}x{image.height()}) frame {target}")

    def on_update() -> None:
        if controller.state.current_file_path != target:
            return
        state["loaded"] = True
        if state["settled"] is None:
            state["settled"] = time.monotonic()
        elif time.monotonic() - state["settled"] >= args.settle:
            QTimer.singleShot(0, save)

    controller.image_updated.connect(on_update)

    def select() -> None:
        if controller.state.current_file_path != target and not controller._select_file_by_path(target):
            return
        window.right_panel.show_tab_by_key("roll")

    controller.batch_finished.connect(lambda *_: QTimer.singleShot(300, select))
    QTimer.singleShot(1000, lambda: controller.open_library_folder(str(args.folder), add_to_session=False))
    QTimer.singleShot(2000, select)
    QTimer.singleShot(int(args.timeout * 1000), lambda: (save(), app.quit()))
    app.exec()
    if not state["loaded"]:
        print(f"frame not rendered: {target}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
