"""Read the authorized skill roots for Yahu, including optional display copy."""

import json
import os
import sys
from pathlib import Path

import yaml


def yaml_mapping(text):
    value = yaml.safe_load(text)
    return value if isinstance(value, dict) else {}


def read_frontmatter(path):
    with path.open(encoding="utf-8") as source:
        text = source.read(262144).removeprefix("\ufeff")
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return {}
    for index, line in enumerate(lines[1:], 1):
        if line.strip() == "---":
            return yaml_mapping("\n".join(lines[1:index]))
    return {}


def display_copy(value, fallback):
    return value.strip() if isinstance(value, str) and value.strip() else fallback


def inventory(roots, home):
    config_path = home / "config.yaml"
    try:
        config = yaml_mapping(config_path.read_text(encoding="utf-8")) if config_path.is_file() else {}
    except yaml.YAMLError:
        raise ValueError("invalid skills configuration YAML") from None
    skills_config = config.get("skills")
    disabled_values = skills_config.get("disabled", []) if isinstance(skills_config, dict) else []
    disabled = {name for name in disabled_values if isinstance(name, str)} if isinstance(disabled_values, list) else set()
    found = {}
    for root in roots:
        root = root.resolve()
        if not root.is_dir():
            continue
        for directory, dirs, files in os.walk(root, followlinks=False):
            dirs[:] = sorted(name for name in dirs if name != ".archive")
            if "SKILL.md" not in files:
                continue
            dirs[:] = []
            path = Path(directory) / "SKILL.md"
            if not path.resolve().is_relative_to(root):
                continue
            try:
                frontmatter = read_frontmatter(path)
            except (OSError, UnicodeError, yaml.YAMLError):
                continue
            name = frontmatter.get("name")
            if not isinstance(name, str) or not name or name in found:
                continue
            description = frontmatter.get("description")
            description = description if isinstance(description, str) else ""
            metadata = frontmatter.get("metadata")
            hermes = metadata.get("hermes") if isinstance(metadata, dict) else None
            hermes = hermes if isinstance(hermes, dict) else {}
            category = str(Path(directory).relative_to(root).parent)
            version = frontmatter.get("version", "")
            found[name] = {
                "name": name,
                "description": description,
                "editorial_name": display_copy(hermes.get("editorial_name"), name),
                "editorial_description": display_copy(hermes.get("editorial_description"), description),
                "version": str(version) if isinstance(version, (str, int, float)) else "",
                "category": category if category != "." else "uncategorized",
                "enabled": name not in disabled,
            }
    return sorted(found.values(), key=lambda row: (row["category"], row["name"]))


if __name__ == "__main__":
    print(json.dumps({"object": "list", "data": inventory(
        [Path(root) for root in sys.argv[1:]], Path(os.environ["HERMES_HOME"])
    )}, ensure_ascii=False))
