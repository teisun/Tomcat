#!/usr/bin/env python3
"""Recompute a finished session's tool traffic under the current tool mechanics.

Why this exists: after changing how `read` / `edit` batch, how read dedup decides
"you already have this", and how `search_files` returns context, the honest
question is "would the same work have cost fewer LLM round-trips?". Re-running the
task against a live model answers a different question every time it runs. A
recorded transcript, on the other hand, holds the exact sequence of reads, edits
and searches that actually happened, so we can replay that sequence against the
new rules and get the same answer every time.

What this can and cannot tell you:

- It CAN measure how much content the old dedup re-delivered that the new
  range-containment dedup would have answered with an "unchanged" stub, and how
  many round-trips the batch shapes could have absorbed.
- It CANNOT predict how the model behaves under the new prompts. It replays the
  old model's decisions. Read the numbers as "what the mechanism makes possible
  on this trace", not as "what will happen next time".

Batching is reported as two bounds because folding turns together requires
knowing whether a later call depended on an earlier call's result, which the
transcript does not record:

- conservative: only folds consecutive turns that read THE SAME file. Those are
  provably one wide read, no dependency question to answer.
- optimistic: folds any run of consecutive read-only turns, capped at the batch
  size the tool description recommends.

Usage:
  python3 scripts/replay-tool-metrics.py <transcript.jsonl> [--json out.json]
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass, field
from typing import Any

# `read` 工具描述里推荐的一批文件数；批量的上限是 10，但超过这个数整批结果就大到
# 会被落盘，省下的往返又会以「再去读磁盘引用」的形式还回去。
BATCH_FILES = 5
READ_ONLY_TOOLS = {"read", "search_files", "list_dir", "web_fetch", "web_search"}
MUTATING_TOOLS = {"edit", "hashline_edit", "write", "delete"}


@dataclass
class ToolCall:
    turn: int
    call_id: str
    name: str
    args: dict[str, Any]


@dataclass
class Coverage:
    """一个文件当前「模型手上已经有的行」。"""

    ranges: list[tuple[int, int]] = field(default_factory=list)

    def covers(self, lo: int, hi: int) -> bool:
        return any(start <= lo and hi <= end for start, end in self.ranges)

    def add(self, lo: int, hi: int) -> None:
        self.ranges.append((lo, hi))

    def clear(self) -> None:
        self.ranges.clear()


def load_turns(path: str) -> tuple[list[list[ToolCall]], dict[str, str], int]:
    """返回 (每个 assistant 轮的工具调用, tool_call_id -> 结果正文, 压缩次数)。"""
    turns: list[list[ToolCall]] = []
    results: dict[str, str] = {}
    compactions = 0
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if not line:
                continue
            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                continue
            kind = entry.get("type")
            if kind == "branch_summary":
                compactions += 1
                continue
            if kind != "message":
                continue
            message = entry.get("message") or {}
            role = message.get("role")
            if role == "assistant":
                calls = message.get("tool_calls") or []
                if not calls:
                    continue
                turn_index = len(turns)
                parsed: list[ToolCall] = []
                for call in calls:
                    fn = call.get("function") or {}
                    raw = fn.get("arguments")
                    if isinstance(raw, str):
                        try:
                            args = json.loads(raw)
                        except json.JSONDecodeError:
                            args = {}
                    elif isinstance(raw, dict):
                        args = raw
                    else:
                        args = {}
                    parsed.append(
                        ToolCall(
                            turn=turn_index,
                            call_id=call.get("id") or "",
                            name=fn.get("name") or "",
                            args=args if isinstance(args, dict) else {},
                        )
                    )
                turns.append(parsed)
            elif role == "tool":
                content = message.get("content")
                if isinstance(content, str):
                    results[message.get("tool_call_id") or ""] = content
    return turns, results, compactions


def read_window(call: ToolCall, result: str) -> tuple[str, int, int, int]:
    """(路径, 起始行, 结束行, 实际交付行数)。用真正读回来的行数定区间。"""
    path = str(call.args.get("path") or "")
    offset = call.args.get("offset")
    start = int(offset) if isinstance(offset, (int, float)) and offset else 1
    delivered = result.count("\n") + 1 if result else 0
    end = start + max(delivered - 1, 0)
    return path, start, end, delivered


def analyze(transcript: str) -> dict[str, Any]:
    turns, results, compactions = load_turns(transcript)

    tools_per_turn = Counter(len(calls) for calls in turns)
    tool_names = Counter(call.name for calls in turns for call in calls)
    total_calls = sum(tool_names.values())

    # ---- 读放大：老规则 vs 新规则（区间包含判定） ----------------------------
    old_delivered = 0
    new_delivered = 0
    new_delivered_pessimistic = 0
    unique_lines = 0
    reads_per_file: Counter[str] = Counter()
    content_reads_per_file_new: Counter[str] = Counter()
    dedup_hits_new = 0

    coverage: dict[str, Coverage] = defaultdict(Coverage)
    coverage_pessimistic: dict[str, Coverage] = defaultdict(Coverage)
    seen_exact: set[tuple[str, Any, Any]] = set()
    union: dict[str, Coverage] = defaultdict(Coverage)

    # 每轮结束后按老机制的落盘/占位符替换清一次 stamp，模拟 F3-a 的悲观边界：
    # 结果被压缩掉之后 stamp 也跟着失效，模型必须重新读。
    compaction_turns = set()
    if compactions and turns:
        step = max(len(turns) // (compactions + 1), 1)
        compaction_turns = {step * (i + 1) for i in range(compactions)}

    for calls in turns:
        for call in calls:
            if call.name in MUTATING_TOOLS:
                target = str(call.args.get("path") or "")
                if target:
                    coverage[target].clear()
                    coverage_pessimistic[target].clear()
                continue
            if call.name != "read":
                continue
            result = results.get(call.call_id, "")
            path, lo, hi, delivered = read_window(call, result)
            if not path:
                continue
            reads_per_file[path] += 1

            key = (path, call.args.get("offset"), call.args.get("limit"))
            old_hit = key in seen_exact
            seen_exact.add(key)
            old_delivered += 0 if old_hit else delivered

            if not union[path].covers(lo, hi):
                # 第一次真正看到这些行
                unique_lines += max(hi - lo + 1, 0)
                union[path].add(lo, hi)

            if coverage[path].covers(lo, hi):
                dedup_hits_new += 1
            else:
                new_delivered += delivered
                content_reads_per_file_new[path] += 1
                coverage[path].add(lo, hi)

            if coverage_pessimistic[path].covers(lo, hi):
                pass
            else:
                new_delivered_pessimistic += delivered
                coverage_pessimistic[path].add(lo, hi)

        if calls and calls[0].turn in compaction_turns:
            for cov in coverage_pessimistic.values():
                cov.clear()

    # ---- 宽读天花板：每个文件的每个版本只读一次 -------------------------------
    # 批量只能合并「相邻的只读轮」，而这条链路上 read 和 edit/bash 是交替的，能合并的
    # 相邻轮很少。真正的大头是窄读：同一个文件被切成几十个 20-60 行的窗口，每个窗口
    # 一次往返。这里算的是另一个极端 —— 一个文件在两次改动之间只读一次（读宽点），
    # 剩下多少次 read。它是提示词那条改动的天花板，不是保证。
    epoch: dict[str, int] = defaultdict(int)
    read_once: set[tuple[str, int]] = set()
    narrow_reads = 0
    windows: list[int] = []
    for calls in turns:
        for call in calls:
            if call.name in MUTATING_TOOLS:
                target = str(call.args.get("path") or "")
                if target:
                    epoch[target] += 1
                continue
            if call.name != "read":
                continue
            path = str(call.args.get("path") or "")
            if not path:
                continue
            result = results.get(call.call_id, "")
            delivered = result.count("\n") + 1 if result else 0
            windows.append(delivered)
            if delivered and delivered < 100:
                narrow_reads += 1
            read_once.add((path, epoch[path]))
    reads_if_wide = len(read_once)
    windows.sort()
    median_window = windows[len(windows) // 2] if windows else 0

    # ---- 批量：保守下界与乐观上界 -------------------------------------------
    conservative_saved = 0
    run_file: str | None = None
    run_len = 0
    for calls in turns:
        single_file = None
        if calls and all(c.name == "read" for c in calls):
            paths = {str(c.args.get("path") or "") for c in calls}
            if len(paths) == 1:
                single_file = paths.pop()
        if single_file and single_file == run_file:
            run_len += 1
        else:
            if run_len > 1:
                conservative_saved += run_len - 1
            run_file = single_file
            run_len = 1 if single_file else 0
    if run_len > 1:
        conservative_saved += run_len - 1

    optimistic_saved = 0
    pending_files: set[str] = set()
    pending_turns = 0

    def flush() -> int:
        if pending_turns <= 1:
            return 0
        batches = max(-(-len(pending_files) // BATCH_FILES), 1)
        return max(pending_turns - batches, 0)

    for calls in turns:
        if calls and all(c.name in READ_ONLY_TOOLS for c in calls):
            pending_turns += 1
            for c in calls:
                path = c.args.get("path") or c.args.get("pattern") or ""
                pending_files.add(str(path))
        else:
            optimistic_saved += flush()
            pending_files, pending_turns = set(), 0
    optimistic_saved += flush()

    round_trips = len(turns)
    return {
        "transcript": transcript,
        "baseline": {
            "round_trips": round_trips,
            "tool_calls": total_calls,
            "tools_per_turn_avg": round(total_calls / round_trips, 2) if round_trips else 0,
            "single_tool_turns": tools_per_turn.get(1, 0),
            "compactions": compactions,
            "read_calls": tool_names.get("read", 0),
            "read_lines_delivered": old_delivered,
            "read_lines_unique": unique_lines,
            "read_amplification": round(old_delivered / unique_lines, 2) if unique_lines else 0,
            "max_reads_one_file": max(reads_per_file.values(), default=0),
            "read_window_lines_median": median_window,
            "reads_under_100_lines": narrow_reads,
            "dispatch_agent_calls": tool_names.get("dispatch_agent", 0),
            "top_read_files": reads_per_file.most_common(5),
        },
        "replayed": {
            "read_lines_delivered": new_delivered,
            "read_amplification": round(new_delivered / unique_lines, 2) if unique_lines else 0,
            "read_lines_delivered_if_stamps_die_with_compaction": new_delivered_pessimistic,
            "read_amplification_pessimistic": (
                round(new_delivered_pessimistic / unique_lines, 2) if unique_lines else 0
            ),
            "dedup_hits": dedup_hits_new,
            "max_content_reads_one_file": max(content_reads_per_file_new.values(), default=0),
            "round_trips_conservative": round_trips - conservative_saved,
            "round_trips_optimistic": round_trips - optimistic_saved,
            "round_trips_saved_conservative": conservative_saved,
            "round_trips_saved_optimistic": optimistic_saved,
            "read_calls_if_one_wide_read_per_file_version": reads_if_wide,
            "round_trips_if_wide_reads_and_batching": round_trips
            - optimistic_saved
            - max(tool_names.get("read", 0) - reads_if_wide, 0),
            "tools_per_turn_avg_optimistic": (
                round(total_calls / (round_trips - optimistic_saved), 2)
                if round_trips - optimistic_saved
                else 0
            ),
        },
        "tools_per_turn_histogram": dict(sorted(tools_per_turn.items())),
        "tool_name_histogram": dict(tool_names.most_common()),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("transcript")
    parser.add_argument("--json", dest="json_out")
    args = parser.parse_args()

    report = analyze(args.transcript)
    base, new = report["baseline"], report["replayed"]

    print(f"transcript: {report['transcript']}")
    print(f"  LLM 往返           {base['round_trips']}")
    print(
        f"    └ 单工具轮        {base['single_tool_turns']}"
        f" ({base['single_tool_turns'] * 100 // max(base['round_trips'], 1)}%)"
    )
    print(f"  工具调用           {base['tool_calls']}  平均每轮 {base['tools_per_turn_avg']}")
    print(f"  压缩次数           {base['compactions']}")
    print(f"  read 调用          {base['read_calls']}  单文件最多 {base['max_reads_one_file']} 次")
    print(f"  dispatch_agent     {base['dispatch_agent_calls']}")
    print()
    print("  指标                        基线        复算后")
    print(
        f"  read 交付行数          {base['read_lines_delivered']:>10}"
        f"  {new['read_lines_delivered']:>10}"
    )
    print(
        f"  read 放大              {base['read_amplification']:>10}x"
        f"  {new['read_amplification']:>9}x"
        f"   (stamp 随压缩失效: {new['read_amplification_pessimistic']}x)"
    )
    print(
        f"  单文件内容读次数       {base['max_reads_one_file']:>10}"
        f"  {new['max_content_reads_one_file']:>10}"
    )
    print(
        f"  LLM 往返               {base['round_trips']:>10}"
        f"  {new['round_trips_conservative']:>10}"
        f"   (乐观: {new['round_trips_optimistic']})"
    )
    print(
        f"  平均每轮工具数         {base['tools_per_turn_avg']:>10}"
        f"  {'-':>10}   (乐观: {new['tools_per_turn_avg_optimistic']})"
    )
    print()
    print(
        f"  窄读（<100 行）        {base['reads_under_100_lines']} / {base['read_calls']}"
        f"   窗口中位 {base['read_window_lines_median']} 行"
    )
    print(
        f"  若每个文件版本只宽读一次: read {base['read_calls']}"
        f" -> {new['read_calls_if_one_wide_read_per_file_version']}"
        f"，往返 {base['round_trips']} -> {new['round_trips_if_wide_reads_and_batching']}"
    )

    if args.json_out:
        with open(args.json_out, "w", encoding="utf-8") as handle:
            json.dump(report, handle, ensure_ascii=False, indent=2)
        print(f"\nJSON: {args.json_out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
