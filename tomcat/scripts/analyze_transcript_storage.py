#!/usr/bin/env python3
"""Report byte ownership in main Tomcat transcript JSONL files.

This is an operational, read-only investigation tool. It deliberately ignores derived
``*.tool_display.jsonl`` and ``*.user_messages.jsonl`` sidecars so its report answers:
"after rich-render migration, what still occupies the main transcripts?"

Example:
    python3 scripts/analyze_transcript_storage.py \
        --sessions-dir ~/.tomcat/agents/main/sessions
"""

from __future__ import annotations

import argparse
import collections
import json
from pathlib import Path
from typing import Any, Iterable


UNKNOWN = "(unknown)"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--sessions-dir",
        type=Path,
        default=Path.home() / ".tomcat" / "agents" / "main" / "sessions",
        help="directory containing transcript JSONL files",
    )
    parser.add_argument(
        "--top",
        type=int,
        default=30,
        help="maximum rows to show per breakdown (default: 30)",
    )
    return parser.parse_args()


def main_transcript_paths(sessions_dir: Path) -> Iterable[Path]:
    for path in sorted(sessions_dir.glob("*.jsonl")):
        if path.name.endswith((".tool_display.jsonl", ".user_messages.jsonl")):
            continue
        yield path


def message_tool_names(message: dict[str, Any]) -> dict[str, str]:
    names: dict[str, str] = {}
    calls = message.get("tool_calls")
    if not isinstance(calls, list):
        return names
    for call in calls:
        if not isinstance(call, dict):
            continue
        call_id = call.get("id")
        function = call.get("function")
        name = function.get("name") if isinstance(function, dict) else call.get("name")
        if isinstance(call_id, str) and isinstance(name, str):
            names[call_id] = name
    return names


def classify_message(
    message: dict[str, Any],
    tool_names: dict[str, str],
) -> tuple[str, str, str]:
    role = message.get("role")
    kind = message.get("kind", "normal")
    tool = "-"
    if role == "tool":
        tool_call_id = message.get("tool_call_id")
        tool = tool_names.get(tool_call_id, UNKNOWN) if isinstance(tool_call_id, str) else UNKNOWN
    return (
        role if isinstance(role, str) else UNKNOWN,
        kind if isinstance(kind, str) else UNKNOWN,
        tool,
    )


def print_breakdown(
    title: str,
    counts: collections.Counter[tuple[str, ...]],
    total_bytes: int,
    top: int,
) -> None:
    print(f"\n{title}")
    print("bytes       share     category")
    for labels, size in counts.most_common(top):
        rows = " × ".join(labels)
        share = 0 if total_bytes == 0 else size / total_bytes * 100
        print(f"{size:>10,}  {share:>6.2f}%       {rows}")


def run(sessions_dir: Path, top: int) -> int:
    if not sessions_dir.is_dir():
        raise SystemExit(f"sessions directory does not exist: {sessions_dir}")
    if top < 1:
        raise SystemExit("--top must be at least 1")

    by_entry_type: collections.Counter[tuple[str, ...]] = collections.Counter()
    by_message: collections.Counter[tuple[str, ...]] = collections.Counter()
    by_file: collections.Counter[tuple[str, ...]] = collections.Counter()
    malformed_bytes = 0
    total_bytes = 0
    files = 0

    for path in main_transcript_paths(sessions_dir):
        files += 1
        file_bytes = 0
        tool_names: dict[str, str] = {}
        with path.open("rb") as source:
            for line_number, raw_line in enumerate(source, start=1):
                size = len(raw_line)
                total_bytes += size
                file_bytes += size
                try:
                    row = json.loads(raw_line)
                except (UnicodeDecodeError, json.JSONDecodeError):
                    malformed_bytes += size
                    by_entry_type[("malformed",)] += size
                    continue
                if not isinstance(row, dict):
                    by_entry_type[("non_object",)] += size
                    continue
                entry_type = row.get("type", UNKNOWN)
                entry_type = entry_type if isinstance(entry_type, str) else UNKNOWN
                by_entry_type[(entry_type,)] += size
                message = row.get("message")
                if not isinstance(message, dict):
                    continue
                tool_names.update(message_tool_names(message))
                by_message[classify_message(message, tool_names)] += size
        by_file[(path.name,)] += file_bytes

    print(f"sessions_dir: {sessions_dir}")
    print(f"main transcripts: {files}")
    print(f"main transcript bytes: {total_bytes:,}")
    print(f"malformed JSONL bytes: {malformed_bytes:,}")
    print_breakdown("By transcript entry type", by_entry_type, total_bytes, top)
    print_breakdown(
        "By message role × kind × originating tool",
        by_message,
        total_bytes,
        top,
    )
    print_breakdown("Largest main transcripts", by_file, total_bytes, top)
    return 0


if __name__ == "__main__":
    arguments = parse_args()
    raise SystemExit(run(arguments.sessions_dir.expanduser(), arguments.top))
