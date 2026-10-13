"""Use Hermes' installed catalog, with a separate optional-skill library."""

import json
import os
import sys
from pathlib import Path

import yaml


def read_frontmatter(path):
    with path.open(encoding="utf-8") as source:
        text = source.read(262144).removeprefix("\ufeff")
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return {}
    for index, line in enumerate(lines[1:], 1):
        if line.strip() == "---":
            value = yaml.safe_load("\n".join(lines[1:index]))
            return value if isinstance(value, dict) else {}
    return {}


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


def inventory(roots, home):
    # Import the installed runtime; never start a chat or call the API server.
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
        _read_skill_text, _skill_catalog, _truncate_description,
    )
    from agent.skill_utils import (
        is_disabled_entry, iter_skill_index_files, skill_matches_apps,
        skill_matches_environment, skill_matches_platform,
    )

    disabled = _get_disabled_skill_names()
    # One shared scan applies compatibility, root precedence, ambiguity and
    # directory-name fallback exactly as Hermes skills_list does.
    catalog = _skill_catalog(skip_disabled=True)
    installed_names = {row["name"] for row in catalog}
    installed = [enrich(
        row, installed=True, enabled=not is_disabled_entry(row, disabled),
        mutable=Path(row["path"]).resolve().is_relative_to((home / "skills").resolve()),
    ) for row in catalog if row.get("load_name")]

    optional = {}
    for root in roots[1:]:
        root = root.resolve()
        if not root.is_dir():
            continue
        for path in iter_skill_index_files(root, "SKILL.md"):
            if any(part in _EXCLUDED_SKILL_DIRS for part in path.parts) or not path.resolve().is_relative_to(root):
                continue
            try:
                frontmatter, body = _parse_frontmatter(_read_skill_text(path)[:4000])
                if not (skill_matches_platform(frontmatter) and skill_matches_environment(frontmatter) and skill_matches_apps(frontmatter)):
                    continue
                name = frontmatter.get("name", path.parent.name)
                if name in installed_names or name in optional:
                    continue
                description = frontmatter.get("description") or next((line.strip() for line in body.splitlines() if line.strip() and not line.strip().startswith("#")), "")
                category = str(path.parent.relative_to(root).parent)
                optional[name] = enrich({
                    "name": name, "description": _truncate_description(description),
                    "category": category if category != "." else "uncategorized", "path": path,
                }, installed=False, enabled=False, mutable=False)
            except (OSError, UnicodeError, ValueError, TypeError):
                continue
    order = lambda row: (row["category"], row["name"])
    return {"object": "list", "data": sorted(installed, key=order), "optional": sorted(optional.values(), key=order)}


if __name__ == "__main__":
    print(json.dumps(inventory([Path(root) for root in sys.argv[1:]], Path(os.environ["HERMES_HOME"])), ensure_ascii=False))
