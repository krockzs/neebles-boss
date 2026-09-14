#!/usr/bin/env python3
import json
import sys
import tarfile
from pathlib import PurePosixPath

REQUIRED_LANGUAGES = {"es_CL", "en_US"}
REQUIRED_PATHS = {
    "languages/manifest.json",
    "launcher/metadata.json",
    "spacer/metadata.json",
}
REQUIRED_PREFIXES = {
    "assets/",
    "config/",
    "launcher/contents/",
    "notifications/",
    "spacer/contents/",
}


def fail(message: str) -> None:
    raise SystemExit(f"CLIENT PAYLOAD VALIDATION ERROR: {message}")


def safe_name(name: str) -> str:
    path = PurePosixPath(name)
    if path.is_absolute() or ".." in path.parts:
        fail(f"unsafe archive path: {name}")
    normalized = str(path)
    if normalized.startswith("./"):
        normalized = normalized[2:]
    return normalized.rstrip("/") if normalized != "." else normalized


def validate_language_contract(read_bytes, names: set[str]) -> int:
    manifest_path = "languages/manifest.json"
    try:
        manifest = json.loads(read_bytes(manifest_path).decode("utf-8"))
    except Exception as error:
        fail(f"invalid {manifest_path}: {error}")

    if not isinstance(manifest, dict):
        fail("language manifest root must be an object")
    if manifest.get("schema") != 1:
        fail(f"language manifest schema must be 1, got {manifest.get('schema')!r}")

    default = manifest.get("default")
    languages = manifest.get("languages")
    if not isinstance(default, str) or not default.strip():
        fail("language manifest must define a non-empty default")
    if not isinstance(languages, list) or not languages:
        fail("language manifest must define at least one language")

    codes: set[str] = set()
    declared_files: set[str] = set()
    reference_keys: set[str] | None = None

    for entry in languages:
        if not isinstance(entry, dict):
            fail("every language entry must be an object")
        code = entry.get("code")
        filename = entry.get("file")
        if not isinstance(code, str) or not code.strip():
            fail("language entry contains an invalid code")
        if not isinstance(filename, str) or not filename.strip():
            fail(f"language {code!r} contains an invalid file")
        file_path = PurePosixPath(filename)
        if file_path.is_absolute() or len(file_path.parts) != 1 or ".." in file_path.parts or file_path.suffix != ".json":
            fail(f"language {code!r} file must be a simple JSON filename")
        if code in codes:
            fail(f"duplicate language code: {code}")
        if filename in declared_files:
            fail(f"duplicate language file: {filename}")
        codes.add(code)
        declared_files.add(filename)

        payload_path = f"languages/{filename}"
        if payload_path not in names:
            fail(f"missing declared language file: {payload_path}")
        try:
            data = json.loads(read_bytes(payload_path).decode("utf-8"))
        except Exception as error:
            fail(f"invalid {payload_path}: {error}")
        if not isinstance(data, dict):
            fail(f"{payload_path} root must be an object")
        if any(not isinstance(key, str) or not key for key in data):
            fail(f"{payload_path} contains an invalid key")
        if any(not isinstance(value, str) for value in data.values()):
            fail(f"{payload_path} values must all be strings")

        keys = set(data)
        if reference_keys is None:
            reference_keys = keys
        elif keys != reference_keys:
            missing = sorted(reference_keys - keys)
            extra = sorted(keys - reference_keys)
            fail(f"language key mismatch in {payload_path}; missing={missing}, extra={extra}")

    if default not in codes:
        fail(f"default language {default!r} is not declared")
    missing_required = sorted(REQUIRED_LANGUAGES - codes)
    if missing_required:
        fail(f"missing required Boss languages: {', '.join(missing_required)}")

    actual_json = {
        PurePosixPath(name).name
        for name in names
        if name.startswith("languages/") and PurePosixPath(name).suffix == ".json" and name != manifest_path
    }
    if actual_json != declared_files:
        orphan = sorted(actual_json - declared_files)
        missing = sorted(declared_files - actual_json)
        fail(f"language payload mismatch; orphan={orphan}, missing={missing}")

    return len(reference_keys or set())


def validate_archive(path: str) -> None:
    with tarfile.open(path, "r:gz") as archive:
        members = archive.getmembers()
        normalized: dict[str, tarfile.TarInfo] = {}
        for member in members:
            name = safe_name(member.name)
            if name in normalized and member.isfile():
                fail(f"duplicate archive member: {name}")
            normalized[name] = member

        names = set(normalized)
        for required in REQUIRED_PATHS:
            if required not in names or not normalized[required].isfile():
                fail(f"missing required payload file: {required}")
        for prefix in REQUIRED_PREFIXES:
            if not any(name.startswith(prefix) for name in names):
                fail(f"missing required payload tree: {prefix}")

        def read_bytes(name: str) -> bytes:
            member = normalized.get(name)
            if member is None or not member.isfile():
                fail(f"missing payload file: {name}")
            fileobj = archive.extractfile(member)
            if fileobj is None:
                fail(f"could not read payload file: {name}")
            return fileobj.read()

        key_count = validate_language_contract(read_bytes, names)

    print("OK: client payload contract valid")
    print("OK: required payload trees present")
    print(f"OK: Boss languages in parity ({key_count} keys)")
    print("CLIENT PAYLOAD: VALID")


def main() -> None:
    if len(sys.argv) != 2:
        fail("usage: validate-client-payload.py <client-data.tar.gz>")
    validate_archive(sys.argv[1])


if __name__ == "__main__":
    main()
