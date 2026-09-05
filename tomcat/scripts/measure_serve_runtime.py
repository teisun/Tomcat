#!/usr/bin/env python3
"""Measure a release ``tomcat serve --stdio`` against a disposable session copy.

The script never opens the source sessions directory for writing: it copies it into a
temporary Tomcat work directory and directs the child process there through
``TOMCAT__STORAGE__WORK_DIR``. It reports command round-trip times in milliseconds.

Example:
    python3 scripts/measure_serve_runtime.py \
      --binary target/release/tomcat \
      --sessions-dir ~/.tomcat/agents/main/sessions \
      --session-id 1787731662723_567f53d9d26fbd27
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import tempfile
import threading
import time
from typing import Any, Callable


RESPONSE_TIMEOUT_SEC = 30


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True, help="release tomcat binary")
    parser.add_argument(
        "--sessions-dir",
        type=Path,
        required=True,
        help="source <work-dir>/agents/<agent-id>/sessions directory to copy",
    )
    parser.add_argument(
        "--session-id",
        help="existing session to open; defaults to the largest main transcript filename",
    )
    parser.add_argument("--agent-id", default="main", help="agent id represented by sessions-dir")
    parser.add_argument(
        "--stub-api-key-env",
        default="FCODEX_OPENAI_API_KEY,DEEPSEEK_API_KEY",
        help=(
            "comma-separated credential variables required by local model preferences; each is "
            "set to a harmless placeholder because this benchmark sends no prompt"
        ),
    )
    return parser.parse_args()


def largest_session_id(sessions_dir: Path) -> str:
    candidates = [
        path
        for path in sessions_dir.glob("*.jsonl")
        if not path.name.endswith((".tool_display.jsonl", ".user_messages.jsonl"))
    ]
    if not candidates:
        raise RuntimeError(f"no main transcripts in {sessions_dir}")
    return max(candidates, key=lambda path: path.stat().st_size).stem


def copy_runtime_preferences(source_sessions: Path, destination_work_dir: Path) -> None:
    """Copy only model preference files needed to open historical sessions.

    ``tomcat serve`` still reads the user's normal config file, but model catalogue entries
    live below the configured work directory. Copying these two small files keeps the staged
    run semantically equivalent without copying unrelated credentials or assets.
    """

    source_work_dir = source_sessions.parent.parent.parent
    for name in ("models.toml", "model-thinking.json"):
        source = source_work_dir / name
        if source.is_file():
            shutil.copy2(source, destination_work_dir / name)


def start_reader(
    stream: Any,
    output: queue.Queue[dict[str, Any] | BaseException | None],
) -> threading.Thread:
    def run() -> None:
        try:
            for line in stream:
                try:
                    payload = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if isinstance(payload, dict):
                    output.put(payload)
        except BaseException as error:  # surface child reader failures to the caller
            output.put(error)
        finally:
            output.put(None)

    thread = threading.Thread(target=run, daemon=True)
    thread.start()
    return thread


def request(
    process: subprocess.Popen[str],
    output: queue.Queue[dict[str, Any] | BaseException | None],
    frame: dict[str, Any],
    matches: Callable[[dict[str, Any]], bool],
) -> tuple[float, dict[str, Any]]:
    assert process.stdin is not None
    started = time.perf_counter()
    process.stdin.write(json.dumps(frame, separators=(",", ":")) + "\n")
    process.stdin.flush()
    deadline = started + RESPONSE_TIMEOUT_SEC
    while True:
        remaining = deadline - time.perf_counter()
        if remaining <= 0:
            raise TimeoutError(f"timed out waiting for {frame}")
        try:
            received = output.get(timeout=remaining)
        except queue.Empty as error:
            raise TimeoutError(f"timed out waiting for {frame}") from error
        if received is None:
            assert process.stderr is not None
            stderr = process.stderr.read().strip()
            detail = f": {stderr}" if stderr else ""
            raise RuntimeError(f"serve exited before returning a response{detail}")
        if isinstance(received, BaseException):
            raise received
        if matches(received):
            return (time.perf_counter() - started) * 1000, received


def command(
    process: subprocess.Popen[str],
    output: queue.Queue[dict[str, Any] | BaseException | None],
    command_type: str,
    payload: dict[str, Any],
) -> tuple[float, dict[str, Any]]:
    request_id = f"measure-{command_type}"
    frame = {"type": command_type, "id": request_id, **payload}
    return request(process, output, frame, lambda item: item.get("id") == request_id)


def run(
    binary: Path,
    sessions_dir: Path,
    session_id: str,
    agent_id: str,
    stub_api_key_env: str,
) -> dict[str, float]:
    if not binary.is_file():
        raise RuntimeError(f"binary does not exist: {binary}")
    if not sessions_dir.is_dir():
        raise RuntimeError(f"sessions directory does not exist: {sessions_dir}")

    with tempfile.TemporaryDirectory(prefix="tomcat-serve-runtime-") as temporary:
        work_dir = Path(temporary) / "work"
        staged_sessions = work_dir / "agents" / agent_id / "sessions"
        staged_sessions.parent.mkdir(parents=True)
        shutil.copytree(sessions_dir, staged_sessions)
        copy_runtime_preferences(sessions_dir, work_dir)

        environment = os.environ.copy()
        environment["TOMCAT__STORAGE__WORK_DIR"] = str(work_dir)
        environment["TOMCAT__AGENT__ID"] = agent_id
        for name in stub_api_key_env.split(","):
            name = name.strip()
            if name:
                environment.setdefault(name, "tomcat-runtime-benchmark-no-network")
        process = subprocess.Popen(
            [str(binary), "serve", "--stdio"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
            env=environment,
        )
        assert process.stdout is not None
        output: queue.Queue[dict[str, Any] | BaseException | None] = queue.Queue()
        reader = start_reader(process.stdout, output)
        measurements: dict[str, float] = {}
        try:
            milliseconds, _ = request(
                process,
                output,
                {
                    "type": "control_request",
                    "requestId": "measure-initialize",
                    "subtype": "initialize",
                    "payload": {},
                },
                lambda item: item.get("type") == "control_response"
                and item.get("requestId") == "measure-initialize",
            )
            measurements["initialize"] = milliseconds

            for command_type, payload in [
                ("list_models", {}),
                ("list_sessions", {}),
                ("switch_session", {"sessionId": session_id}),
                ("get_state", {"sessionId": session_id}),
                (
                    "get_messages",
                    {
                        "sessionId": session_id,
                        "params": {"limit": 128, "attachmentMode": "reference"},
                    },
                ),
            ]:
                milliseconds, response = command(process, output, command_type, payload)
                if response.get("success") is not True:
                    raise RuntimeError(f"{command_type} failed: {response}")
                measurements[command_type] = milliseconds

            # Queue ordinary reads first, then confirm that the control-plane interrupt
            # response does not wait behind them. Their individual latency scales with the
            # selected historical session, so this is a reproducible fast-path measurement
            # without issuing an LLM prompt or touching the copied transcript.
            _, fresh_session = command(process, output, "new_session", {"params": {}})
            interrupt_session_id = (
                fresh_session.get("payload", {}).get("sessionId")
                if isinstance(fresh_session.get("payload"), dict)
                else None
            )
            if not isinstance(interrupt_session_id, str):
                raise RuntimeError(f"new_session did not return a session id: {fresh_session}")
            assert process.stdin is not None
            for index in range(3):
                process.stdin.write(
                    json.dumps(
                        {
                            "type": "get_messages",
                            "id": f"measure-backlog-{index}",
                            "sessionId": session_id,
                            "params": {"limit": 128, "attachmentMode": "reference"},
                        },
                        separators=(",", ":"),
                    )
                    + "\n"
                )
            process.stdin.flush()
            milliseconds, response = command(
                process,
                output,
                "interrupt",
                {"sessionId": interrupt_session_id},
            )
            if response.get("success") is not True:
                raise RuntimeError(f"interrupt failed: {response}")
            measurements["interrupt_behind_3_get_messages"] = milliseconds
            return measurements
        finally:
            if process.stdin is not None:
                process.stdin.close()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.terminate()
                process.wait(timeout=5)
            reader.join(timeout=1)


if __name__ == "__main__":
    arguments = parse_args()
    selected_session = arguments.session_id or largest_session_id(arguments.sessions_dir.expanduser())
    results = run(
        arguments.binary.expanduser().resolve(),
        arguments.sessions_dir.expanduser().resolve(),
        selected_session,
        arguments.agent_id,
        arguments.stub_api_key_env,
    )
    print(f"session_id: {selected_session}")
    for name, milliseconds in results.items():
        print(f"{name}: {milliseconds:.1f} ms")
