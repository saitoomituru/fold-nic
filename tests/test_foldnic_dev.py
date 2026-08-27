from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest


PROJECT_ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location(
    "foldnic_dev",
    PROJECT_ROOT / "scripts/foldnic_dev.py",
)
assert SPEC is not None and SPEC.loader is not None
foldnic_dev = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(foldnic_dev)


def copy_fixture(target: Path) -> None:
    for relative in (
        "AGENTS.md",
        "README.md",
        "LICENSE",
        "Cargo.toml",
        "crates",
        "docs",
        "experiments",
        "development-log",
        "sphere-dos",
        "workspace",
        "status",
    ):
        source = PROJECT_ROOT / relative
        destination = target / relative
        if source.is_dir():
            shutil.copytree(source, destination)
        else:
            shutil.copy2(source, destination)


class FoldNicDevelopmentToolTest(unittest.TestCase):
    def test_repository契約が検証を通る(self) -> None:
        result = foldnic_dev.validate_repository(PROJECT_ROOT)
        self.assertEqual(result["status"], "pass", result)
        self.assertFalse(result["network_access_performed"])

    def test_log生成はDRAFTとunknownを保持する(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            copy_fixture(root)
            path = foldnic_dev.new_log(root, "experiment", "GNS二ノード試験")
            content = path.read_text(encoding="utf-8")
            self.assertIn("状態: `[DRAFT]`", content)
            self.assertIn("`[UNKNOWN]`", content)
            self.assertRegex(path.name, foldnic_dev.LOG_FILENAME)
            self.assertEqual(foldnic_dev.validate_logs(root), [])

    def test_SphereDOSはstandalone_runtimeを偽装しない(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            copy_fixture(root)
            receipt = foldnic_dev.boot_sphere_dos(root)
            self.assertFalse(receipt["standalone_runtime_implemented"])
            self.assertFalse(receipt["network_access_performed"])
            self.assertFalse(receipt["model_invoked"])
            self.assertEqual(receipt["component_runtimes_started"], [])

            status = foldnic_dev.sphere_dos_status(root)
            self.assertEqual(status["session_id"], receipt["session_id"])
            self.assertFalse(status["standalone_runtime_implemented"])

    def test_profileがruntime実装済みを名乗ると拒否する(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            copy_fixture(root)
            profile_path = root / foldnic_dev.PROFILE_PATH
            profile = json.loads(profile_path.read_text(encoding="utf-8"))
            profile["standalone_runtime_implemented"] = True
            profile_path.write_text(
                json.dumps(profile, ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
            )
            errors = foldnic_dev.validate_profile(root)
            self.assertTrue(any("standalone_runtime_implemented" in error for error in errors))

    def test_必須見出しのないlogを拒否する(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            copy_fixture(root)
            path = root / "experiments/20260827-1200__欠損試験.ja.md"
            path.write_text("# 欠損\n\n状態: `[DRAFT]`\n", encoding="utf-8")
            errors = foldnic_dev.validate_logs(root)
            self.assertTrue(any("`[UNKNOWN]`" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
