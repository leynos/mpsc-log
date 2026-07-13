"""Validate and render shared en-GB-oxendict dictionary policy.

The module owns the pure dictionary schema, merge, Oxford suffix expansion,
and deterministic TOML rendering used by the cache refresh adapter.

Examples
--------
Load a dictionary and render the generated configuration::

    dictionary = load_dictionary(path)
    rendered = render_typos_config(dictionary)
"""

from __future__ import annotations

import dataclasses as dc
import json
import pathlib
import tomllib
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

SCHEMA_VERSION = 1
SUFFIX_PAIRS = (
    ("isably", "izably"),
    ("ise", "ize"),
    ("ises", "izes"),
    ("ised", "ized"),
    ("ising", "izing"),
    ("iser", "izer"),
    ("isers", "izers"),
    ("isable", "izable"),
    ("isation", "ization"),
    ("isations", "izations"),
)


@dc.dataclass(frozen=True)
class Dictionary:
    """Curated words and exclusions used to generate a ``typos`` config.

    Attributes
    ----------
    stems, accepted, corrections, ignore_patterns, excluded_files
        Normalized dictionary data used by merging and rendering.
    """

    stems: tuple[str, ...] = ()
    accepted: tuple[str, ...] = ()
    corrections: tuple[tuple[str, str], ...] = ()
    ignore_patterns: tuple[str, ...] = ()
    excluded_files: tuple[str, ...] = ()


def _string_list(table: cabc.Mapping[str, object], key: str) -> tuple[str, ...]:
    """Read and validate a list of strings from a TOML table."""
    value = table.get(key, [])
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        message = f"{key!r} must be a list of strings"
        raise TypeError(message)
    return tuple(sorted(set(value)))


def _table(document: cabc.Mapping[str, object], key: str) -> cabc.Mapping[str, object]:
    """Read and validate a TOML table."""
    value = document.get(key, {})
    if not isinstance(value, dict):
        message = f"{key!r} must be a table"
        raise TypeError(message)
    return typ.cast("cabc.Mapping[str, object]", value)


def _dictionary_from_text(text: str) -> Dictionary:
    """Parse and validate shared dictionary text."""
    document = tomllib.loads(text)
    schema = document.get("schema")
    if schema != SCHEMA_VERSION:
        message = f"unsupported dictionary schema {schema!r}"
        raise ValueError(message)
    oxford = _table(document, "oxford")
    words = _table(document, "words")
    patterns = _table(document, "patterns")
    files = _table(document, "files")
    corrections_table = _table(words, "corrections")
    if not all(
        isinstance(key, str) and isinstance(value, str)
        for key, value in corrections_table.items()
    ):
        message = "word corrections must map strings to strings"
        raise TypeError(message)
    corrections = typ.cast("cabc.Mapping[str, str]", corrections_table)
    return Dictionary(
        stems=_string_list(oxford, "stems"),
        accepted=_string_list(words, "accepted"),
        corrections=tuple(sorted(corrections.items())),
        ignore_patterns=_string_list(patterns, "ignore"),
        excluded_files=_string_list(files, "exclude"),
    )


def load_dictionary(path: pathlib.Path) -> Dictionary:
    """Load a validated shared dictionary from a TOML file.

    Parameters
    ----------
    path
        Dictionary file to parse.

    Returns
    -------
    Dictionary
        Normalized validated dictionary data.

    Raises
    ------
    OSError, TypeError, ValueError, tomllib.TOMLDecodeError
        If the file cannot be read or its dictionary data is invalid.

    Examples
    --------
    Load the shared authority::

        dictionary = load_dictionary(pathlib.Path("shared.toml"))
    """
    return _dictionary_from_text(path.read_text(encoding="utf-8"))


def merge_dictionaries(base: Dictionary, local: Dictionary) -> Dictionary:
    """Merge shared policy with a non-conflicting local overlay.

    Parameters
    ----------
    base, local
        Shared policy and repository-local additions.

    Returns
    -------
    Dictionary
        Deterministically merged policy.

    Raises
    ------
    ValueError
        If both inputs define different corrections for one word.

    Examples
    --------
    Add a repository-local accepted name::

        merge_dictionaries(base, Dictionary(accepted=("LocalName",)))
    """
    corrections = dict(base.corrections)
    for word, correction in local.corrections:
        existing = corrections.get(word)
        if existing is not None and existing != correction:
            message = (
                f"conflicting correction for {word!r}: {existing!r} != {correction!r}"
            )
            raise ValueError(message)
        corrections[word] = correction
    return Dictionary(
        stems=tuple(sorted(set(base.stems) | set(local.stems))),
        accepted=tuple(sorted(set(base.accepted) | set(local.accepted))),
        corrections=tuple(sorted(corrections.items())),
        ignore_patterns=tuple(
            sorted(set(base.ignore_patterns) | set(local.ignore_patterns))
        ),
        excluded_files=tuple(
            sorted(set(base.excluded_files) | set(local.excluded_files))
        ),
    )


def generate_word_mappings(dictionary: Dictionary) -> dict[str, str]:
    """Expand Oxford stems and explicit words into deterministic mappings.

    Parameters
    ----------
    dictionary
        Curated Oxford stems, accepted words, and explicit corrections.

    Returns
    -------
    dict[str, str]
        Sorted spelling-to-correction mappings.

    Raises
    ------
    ValueError
        If generated and explicit entries conflict.

    Examples
    --------
    Expand the Oxford forms for a stem::

        generate_word_mappings(Dictionary(stems=("organ",)))
    """
    mappings = {word: word for word in dictionary.accepted}

    def add(word: str, correction: str) -> None:
        existing = mappings.get(word)
        if existing is not None and existing != correction:
            message = (
                f"conflicting generated correction for {word!r}: "
                f"{existing!r} != {correction!r}"
            )
            raise ValueError(message)
        mappings[word] = correction

    for word, correction in dictionary.corrections:
        add(word, correction)
    for stem in dictionary.stems:
        for plain_british, oxford in SUFFIX_PAIRS:
            add(f"{stem}{plain_british}", f"{stem}{oxford}")
            add(f"{stem}{oxford}", f"{stem}{oxford}")
    return dict(sorted(mappings.items()))


def _toml_string(value: str) -> str:
    """Render a string using TOML-compatible JSON quoting."""
    return json.dumps(value, ensure_ascii=False)


def _render_array(name: str, values: tuple[str, ...]) -> list[str]:
    """Render a deterministic TOML string array."""
    lines = [f"{name} = ["]
    lines.extend(f"    {_toml_string(value)}," for value in values)
    lines.append("]")
    return lines


def render_typos_config(dictionary: Dictionary) -> str:
    """Render a deterministic, parse-checked ``typos.toml`` document.

    Parameters
    ----------
    dictionary
        Merged spelling policy to render.

    Returns
    -------
    str
        Valid TOML ending in one newline.

    Raises
    ------
    ValueError, tomllib.TOMLDecodeError
        If mappings conflict or rendered TOML is invalid.

    Examples
    --------
    Render a minimal spelling policy::

        render_typos_config(Dictionary(stems=("organ",)))
    """
    lines = [
        "# Generated from the shared en-GB-oxendict dictionary.",
        "# Regenerate with scripts/generate_typos_config.py; do not edit by hand.",
        "",
        "[files]",
        *_render_array("extend-exclude", dictionary.excluded_files),
        "",
        "[default]",
        'locale = "en-gb"',
        *_render_array("extend-ignore-re", dictionary.ignore_patterns),
        "",
        "[default.extend-words]",
    ]
    lines.extend(
        f"{_toml_string(word)} = {_toml_string(correction)}"
        for word, correction in generate_word_mappings(dictionary).items()
    )
    rendered = "\n".join(lines) + "\n"
    tomllib.loads(rendered)
    return rendered
