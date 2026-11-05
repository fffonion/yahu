"""Long-lived local skill inventory worker (line-delimited JSON over stdio).

The Skills page used to pay, on every request, a fresh interpreter start, a full
Hermes runtime import and a complete plugin discovery (seconds of work that is
identical each time). This worker pays that once and then answers from
stat-keyed caches, so repeat requests only walk the tree and diff file stats.

Nothing here is a blind cache: every cached entry is keyed by ``(mtime_ns,
size)`` and the upstream catalog cache is dropped whenever the SKILL.md stat
digest changes, so edits show up on the very next request.
"""

import json
import os
import sys
from pathlib import Path

import yaml

_FRONTMATTER_CACHE = {}
_INDEX_CACHE = {}
_RUNTIME = None
_CATALOG_DIGEST = None


def runtime():
    """Import the installed Hermes runtime once (never starts an agent)."""
    global _RUNTIME
    if _RUNTIME is not None:
        return _RUNTIME
    if os.environ.get("HERMES_AGENT_DIR"):
        sys.path.insert(0, os.environ["HERMES_AGENT_DIR"])
    import importlib.util

    tools_spec = importlib.util.find_spec("tools")
    for location in tools_spec.submodule_search_locations or [] if tools_spec else []:
        runtime_root = Path(location).parent
        if (runtime_root / "hermes_cli/config.py").is_file():
            sys.path.insert(0, str(runtime_root))
            break
    from tools.skills_tool import (
        _EXCLUDED_SKILL_DIRS, _get_disabled_skill_names, _parse_frontmatter,
        _read_skill_text, _skill_catalog, _skill_search_dirs, _truncate_description,
        clear_skills_cache,
    )
    from agent.skill_utils import (
        is_disabled_entry, iter_skill_index_files, skill_matches_apps,
        skill_matches_environment, skill_matches_platform,
    )

    _RUNTIME = {
        "_EXCLUDED_SKILL_DIRS": _EXCLUDED_SKILL_DIRS,
        "_get_disabled_skill_names": _get_disabled_skill_names,
        "_parse_frontmatter": _parse_frontmatter,
        "_read_skill_text": _read_skill_text,
        "_skill_catalog": _skill_catalog,
        "_skill_search_dirs": _skill_search_dirs,
        "_truncate_description": _truncate_description,
        "clear_skills_cache": clear_skills_cache,
        "is_disabled_entry": is_disabled_entry,
        "iter_skill_index_files": iter_skill_index_files,
        "skill_matches_apps": skill_matches_apps,
        "skill_matches_environment": skill_matches_environment,
        "skill_matches_platform": skill_matches_platform,
    }
    return _RUNTIME


def stat_key(path):
    try:
        stat = path.stat()
    except OSError:
        return None
    return (stat.st_mtime_ns, stat.st_size)


def read_frontmatter(path):
    key = stat_key(path)
    if key is None:
        return {}
    cached = _FRONTMATTER_CACHE.get(str(path))
    if cached is not None and cached[0] == key:
        return cached[1]
    with path.open(encoding="utf-8") as source:
        text = source.read(262144).removeprefix("\ufeff")
    value = {}
    lines = text.splitlines()
    if lines and lines[0].strip() == "---":
        for index, line in enumerate(lines[1:], 1):
            if line.strip() == "---":
                parsed = yaml.safe_load("\n".join(lines[1:index]))
                value = parsed if isinstance(parsed, dict) else {}
                break
    _FRONTMATTER_CACHE[str(path)] = (key, value)
    return value


def read_index(path):
    """``(frontmatter, body)`` from the first 4 KiB, reused until the file changes."""
    key = stat_key(path)
    if key is None:
        return None, None
    cached = _INDEX_CACHE.get(str(path))
    if cached is not None and cached[0] == key:
        return cached[1], cached[2]
    rt = runtime()
    frontmatter, body = rt["_parse_frontmatter"](rt["_read_skill_text"](path)[:4000])
    _INDEX_CACHE[str(path)] = (key, frontmatter, body)
    return frontmatter, body


def display_copy(value, fallback):
    return value.strip() if isinstance(value, str) and value.strip() else fallback


def enrich(row, *, installed, enabled, mutable):
    try:
        frontmatter = read_frontmatter(Path(row["path"]))
    except (OSError, UnicodeError, yaml.YAMLError):
        frontmatter = {}
    metadata = frontmatter.get("metadata")
    hermes = metadata.get("hermes") if isinstance(metadata, dict) else None
    hermes = hermes if isinstance(hermes, dict) else {}
    name = row.get("load_name") or row["name"]
    description = row.get("description") or ""
    version = frontmatter.get("version", "")
    return {
        "name": name, "description": description,
        "editorial_name": display_copy(hermes.get("editorial_name"), name),
        "editorial_description": display_copy(hermes.get("editorial_description"), description),
        "version": str(version) if isinstance(version, (str, int, float)) else "",
        "category": row.get("category") or "uncategorized",
        "installed": installed, "enabled": enabled, "mutable": mutable,
    }


def catalog_digest(roots, scan_dirs):
    """Every SKILL.md this answer derives from, as (path, mtime_ns, size).

    Cheaper than a scan and exact: any edit or add/remove changes the tuple, so
    the upstream 30s catalog TTL can never serve a stale row.
    """
    rt = runtime()
    digest = []
    for directory in list(scan_dirs) + [Path(root) for root in roots]:
        if not directory.is_dir():
            continue
        try:
            found = sorted(rt["iter_skill_index_files"](directory, "SKILL.md"))
        except OSError:
            continue
        for path in found:
            if any(part in rt["_EXCLUDED_SKILL_DIRS"] for part in path.parts):
                continue
            key = stat_key(path)
            if key is not None:
                digest.append((str(path), key[0], key[1]))
    return tuple(digest)


def inventory(roots, home):
    global _CATALOG_DIGEST
    rt = runtime()
    scan_dirs, _ = rt["_skill_search_dirs"]()
    digest = catalog_digest(roots, [directory for _tier, directory in scan_dirs])
    if digest != _CATALOG_DIGEST:
        rt["clear_skills_cache"]()
        _CATALOG_DIGEST = digest

    disabled = rt["_get_disabled_skill_names"]()
    # One shared scan applies compatibility, root precedence, ambiguity and
    # directory-name fallback exactly as Hermes skills_list does.
    catalog = rt["_skill_catalog"](skip_disabled=True)
    installed_names = {row["name"] for row in catalog}
    root_skills = (home / "skills").resolve()
    installed = [enrich(
        row, installed=True, enabled=not rt["is_disabled_entry"](row, disabled),
        mutable=Path(row["path"]).resolve().is_relative_to(root_skills),
    ) for row in catalog if row.get("load_name")]

    optional = {}
    for root in roots[1:]:
        root = root.resolve()
        if not root.is_dir():
            continue
        for path in rt["iter_skill_index_files"](root, "SKILL.md"):
            if any(part in rt["_EXCLUDED_SKILL_DIRS"] for part in path.parts) or not path.resolve().is_relative_to(root):
                continue
            try:
                frontmatter, body = read_index(path)
                if frontmatter is None:
                    continue
                if not (rt["skill_matches_platform"](frontmatter) and rt["skill_matches_environment"](frontmatter) and rt["skill_matches_apps"](frontmatter)):
                    continue
                name = frontmatter.get("name", path.parent.name)
                if name in installed_names or name in optional:
                    continue
                description = frontmatter.get("description") or next((line.strip() for line in (body or "").splitlines() if line.strip() and not line.strip().startswith("#")), "")
                category = str(path.parent.relative_to(root).parent)
                optional[name] = enrich({
                    "name": name, "description": rt["_truncate_description"](description),
                    "category": category if category != "." else "uncategorized", "path": path,
                }, installed=False, enabled=False, mutable=False)
            except (OSError, UnicodeError, ValueError, TypeError):
                continue
    order = lambda row: (row["category"], row["name"])
    return {"object": "list", "data": sorted(installed, key=order), "optional": sorted(optional.values(), key=order)}


def respond(payload):
    sys.stdout.write(json.dumps(payload, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def serve():
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
        except ValueError:
            continue
        request_id = request.get("id")
        if request.get("op") == "shutdown":
            respond({"id": request_id, "ok": True, "body": None})
            return
        try:
            body = inventory([Path(root) for root in request["roots"]], Path(request["home"]))
            respond({"id": request_id, "ok": True, "body": body})
        except Exception as exc:  # keep serving: one bad request must not kill the worker
            respond({"id": request_id, "ok": False, "error": f"{type(exc).__name__}: {exc}"})


if __name__ == "__main__":
    serve()
