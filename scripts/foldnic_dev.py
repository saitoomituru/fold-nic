#!/usr/bin/env python3
"""Fold NICの開発契約、ログ、local Sphere-DOS receiptを管理する。"""

from __future__ import annotations

import argparse
from datetime import datetime
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
from typing import Any


PROFILE_PATH = Path("sphere-dos/profile.json")
COMPONENTS_PATH = Path("workspace/components.json")
STATE_ROOT = Path(".fold-nic/sphere-dos")
SAFE_WORLD_ID = re.compile(r"^[a-z0-9][a-z0-9._-]*$")
REVISION = re.compile(r"^[0-9a-f]{40}$")
LOG_FILENAME = re.compile(r"^\d{8}-\d{4}__[^/]+\.ja\.md$")

REQUIRED_FILES = (
    Path("AGENTS.md"),
    Path("README.md"),
    Path("LICENSE"),
    Path("docs/architecture/repository-boundary.ja.md"),
    Path("docs/development/sphereos-atlantis-dos-pli.ja.md"),
    Path("experiments/AGENTS.md"),
    Path("experiments/TEMPLATE.ja.md"),
    Path("development-log/AGENTS.md"),
    Path("development-log/TEMPLATE.ja.md"),
    PROFILE_PATH,
    COMPONENTS_PATH,
    Path("status/capability-matrix.json"),
    Path("Cargo.toml"),
    Path("crates/fold-core/Cargo.toml"),
    Path("crates/fold-core/src/lib.rs"),
    Path("crates/fold-store/Cargo.toml"),
    Path("crates/fold-store/src/lib.rs"),
    Path("crates/fold-http-policy/Cargo.toml"),
    Path("crates/fold-http-policy/src/lib.rs"),
    Path("crates/fold-gateway/Cargo.toml"),
    Path("crates/fold-gateway/src/lib.rs"),
    Path("crates/fold-gateway/src/main.rs"),
)

LOG_REQUIRED_MARKERS = (
    "`[FACT]`",
    "`[RESULT]`",
    "`[INTERPRETATION]`",
    "`[HYPOTHESIS]`",
    "`[UNKNOWN]`",
)


def repository_root(start: Path | None = None) -> Path:
    current = (start or Path.cwd()).resolve()
    for candidate in (current, *current.parents):
        if (candidate / "AGENTS.md").is_file() and (candidate / PROFILE_PATH).is_file():
            return candidate
    raise ValueError("Fold NIC repository rootを解決できません。")


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"JSON rootはobjectである必要があります: {path}")
    return value


def atomic_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(value, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    temporary.replace(path)


def git_output(root: Path, *arguments: str) -> str | None:
    try:
        completed = subprocess.run(
            ["git", "-C", str(root), *arguments],
            check=True,
            capture_output=True,
            text=True,
        )
    except (FileNotFoundError, subprocess.CalledProcessError):
        return None
    return completed.stdout.strip()


def git_state(root: Path) -> dict[str, str]:
    revision = git_output(root, "rev-parse", "HEAD") or "unknown"
    branch = git_output(root, "branch", "--show-current") or "unknown"
    porcelain = git_output(root, "status", "--short")
    worktree = "unknown" if porcelain is None else ("dirty" if porcelain else "clean")
    return {"revision": revision, "branch": branch, "worktree": worktree}


def validate_profile(root: Path) -> list[str]:
    errors: list[str] = []
    try:
        profile = load_json(root / PROFILE_PATH)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        return [f"Sphere-DOS profileを読めません: {error}"]

    expected = {
        "schema_version": "1.0.0",
        "product": "fold-nic",
        "edition": "prompt-engineering",
        "distribution": "sphere-dos",
        "standalone_runtime_implemented": False,
    }
    for key, value in expected.items():
        if profile.get(key) != value:
            errors.append(f"profile.{key}が不正です: {profile.get(key)!r} != {value!r}")

    interface = profile.get("interface")
    if not isinstance(interface, dict) or interface.get("machine_id") != "prompt-line":
        errors.append("profile.interface.machine_idはprompt-lineである必要があります。")

    worlds = profile.get("world_profiles")
    if not isinstance(worlds, list) or not worlds:
        errors.append("profile.world_profilesには1件以上必要です。")
    else:
        seen: set[str] = set()
        for world in worlds:
            if not isinstance(world, dict):
                errors.append("world profileはobjectである必要があります。")
                continue
            world_id = world.get("id")
            if not isinstance(world_id, str) or not SAFE_WORLD_ID.fullmatch(world_id):
                errors.append(f"安全でないworld profile idです: {world_id!r}")
            elif world_id in seen:
                errors.append(f"world profile idが重複しています: {world_id}")
            else:
                seen.add(world_id)
            for key in ("authority", "fact_scope", "registry_ref", "causality_profile"):
                if not isinstance(world.get(key), str) or not world[key]:
                    errors.append(f"world profileに{key}が必要です: {world_id!r}")
    return errors


def validate_components(root: Path) -> list[str]:
    errors: list[str] = []
    try:
        registry = load_json(root / COMPONENTS_PATH)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        return [f"component registryを読めません: {error}"]

    if registry.get("schema_version") != "1.0.0":
        errors.append("component registry schema_versionは1.0.0である必要があります。")
    policy = registry.get("revision_policy")
    if not isinstance(policy, dict) or policy.get("mode") != "pinned-revision":
        errors.append("componentはpinned-revisionである必要があります。")

    components = registry.get("components")
    if not isinstance(components, list):
        return [*errors, "componentsはarrayである必要があります。"]
    ids: set[str] = set()
    for component in components:
        if not isinstance(component, dict):
            errors.append("componentはobjectである必要があります。")
            continue
        component_id = component.get("id")
        if not isinstance(component_id, str) or not component_id:
            errors.append("component idがありません。")
            continue
        if component_id in ids:
            errors.append(f"component idが重複しています: {component_id}")
        ids.add(component_id)
        revision = component.get("revision")
        if not isinstance(revision, str) or not REVISION.fullmatch(revision):
            errors.append(f"{component_id}のrevisionは40桁SHAである必要があります。")
        repository = component.get("repository")
        if not isinstance(repository, str) or not repository.startswith("https://github.com/"):
            errors.append(f"{component_id}のrepository URLが不正です。")
        if not isinstance(component.get("write_boundary"), str):
            errors.append(f"{component_id}にwrite_boundaryが必要です。")
    for required_id in ("ZeroRoomLab-manifest", "SphereOS-Atlantis"):
        if required_id not in ids:
            errors.append(f"必須componentがありません: {required_id}")
    return errors


def validate_logs(root: Path) -> list[str]:
    errors: list[str] = []
    for directory in (Path("experiments"), Path("development-log")):
        target = root / directory
        for path in sorted(target.glob("*.ja.md")):
            if path.name == "TEMPLATE.ja.md":
                continue
            if not LOG_FILENAME.fullmatch(path.name):
                errors.append(f"ログfilenameが規約外です: {path.relative_to(root)}")
            content = path.read_text(encoding="utf-8")
            if "状態: `[DRAFT]`" not in content:
                errors.append(f"新規ログは[DRAFT]が必要です: {path.relative_to(root)}")
            for marker in LOG_REQUIRED_MARKERS:
                if marker not in content:
                    errors.append(f"{path.relative_to(root)}に{marker}がありません。")
    return errors


def validate_repository(root: Path) -> dict[str, Any]:
    checks: list[dict[str, Any]] = []
    missing = [str(path) for path in REQUIRED_FILES if not (root / path).is_file()]
    checks.append(
        {
            "id": "required-files",
            "status": "pass" if not missing else "fail",
            "detail": "all present" if not missing else f"missing: {', '.join(missing)}",
        }
    )

    license_ok = False
    try:
        license_ok = (root / "LICENSE").read_text(encoding="utf-8").startswith(
            "                    GNU AFFERO GENERAL PUBLIC LICENSE"
        )
    except OSError:
        pass
    checks.append(
        {
            "id": "license",
            "status": "pass" if license_ok else "fail",
            "detail": "AGPL-3.0-or-later text" if license_ok else "AGPL license textを確認できません",
        }
    )

    validators = (
        ("sphere-dos-profile", validate_profile),
        ("component-registry", validate_components),
        ("logs", validate_logs),
    )
    for check_id, validator in validators:
        errors = validator(root)
        checks.append(
            {
                "id": check_id,
                "status": "pass" if not errors else "fail",
                "detail": "valid" if not errors else errors,
            }
        )

    status = "pass" if all(check["status"] == "pass" for check in checks) else "fail"
    return {
        "schema_version": "1.0.0",
        "status": status,
        "checks": checks,
        "network_access_performed": False,
        "mutations_performed": False,
    }


def doctor(root: Path) -> dict[str, Any]:
    validation = validate_repository(root)
    git = git_state(root)
    tools = {
        "git": shutil.which("git") or "unavailable",
        "python": sys.executable,
        "gnunet-arm": shutil.which("gnunet-arm") or "unavailable",
        "gnunet-gns": shutil.which("gnunet-gns") or "unavailable",
    }
    warnings = []
    if tools["gnunet-arm"] == "unavailable" or tools["gnunet-gns"] == "unavailable":
        warnings.append("GNUnet実行物は未検出です。GNS runtimeはNOT_IMPLEMENTEDのままです。")
    return {
        "schema_version": "1.0.0",
        "status": "pass" if validation["status"] == "pass" else "fail",
        "repository_validation": validation["status"],
        "git": git,
        "tools": tools,
        "warnings": warnings,
        "clock_source": "host_system_clock",
        "clock_calibration": "unverified",
        "network_access_performed": False,
        "mutations_performed": False,
        "secret_scan_performed": False,
    }


def select_world(profile: dict[str, Any], world_id: str | None) -> dict[str, Any]:
    selected = world_id or profile["default_world_profile"]
    for world in profile["world_profiles"]:
        if world["id"] == selected:
            return world
    raise ValueError(f"未登録world profileです: {selected}")


def boot_sphere_dos(root: Path, world_id: str | None = None) -> dict[str, Any]:
    validation = validate_repository(root)
    if validation["status"] != "pass":
        raise ValueError("repository validationが失敗したためSphere-DOSをbootしません。")
    profile = load_json(root / PROFILE_PATH)
    world = select_world(profile, world_id)
    observed = datetime.now().astimezone()
    session_id = f"{observed.strftime('%Y%m%dT%H%M%S%f%z')}-{world['id']}"
    git = git_state(root)
    receipt = {
        "schema_version": "1.0.0",
        "session_id": session_id,
        "product": profile["product"],
        "profile_id": profile["profile_id"],
        "edition": profile["edition"],
        "distribution": profile["distribution"],
        "distribution_role": profile["distribution_role"],
        "interface": profile["interface"],
        "runtime_state": "development-control-plane-ready",
        "deployment_scope": "local-development-scaffold-only",
        "standalone_runtime_implemented": False,
        "world_profile": world,
        "git": git,
        "component_registry": str(COMPONENTS_PATH),
        "component_runtimes_started": [],
        "model_invoked": False,
        "network_access_performed": False,
        "authentication_started": False,
        "secret_scan_performed": False,
        "observed_at": observed.isoformat(timespec="seconds"),
        "clock_source": "host_system_clock",
        "clock_calibration": "unverified",
        "unknowns": list(profile.get("unknowns", [])),
    }
    receipt_path = root / STATE_ROOT / "sessions" / session_id / "receipt.json"
    atomic_json(receipt_path, receipt)
    atomic_json(
        root / STATE_ROOT / "current.json",
        {
            "schema_version": "1.0.0",
            "session_id": session_id,
            "runtime_state": receipt["runtime_state"],
            "receipt": str(receipt_path.relative_to(root)),
            "updated_at": receipt["observed_at"],
        },
    )
    return receipt


def sphere_dos_status(root: Path) -> dict[str, Any]:
    profile = load_json(root / PROFILE_PATH)
    current_path = root / STATE_ROOT / "current.json"
    if not current_path.is_file():
        return {
            "schema_version": "1.0.0",
            "distribution": profile["distribution"],
            "runtime_state": "not-booted",
            "standalone_runtime_implemented": False,
            "network_access_performed": False,
            "mutations_performed": False,
        }
    current = load_json(current_path)
    receipt_path = root / current["receipt"]
    try:
        receipt_path.resolve().relative_to(root.resolve())
    except ValueError as error:
        raise ValueError("current receiptがrepository外を指しています。") from error
    receipt = load_json(receipt_path)
    return {
        "schema_version": "1.0.0",
        "distribution": profile["distribution"],
        "runtime_state": receipt["runtime_state"],
        "session_id": receipt["session_id"],
        "receipt": str(receipt_path),
        "world_profile": receipt["world_profile"],
        "standalone_runtime_implemented": False,
        "component_runtimes_started": [],
        "network_access_performed": False,
        "mutations_performed": False,
    }


def safe_title(title: str) -> str:
    value = re.sub(r"[\\/:*?\"<>|\s]+", "_", title.strip())
    value = value.strip("._")
    if not value:
        raise ValueError("題名から安全なfilenameを生成できません。")
    return value[:80]


def new_log(root: Path, kind: str, title: str) -> Path:
    now = datetime.now().astimezone()
    mapping = {
        "experiment": (Path("experiments"), Path("experiments/TEMPLATE.ja.md")),
        "development": (Path("development-log"), Path("development-log/TEMPLATE.ja.md")),
    }
    directory, template_path = mapping[kind]
    git = git_state(root)
    filename = f"{now.strftime('%Y%m%d-%H%M')}__{safe_title(title)}.ja.md"
    destination = root / directory / filename
    if destination.exists():
        raise ValueError(f"同名logがすでにあります: {destination.relative_to(root)}")
    content = (root / template_path).read_text(encoding="utf-8")
    replacements = {
        "{{TITLE}}": title,
        "{{CREATED_AT}}": now.isoformat(timespec="minutes"),
        "{{GIT_REVISION}}": git["revision"],
        "{{WORKTREE_STATE}}": git["worktree"],
    }
    for source, target in replacements.items():
        content = content.replace(source, target)
    destination.write_text(content, encoding="utf-8", newline="\n")
    return destination


def format_human(result: dict[str, Any]) -> str:
    lines = [f"status: {str(result.get('status', result.get('runtime_state', 'unknown'))).upper()}"]
    for check in result.get("checks", []):
        lines.append(f"[{check['status'].upper()}] {check['id']}: {check['detail']}")
    if "session_id" in result:
        lines.append(f"session: {result['session_id']}")
    if result.get("warnings"):
        for warning in result["warnings"]:
            lines.append(f"[WARNING] {warning}")
    lines.append(f"network: {str(result.get('network_access_performed', False)).lower()}")
    return "\n".join(lines)


def emit(result: dict[str, Any], as_json: bool) -> None:
    if as_json:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(format_human(result))


def parser() -> argparse.ArgumentParser:
    root_parser = argparse.ArgumentParser(description="Fold NIC開発制御面")
    subcommands = root_parser.add_subparsers(dest="command", required=True)

    for name in ("validate", "doctor"):
        command = subcommands.add_parser(name)
        command.add_argument("--json", action="store_true")

    log = subcommands.add_parser("log")
    log_commands = log.add_subparsers(dest="log_command", required=True)
    log_new = log_commands.add_parser("new")
    log_new.add_argument("--kind", choices=("experiment", "development"), required=True)
    log_new.add_argument("--title", required=True)
    log_new.add_argument("--json", action="store_true")
    log_validate = log_commands.add_parser("validate")
    log_validate.add_argument("--json", action="store_true")

    sphere_dos = subcommands.add_parser("sphere-dos")
    sphere_commands = sphere_dos.add_subparsers(dest="sphere_command", required=True)
    boot = sphere_commands.add_parser("boot")
    boot.add_argument("--world")
    boot.add_argument("--json", action="store_true")
    status = sphere_commands.add_parser("status")
    status.add_argument("--json", action="store_true")
    return root_parser


def main(arguments: list[str] | None = None) -> int:
    args = parser().parse_args(arguments)
    try:
        root = repository_root()
        if args.command == "validate":
            result = validate_repository(root)
            emit(result, args.json)
            return 0 if result["status"] == "pass" else 1
        if args.command == "doctor":
            result = doctor(root)
            emit(result, args.json)
            return 0 if result["status"] == "pass" else 1
        if args.command == "log" and args.log_command == "validate":
            errors = validate_logs(root)
            result = {
                "schema_version": "1.0.0",
                "status": "pass" if not errors else "fail",
                "errors": errors,
                "network_access_performed": False,
                "mutations_performed": False,
            }
            emit(result, args.json)
            return 0 if not errors else 1
        if args.command == "log" and args.log_command == "new":
            path = new_log(root, args.kind, args.title)
            result = {
                "schema_version": "1.0.0",
                "status": "created",
                "path": str(path.relative_to(root)),
                "state": "DRAFT",
                "network_access_performed": False,
            }
            emit(result, args.json)
            return 0
        if args.command == "sphere-dos" and args.sphere_command == "boot":
            result = boot_sphere_dos(root, args.world)
            emit(result, args.json)
            return 0
        if args.command == "sphere-dos" and args.sphere_command == "status":
            result = sphere_dos_status(root)
            emit(result, args.json)
            return 0
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
