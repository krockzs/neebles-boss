#!/usr/bin/env python3

import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
LANG_DIR = ROOT / "client" / "languages"
MANIFEST_PATH = LANG_DIR / "manifest.json"

REQUIRED_LANGUAGES = {
    "es_CL",
    "en_US",
}

EXPECTED_SCHEMA = 1


def fail(message: str) -> None:
    print(
        f"LANGUAGE VALIDATION ERROR: {message}",
        file=sys.stderr,
    )
    raise SystemExit(1)


def load_json(path: Path):
    try:
        return json.loads(
            path.read_text(
                encoding="utf-8"
            )
        )
    except FileNotFoundError:
        fail(
            f"missing file: {path}"
        )
    except json.JSONDecodeError as error:
        fail(
            f"invalid JSON in {path}: {error}"
        )


if not LANG_DIR.is_dir():
    fail(
        f"language directory does not exist: {LANG_DIR}"
    )

manifest = load_json(
    MANIFEST_PATH
)

if not isinstance(manifest, dict):
    fail(
        "language manifest root must be an object"
    )

if manifest.get("schema") != EXPECTED_SCHEMA:
    fail(
        "language manifest declares schema "
        f"{manifest.get('schema')!r}; "
        f"expected {EXPECTED_SCHEMA}"
    )

default = manifest.get("default")

if not isinstance(default, str) or not default.strip():
    fail(
        "language manifest must declare a non-empty default"
    )

languages = manifest.get("languages")

if not isinstance(languages, list) or not languages:
    fail(
        "language manifest must declare at least one language"
    )

declared_codes = set()
declared_files = set()
language_objects = {}

for index, language in enumerate(
    languages
):
    if not isinstance(language, dict):
        fail(
            f"languages[{index}] must be an object"
        )

    code = language.get("code")
    filename = language.get("file")

    if (
        not isinstance(code, str)
        or not code.strip()
    ):
        fail(
            f"languages[{index}] has an invalid code"
        )

    if (
        not isinstance(filename, str)
        or not filename.strip()
    ):
        fail(
            f"language '{code}' has an invalid file"
        )

    code = code.strip()
    filename = filename.strip()

    if code in declared_codes:
        fail(
            f"duplicate language code: {code}"
        )

    if filename in declared_files:
        fail(
            f"duplicate language file: {filename}"
        )

    file_path = Path(filename)

    if (
        file_path.is_absolute()
        or ".." in file_path.parts
        or len(file_path.parts) != 1
    ):
        fail(
            f"language '{code}' file must be a simple relative filename: "
            f"{filename}"
        )

    if file_path.suffix.lower() != ".json":
        fail(
            f"language '{code}' file must be JSON: {filename}"
        )

    declared_codes.add(
        code
    )

    declared_files.add(
        filename
    )

    data = load_json(
        LANG_DIR / filename
    )

    if not isinstance(data, dict):
        fail(
            f"language '{code}' root must be an object"
        )

    for key, value in data.items():
        if (
            not isinstance(key, str)
            or not key.strip()
        ):
            fail(
                f"language '{code}' contains an invalid key"
            )

        if not isinstance(value, str):
            fail(
                f"language '{code}' key '{key}' must contain a string"
            )

    language_objects[code] = data


if default not in declared_codes:
    fail(
        f"default language '{default}' is not declared"
    )


missing_required = (
    REQUIRED_LANGUAGES
    - declared_codes
)

if missing_required:
    fail(
        "required Boss languages are missing: "
        + ", ".join(
            sorted(
                missing_required
            )
        )
    )


reference_code = default
reference_keys = set(
    language_objects[
        reference_code
    ].keys()
)

for code, data in language_objects.items():
    keys = set(
        data.keys()
    )

    missing = (
        reference_keys
        - keys
    )

    extra = (
        keys
        - reference_keys
    )

    if missing or extra:
        details = []

        if missing:
            details.append(
                "missing: "
                + ", ".join(
                    sorted(
                        missing
                    )
                )
            )

        if extra:
            details.append(
                "extra: "
                + ", ".join(
                    sorted(
                        extra
                    )
                )
            )

        fail(
            f"language '{code}' is not in key parity with "
            f"'{reference_code}' ({'; '.join(details)})"
        )


actual_json_files = {
    path.name
    for path in LANG_DIR.glob(
        "*.json"
    )
    if path.name != "manifest.json"
}

orphaned = (
    actual_json_files
    - declared_files
)

if orphaned:
    fail(
        "undeclared language JSON files exist: "
        + ", ".join(
            sorted(
                orphaned
            )
        )
    )


missing_declared = (
    declared_files
    - actual_json_files
)

if missing_declared:
    fail(
        "declared language JSON files are missing: "
        + ", ".join(
            sorted(
                missing_declared
            )
        )
    )


print(
    "OK: Boss language contract valid"
)

print(
    f"OK: schema {EXPECTED_SCHEMA}"
)

print(
    f"OK: default {default}"
)

print(
    "OK: languages "
    + ", ".join(
        sorted(
            declared_codes
        )
    )
)

print(
    f"OK: {len(reference_keys)} keys in parity"
)

print(
    "BOSS LANGUAGES: VALID"
)
