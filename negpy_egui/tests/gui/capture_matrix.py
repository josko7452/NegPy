#!/usr/bin/env python3
"""Capture a review matrix of the Python app in fixed states.

Loads the frame once, then captures the tabs, sub-tabs and scroll positions in
SCENES. Every capture uses a copied profile, so the user's edits are read but
never written.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import sys
from pathlib import Path

SCENES = [
    # The golden set in tests/gui/golden/tall. Keep both names and order in step.
    {"name": "library", "group": "library"},
    {"name": "roll_top", "group": "roll"},
    {"name": "roll_process", "group": "roll", "scroll_to": "process_section"},
    {"name": "frame_geometry", "group": "frame", "tab": "geometry"},
    {"name": "frame_tone", "group": "frame", "tab": "tone"},
    {"name": "frame_color", "group": "frame", "tab": "color"},
    {"name": "frame_finish", "group": "frame", "tab": "finish"},
    {"name": "frame_favorites", "group": "frame", "tab": "favourites"},
    {"name": "frame_history", "group": "frame", "tab": "history"},
    {"name": "metadata_shot", "group": "metadata", "subtab": "shot"},
    {"name": "metadata_process", "group": "metadata", "subtab": "process"},
    {"name": "metadata_presets", "group": "metadata", "subtab": "presets"},
    {"name": "metadata_gear", "group": "metadata", "subtab": "gear"},
    {"name": "export_file", "group": "export", "subtab": "file"},
    {"name": "export_proof", "group": "export", "subtab": "proof"},
    {"name": "export_presets", "group": "export", "subtab": "presets"},
    {"name": "export_alongside", "group": "export", "subtab": "alongside"},
    {"name": "scan", "group": "scan"},
    {"name": "light_table", "group": "library", "light_table": True},
]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--folder", required=True, type=Path)
    parser.add_argument("--frame", required=True)
    parser.add_argument("--profile", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--width", type=int, default=1920)
    parser.add_argument("--height", type=int, default=1600)
    parser.add_argument("--settle", type=float, default=2.0)
    parser.add_argument("--timeout", type=float, default=120.0)
    args = parser.parse_args(argv)

    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    os.environ.setdefault("QT_ENABLE_HIGHDPI_SCALING", "0")
    os.environ["QT_SCALE_FACTOR"] = "1"
    user_dir = Path("/tmp/negpy-gui-matrix")
    os.environ["NEGPY_USER_DIR"] = str(user_dir)
    os.environ.setdefault("XDG_RUNTIME_DIR", "/tmp/negpy-gui-runtime")
    shutil.copytree(args.profile, user_dir, dirs_exist_ok=True)
    Path(os.environ["XDG_RUNTIME_DIR"]).mkdir(parents=True, exist_ok=True)
    sys.path.insert(0, str(Path(__file__).resolve().parents[3]))

    from PyQt6.QtCore import QTimer
    from PyQt6.QtWidgets import QApplication, QScrollArea

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
    args.output_dir.mkdir(parents=True, exist_ok=True)
    state = {"loaded": False, "index": 0, "saved": [], "started": False, "dialog": None}
    dialog_path = args.output_dir / "20_contact_sheet_dialog.png"

    def current_scroll(scene: dict) -> QScrollArea | None:
        panel = window.right_panel
        group = scene["group"]
        if group == "frame":
            page = panel.stack.currentWidget()
        elif group == "metadata":
            page = panel.metadata_sidebar.tabs.stack.currentWidget()
        elif group == "export":
            page = panel.export_sidebar.tabs.stack.currentWidget()
        else:
            page = panel.group_stack.currentWidget()
        return page if isinstance(page, QScrollArea) else None

    def apply_scene(scene: dict) -> None:
        panel = window.right_panel
        if window.light_table_active() and not scene.get("light_table"):
            window.set_light_table(False)
        panel.show_tab_by_key(scene["group"])
        if scene.get("subtab"):
            if scene["group"] == "metadata":
                panel.metadata_sidebar.show_tab(scene["subtab"])
            elif scene["group"] == "export":
                panel.export_sidebar.tabs.show_tab(scene["subtab"])
        elif scene.get("tab"):
            panel.show_tab_by_key(scene["tab"])
        app.processEvents()
        if scene.get("light_table"):
            window.set_light_table(True)
            app.processEvents()
        if scene.get("scroll_to"):
            widget = getattr(panel.controls_panel, scene["scroll_to"])
            panel.scroll_to(widget, centered=True)
        elif (scroll := current_scroll(scene)) is not None:
            bar = scroll.verticalScrollBar()
            bar.setValue(round(bar.maximum() * float(scene.get("scroll", 0.0))))
        app.processEvents()

    def save_scene() -> None:
        scene = SCENES[state["index"]]
        image = window.grab()
        if image.width() != args.width or image.height() != args.height:
            image = image.scaled(args.width, args.height)
        path = args.output_dir / f"{state['index'] + 1:02d}_{scene['name']}.png"
        image.save(str(path))
        state["saved"].append(
            {
                "file": path.name,
                "width": image.width(),
                "height": image.height(),
                "group": scene["group"],
                "tab": scene.get("tab", ""),
                "subtab": scene.get("subtab", ""),
                "scroll": scene.get("scroll", 0.0),
                "view": "light_table" if scene.get("light_table") else "canvas",
            }
        )
        print(f"saved {path.name}")
        state["index"] += 1
        if state["index"] >= len(SCENES):
            open_contact_sheet()
            return
        apply_scene(SCENES[state["index"]])
        QTimer.singleShot(450, save_scene)

    def write_manifest() -> None:
        manifest = {
            "profile": str(args.profile),
            "folder": str(args.folder),
            "frame": args.frame,
            "width": args.width,
            "height": args.height,
            "captures": state["saved"],
        }
        (args.output_dir / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")

    def open_contact_sheet() -> None:
        from negpy.desktop.view.widgets.contact_sheet_dialog import ContactSheetDialog

        def patched(self, *args, **kwargs):
            ContactSheetDialog.__orig_init__(self, *args, **kwargs)
            state["dialog"] = self
            self.show()
            wait_for_dialog_tiles(self)
            return None

        ContactSheetDialog.__orig_init__ = ContactSheetDialog.__init__
        ContactSheetDialog.__init__ = patched
        ContactSheetDialog.exec = lambda self: 0
        controller.request_contact_sheet()

    def wait_for_dialog_tiles(dialog, attempts: int = 40) -> None:
        if attempts <= 0 or getattr(dialog, "_tiles_done", [0])[dialog._variant] >= len(dialog._frames):
            QTimer.singleShot(500, save_dialog)
            return
        QTimer.singleShot(500, lambda: wait_for_dialog_tiles(dialog, attempts - 1))

    def save_dialog() -> None:
        dialog = state["dialog"]
        if dialog is not None:
            image = dialog.grab()
            if image.width() != 1080 or image.height() != 760:
                image = image.scaled(1080, 760)
            image.save(str(dialog_path))
            state["saved"].append(
                {
                    "file": dialog_path.name,
                    "width": image.width(),
                    "height": image.height(),
                    "group": "export",
                    "tab": "",
                    "subtab": "alongside",
                    "scroll": 0.0,
                    "view": "contact_sheet_dialog",
                }
            )
            print(f"saved {dialog_path.name}")
            dialog.close()
        else:
            print("contact sheet dialog did not open", file=sys.stderr)
        write_manifest()
        app.quit()

    def on_update() -> None:
        if controller.state.current_file_path != target:
            return
        state["loaded"] = True
        if not state["started"]:
            state["started"] = True
            QTimer.singleShot(int(args.settle * 1000), start_scenes)

    def start_scenes() -> None:
        if state["index"] != 0:
            return
        apply_scene(SCENES[0])
        QTimer.singleShot(450, save_scene)

    controller.image_updated.connect(on_update)

    def select() -> None:
        if controller.state.current_file_path != target and not controller._select_file_by_path(target):
            return
        window.right_panel.show_tab_by_key("roll")

    controller.batch_finished.connect(lambda *_: QTimer.singleShot(300, select))
    QTimer.singleShot(1000, lambda: controller.open_library_folder(str(args.folder), add_to_session=False))
    QTimer.singleShot(2000, select)
    QTimer.singleShot(int(args.timeout * 1000), lambda: (write_manifest(), app.quit()))
    app.exec()
    if not state["loaded"]:
        print(f"frame not rendered: {target}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
