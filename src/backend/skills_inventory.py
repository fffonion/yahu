"""Inventory installed and optional skills for the Skills page.

This runs once per request, so everything here is on the critical path. It used to
take ~4.5s because the heavy path (`tools.skills_tool._skill_catalog`) pays three
costs that do not depend on the answer:

* importing `tools.skills_tool` drags in `hermes_cli.config`, the CLI and ruamel;
* `skill_matches_environment` -> `_detect_kanban` -> `_profile_has_kanban_toolset`
  runs a full plugin discovery (~2.6s) just to evaluate a tag for the two
  `environments: [kanban]` skills;
* `parse_frontmatter` parses every SKILL.md with ruamel.yaml (~5ms/file).

So this script imports only the light `agent.skill_utils` API, answers the
`environments:` gate from cheap signals, and parses frontmatter with PyYAML
(libyaml). Hermes still resolves root precedence, duplicates and load names
(`resolve_skill_catalog`), so the listing matches what the agent loads.
"""

import json
import os
import sys
from pathlib import Path

import yaml

try:  # libyaml is ~9x faster than PyYAML's pure-Python loader on these files
    from yaml import CSafeLoader as _SafeLoader
except ImportError:  # pragma: no cover - depends on the host's PyYAML build
    from yaml import SafeLoader as _SafeLoader


def yaml_load(text):
    return yaml.load(text, Loader=_SafeLoader)


# Mirrors tools/skills_tool_plugin.py; importing that module means importing
# tools.skills_tool, which is exactly what this script avoids.
MAX_NAME_LENGTH = 64
MAX_DESCRIPTION_LENGTH = 1024


def load_skill_api():
    """Import `agent.skill_utils` once the Hermes runtime root is reachable."""
    import importlib.util

    if importlib.util.find_spec("agent") is None:
        agent_dir = os.environ.get("HERMES_AGENT_DIR", "")
        if agent_dir:
            sys.path.insert(0, agent_dir)
    if importlib.util.find_spec("agent") is None:
        tools_spec = importlib.util.find_spec("tools")
        for location in tools_spec.submodule_search_locations or [] if tools_spec else []:
            runtime_root = Path(location).parent
            if (runtime_root / "hermes_cli/config.py").is_file():
                sys.path.insert(0, str(runtime_root))
                break
    from agent import skill_utils

    return skill_utils


skill_utils = load_skill_api()


def prime_config_cache():
    """Hand skill_utils its own raw-config cache, parsed once with libyaml.

    `get_skill_search_roots`/`get_disabled_skill_names` read config.yaml through
    `_load_raw_config`, which parses it with ruamel (~0.25s for a 95KB profile). We
    parse the same file with the C loader and store it under the exact cache key
    those functions use, so the parse happens once per request instead of twice.
    Any mismatch (upstream key change, unreadable file) falls back to their parser.
    """
    try:
        from agent import skill_utils as su

        path = su.get_config_path()
        key = su._config_cache_key(path)
        if key is None or key in su._RAW_CONFIG_CACHE:
            return
        parsed = yaml_load(path.read_text(encoding="utf-8-sig"))
        if isinstance(parsed, dict):
            su._RAW_CONFIG_CACHE.clear()
            su._RAW_CONFIG_CACHE[key] = parsed
    except Exception:
        pass


def parse_frontmatter(content):
    """`agent.skill_utils.parse_frontmatter` with PyYAML instead of ruamel."""
    content = content.removeprefix("\ufeff")
    if not content.startswith("---"):
        return {}, content
    lines = content.split("\n")
    end = None
    for index, line in enumerate(lines[1:], 1):
        if line.strip() == "---":
            end = index
            break
    if end is None:
        return {}, content
    yaml_content = "\n".join(lines[1:end])
    body = "\n".join(lines[end + 1:])
    frontmatter = {}
    try:
        parsed = yaml_load(yaml_content)
        if isinstance(parsed, dict):
            frontmatter = parsed
    except Exception:
        for line in yaml_content.strip().split("\n"):
            if ":" in line:
                key, value = line.split(":", 1)
                frontmatter[key.strip()] = value.strip()
    return frontmatter, body


_text_cache = {}


def read_skill_text(path, limit=262144):
    """utf-8-sig + errors="replace", as `tools/skills_tool_plugin._read_skill_text`.

    Memoized for one inventory run: the catalog pass reads the head of every file
    and the editorial pass reads the same files again, so each file is opened once.
    """
    key = str(path)
    text = _text_cache.get(key)
    if text is None:
        text = path.read_text(encoding="utf-8-sig", errors="replace")[:262144]
        _text_cache[key] = text
    return text[:limit]


def truncate_description(description):
    if len(description) <= MAX_DESCRIPTION_LENGTH:
        return description
    return description[: MAX_DESCRIPTION_LENGTH - 3] + "..."


_config_cache = {}


def raw_config(home):
    """The profile config via PyYAML; only read for cheap environment checks."""
    key = str(home)
    if key not in _config_cache:
        _config_cache[key] = dict(skill_utils._load_raw_config())
    return _config_cache[key]


def environment_active(tag, home):
    """Cheap `environments:` gate.

    docker/s6 are exact copies of Hermes' detectors. `kanban` is the expensive one:
    Hermes answers it through `_profile_has_kanban_toolset`, which can trigger a full
    plugin discovery (~2.6s). We take only the free signals (dispatcher env vars and an
    explicit config opt-in) and fail open otherwise, like `_detect_environment` does for
    unknown tags. Failing open can only over-list a skill in this manager UI; it never
    hides one the agent would load.
    """
    tag = str(tag).strip().lower()
    if tag == "docker":
        return os.path.exists("/.dockerenv") or os.path.exists("/run/.containerenv")
    if tag == "s6":
        return os.path.isdir("/run/s6") or os.path.isdir("/package/admin/s6-overlay")
    if tag == "kanban":
        if os.getenv("HERMES_KANBAN_TASK") or os.getenv("HERMES_KANBAN_BOARD"):
            return True
        config = raw_config(home)
        if "kanban" in (config.get("toolsets") or []):
            return True
        for tools in (config.get("platform_toolsets") or {}).values():
            if isinstance(tools, list) and "kanban" in tools:
                return True
    return True


def matches_environment(frontmatter, home):
    environments = frontmatter.get("environments")
    if not environments:
        return True
    tags = [str(env).lower().strip()
            for env in (environments if isinstance(environments, list) else [environments])]
    return any(environment_active(tag, home) for tag in tags if tag)


def visible(frontmatter, home):
    return (skill_utils.skill_matches_platform(frontmatter)
            and matches_environment(frontmatter, home)
            and skill_utils.skill_matches_apps(frontmatter))


def display_copy(value, fallback):
    return value.strip() if isinstance(value, str) and value.strip() else fallback


def enrich(row, *, installed, enabled, mutable):
    """Add local editorial copy: `metadata.hermes.editorial_name/description` + version."""
    try:
        frontmatter, _body = parse_frontmatter(read_skill_text(Path(row["path"])))
    except (OSError, UnicodeError):
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


def category_from_path(skill_path, skill_root):
    try:
        parts = skill_path.relative_to(skill_root).parts
    except ValueError:
        return None
    return parts[0] if len(parts) >= 3 else None


def scan_installed(home):
    """Visible, enabled skills resolved by Hermes' own catalog rules."""
    roots = skill_utils.get_skill_search_roots(home / "skills")
    disabled = skill_utils.get_disabled_skill_names()
    scanned = []
    for tier, scan_dir in roots:
        if tier == skill_utils.TIER_PROJECT:
            iterator = skill_utils.iter_project_skill_files(scan_dir)
        else:
            iterator = skill_utils.iter_skill_index_files(scan_dir, "SKILL.md")
        for skill_md in iterator:
            if any(part in skill_utils.EXCLUDED_SKILL_DIRS for part in skill_md.parts):
                continue
            try:
                frontmatter, body = parse_frontmatter(read_skill_text(skill_md, 4000))
            except (OSError, UnicodeError):
                continue
            description = frontmatter.get("description", "")
            if not description:
                description = next((line for line in map(str.strip, body.strip().split("\n"))
                                    if line and not line.startswith("#")), description)
            scanned.append({
                "name": frontmatter.get("name", skill_md.parent.name)[:MAX_NAME_LENGTH],
                "description": truncate_description(description),
                "category": category_from_path(skill_md, scan_dir),
                "tier": tier, "root": scan_dir, "path": skill_md,
                "visible": bool(visible(frontmatter, home)),
            })
    # Disabled skills stay listed (the UI shows them off); only visibility filters here.
    return [row for row in skill_utils.resolve_skill_catalog(scanned) if row.pop("visible")], disabled


def scan_optional(home, optional_roots, installed_names):
    optional = {}
    for root in optional_roots:
        root = root.resolve()
        if not root.is_dir():
            continue
        for path in skill_utils.iter_skill_index_files(root, "SKILL.md"):
            if any(part in skill_utils.EXCLUDED_SKILL_DIRS for part in path.parts):
                continue
            try:
                if not path.resolve().is_relative_to(root):
                    continue
                frontmatter, body = parse_frontmatter(read_skill_text(path, 4000))
                if not visible(frontmatter, home):
                    continue
                name = frontmatter.get("name", path.parent.name)
                if name in installed_names or name in optional:
                    continue
                description = frontmatter.get("description") or next(
                    (line.strip() for line in body.splitlines()
                     if line.strip() and not line.strip().startswith("#")), "")
                category = str(path.parent.relative_to(root).parent)
                optional[name] = enrich({
                    "name": name, "description": truncate_description(description),
                    "category": category if category != "." else "uncategorized", "path": path,
                }, installed=False, enabled=False, mutable=False)
            except (OSError, UnicodeError, ValueError, TypeError):
                continue
    return optional


def inventory(roots, home):
    """`roots` = [profile skills dir, ...optional library roots]."""
    prime_config_cache()
    catalog, disabled = scan_installed(home)
    installed_names = {row["name"] for row in catalog}
    profile_skills = (home / "skills").resolve()
    installed = [enrich(
        row, installed=True, enabled=not skill_utils.is_disabled_entry(row, disabled),
        mutable=Path(row["path"]).resolve().is_relative_to(profile_skills),
    ) for row in catalog if row.get("load_name")]
    optional = scan_optional(home, roots[1:], installed_names)
    order = lambda row: (row["category"], row["name"])
    return {"object": "list", "data": sorted(installed, key=order),
            "optional": sorted(optional.values(), key=order)}


if __name__ == "__main__":
    print(json.dumps(
        inventory([Path(root) for root in sys.argv[1:]], Path(os.environ["HERMES_HOME"])),
        ensure_ascii=False))
