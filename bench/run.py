#!/usr/bin/env python3
"""bench/run.py — handl vs bun / node / python / rust / c / go / bash / ruby / java / erlang / julia / csharp benchmark harness.

Two measurement modes per task:
  - end-to-end: wall clock per invocation, N runs, report min/median/stdev.
    Includes process startup + runtime init + compute + shutdown.
  - heavy-work: single run with a larger input where startup is negligible.
    Reports the compute time in isolation.

Usage:
    python3 run.py                    # run all tasks, 10 e2e runs each
    python3 run.py --runs 5           # quick: 5 e2e runs
    python3 run.py --task fibonacci   # only one task
    python3 run.py --mode e2e         # only end-to-end
    python3 run.py --build            # build hnl (release) + rust/c first
"""

from __future__ import annotations

import argparse
import csv
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path

# --------------------------------------------------------------------------- #
# Paths
# --------------------------------------------------------------------------- #

BENCH_DIR = Path(__file__).resolve().parent
PLATFORM_DIR = BENCH_DIR.parent
RESULTS_DIR = BENCH_DIR / "results"
BIN_DIR = BENCH_DIR / "bin"
TASKS_DIR = BENCH_DIR / "tasks"

# --------------------------------------------------------------------------- #
# Task definitions
# --------------------------------------------------------------------------- #

@dataclass
class Task:
    name: str
    e2e_args: list[str]
    heavy_args: list[str] | None       # None = no heavy-work mode
    handl_template: str                # filename inside the task dir
    handl_uses_template: bool = True   # False = run the file directly (cold-start)


TASKS: list[Task] = [
    Task("fibonacci",  e2e_args=["35"],     heavy_args=["38"],   handl_template="handl.hnl.template"),
    Task("ackermann",  e2e_args=["2", "5"], heavy_args=["3", "3"], handl_template="handl.hnl.template"),
    Task("cold-start", e2e_args=[],         heavy_args=None,      handl_template="handl.hnl", handl_uses_template=False),
]

# --------------------------------------------------------------------------- #
# Language definitions
# --------------------------------------------------------------------------- #

@dataclass
class Language:
    name: str
    # source file inside the task dir (None for handl — uses template)
    source_file: str | None
    # build command template ({file} → source, {binary} → output, {outdir} → class dir, {projdir} → csproj dir). None = no build.
    build_cmd: str | None
    # binary name (inside BIN_DIR) if compiled, else None
    binary_template: str | None  # e.g. "rust_fib" — filled per-task
    # how to run: list of command parts. Placeholders: {binary}, {file}, {args...}
    # For handl, run_cmd is a function of (generated_file_path, args).
    runner: str  # "handl" | "bun" | "node" | "python" | "compiled" | "bash" | "ruby" | "java" | "erlang" | "julia" | "csharp"
    # extra args to pass to hnl (e.g. ["--engine", "cranelift"] for handl-cranelift)
    hnl_extra_args: list[str] = field(default_factory=list)


LANGUAGES: list[Language] = [
    # handl variants — tree-walk (default), cranelift (AOT cached), bytecode
    Language("handl-tw", source_file=None, build_cmd=None, binary_template=None, runner="handl"),
    Language("handl-cl", source_file=None, build_cmd=None, binary_template=None, runner="handl", hnl_extra_args=["--engine", "cranelift"]),
    Language("handl-bc", source_file=None, build_cmd=None, binary_template=None, runner="handl", hnl_extra_args=["--engine", "bytecode"]),
    # interpreted languages
    Language("bun",    source_file="js.js",     build_cmd=None,                       binary_template=None,        runner="bun"),
    Language("node",   source_file="js.js",     build_cmd=None,                       binary_template=None,        runner="node"),
    Language("python", source_file="python.py", build_cmd=None,                       binary_template=None,        runner="python"),
    Language("bash",   source_file="bash.sh",   build_cmd=None,                       binary_template=None,        runner="bash"),
    Language("ruby",   source_file="ruby.rb",   build_cmd=None,                       binary_template=None,        runner="ruby"),
    Language("erlang", source_file="erlang.erl", build_cmd=None,                      binary_template=None,        runner="erlang"),
    Language("julia",  source_file="julia.jl",  build_cmd=None,                       binary_template=None,        runner="julia"),
    # compiled languages
    Language("rust",   source_file="rust.rs",   build_cmd="rustc -O {file} -o {binary}", binary_template="rust_{task}", runner="compiled"),
    Language("c",      source_file="c.c",       build_cmd="gcc -O2 {file} -o {binary}", binary_template="c_{task}",    runner="compiled"),
    Language("go",     source_file="go.go",     build_cmd="go build -o {binary} {file}", binary_template="go_{task}",    runner="compiled"),
    Language("java",   source_file="java.java", build_cmd="javac {file} -d {outdir}",  binary_template=None,        runner="java"),
    Language("csharp", source_file="csharp.csproj", build_cmd="dotnet build {projdir} -c Release -o {outdir} --nologo -v q", binary_template=None, runner="csharp"),
]

# --------------------------------------------------------------------------- #
# HNL binary detection
# --------------------------------------------------------------------------- #

def find_hnl() -> str:
    """Find the hnl binary, preferring release over debug."""
    for candidate in [
        PLATFORM_DIR / "target" / "release" / "hnl",
        PLATFORM_DIR / "target" / "debug" / "hnl",
    ]:
        if candidate.exists():
            return str(candidate)
    # fall back to PATH
    in_path = shutil.which("hnl")
    if in_path:
        return in_path
    return ""


HNL = ""  # set in main()

# --------------------------------------------------------------------------- #
# Build phase
# --------------------------------------------------------------------------- #

def build_compiled(task: Task) -> None:
    """Build rust, c, java, and csharp binaries for the given task into BIN_DIR."""
    BIN_DIR.mkdir(exist_ok=True)
    for lang in LANGUAGES:
        if lang.build_cmd is None:
            continue
        source = TASKS_DIR / task.name / lang.source_file
        binary_name = lang.binary_template.format(task=task.name) if lang.binary_template else ""
        binary = BIN_DIR / binary_name if binary_name else None
        # Java compiles to a class directory; csharp uses its own outdir.
        # Only create outdir for languages that actually use it.
        outdir = BIN_DIR / f"{lang.name}_{task.name}"
        if lang.runner in ("java", "csharp"):
            outdir.mkdir(exist_ok=True)
        # For csharp, the "source" is the .csproj file; projdir is the task dir.
        projdir = str(TASKS_DIR / task.name)
        cmd = lang.build_cmd.format(
            file=str(source),
            binary=str(binary) if binary else "",
            outdir=str(outdir),
            projdir=projdir,
        )
        print(f"  [build] {lang.name}: {cmd}")
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        if result.returncode != 0:
            print(f"  [build] FAILED {lang.name}:\n{result.stderr}", file=sys.stderr)
            sys.exit(1)


def build_hnl_release() -> None:
    """Build hnl in release mode."""
    print("[build] cargo build --release -p handl-host-cli ...")
    result = subprocess.run(
        ["cargo", "build", "--release", "-p", "handl-host-cli"],
        cwd=str(PLATFORM_DIR),
        capture_output=True, text=True,
    )
    if result.returncode != 0:
        print(f"[build] FAILED:\n{result.stderr}", file=sys.stderr)
        sys.exit(1)
    print("[build] hnl release binary ready")


# --------------------------------------------------------------------------- #
# Run helpers
# --------------------------------------------------------------------------- #

def make_run_cmd(lang: Language, task: Task, args: list[str]) -> list[str]:
    """Build the command list for a given language/task/args combination."""
    task_dir = TASKS_DIR / task.name

    if lang.runner == "handl":
        if task.handl_uses_template:
            template = (task_dir / task.handl_template).read_text()
            source = template
            for i, arg in enumerate(args, 1):
                source = source.replace(f"__ARG{i}__", arg)
            gen_file = task_dir / "_generated.hnl"
            gen_file.write_text(source)
            return [HNL] + lang.hnl_extra_args + ["run", str(gen_file)]
        else:
            return [HNL] + lang.hnl_extra_args + ["run", str(task_dir / task.handl_template)]

    if lang.runner == "bun":
        return ["bun", "run", str(task_dir / lang.source_file)] + args

    if lang.runner == "node":
        return ["node", str(task_dir / lang.source_file)] + args

    if lang.runner == "python":
        return ["python3", str(task_dir / lang.source_file)] + args

    if lang.runner == "bash":
        return ["bash", str(task_dir / lang.source_file)] + args

    if lang.runner == "ruby":
        return ["ruby", str(task_dir / lang.source_file)] + args

    if lang.runner == "erlang":
        return ["escript", str(task_dir / lang.source_file)] + args

    if lang.runner == "julia":
        return ["julia", str(task_dir / lang.source_file)] + args

    if lang.runner == "java":
        outdir = BIN_DIR / f"java_{task.name}"
        return ["java", "-cp", str(outdir), "java"] + args

    if lang.runner == "compiled":
        binary = BIN_DIR / lang.binary_template.format(task=task.name)
        return [str(binary)] + args

    if lang.runner == "csharp":
        outdir = BIN_DIR / f"csharp_{task.name}"
        # The assembly name is cs_<task> (set in the .csproj).
        assembly = f"cs_{task.name.replace('-', '')}"
        return ["dotnet", str(outdir / f"{assembly}.dll")] + args

    raise ValueError(f"unknown runner: {lang.runner}")


def time_once(cmd: list[str]) -> tuple[float, str, str]:
    """Run cmd once, return (wall_clock_seconds, stdout, stderr)."""
    t0 = time.perf_counter()
    result = subprocess.run(cmd, capture_output=True, text=True)
    t1 = time.perf_counter()
    return (t1 - t0, result.stdout.strip(), result.stderr.strip())


def verify_output(task: Task, lang: Language, stdout: str, args: list[str]) -> bool:
    """Check that the output looks plausible (non-empty, numeric for fib/ack)."""
    if not stdout:
        return False
    if task.name == "cold-start":
        return "Hello, world!" in stdout
    # fib/ack should produce a single integer
    first_line = stdout.splitlines()[0] if stdout else ""
    return first_line.isdigit()


# --------------------------------------------------------------------------- #
# Results
# --------------------------------------------------------------------------- #

@dataclass
class E2EResult:
    task: str
    language: str
    args: str
    runs: int
    times: list[float]
    output: str
    correct: bool

    @property
    def min(self) -> float:
        return min(self.times) if self.times else 0.0

    @property
    def median(self) -> float:
        return statistics.median(self.times) if self.times else 0.0

    @property
    def stdev(self) -> float:
        return statistics.stdev(self.times) if len(self.times) > 1 else 0.0


@dataclass
class HeavyResult:
    task: str
    language: str
    args: str
    time: float
    output: str
    correct: bool


# --------------------------------------------------------------------------- #
# Main benchmark loop
# --------------------------------------------------------------------------- #

def run_e2e(task: Task, lang: Language, n_runs: int) -> E2EResult:
    cmd = make_run_cmd(lang, task, task.e2e_args)
    times: list[float] = []
    last_output = ""
    for _ in range(n_runs):
        elapsed, stdout, stderr = time_once(cmd)
        if stderr and not stdout:
            last_output = f"ERROR: {stderr}"
            times.append(elapsed)
            continue
        last_output = stdout.splitlines()[0] if stdout else ""
        times.append(elapsed)
    correct = verify_output(task, lang, last_output, task.e2e_args)
    return E2EResult(
        task=task.name,
        language=lang.name,
        args=" ".join(task.e2e_args) if task.e2e_args else "-",
        runs=n_runs,
        times=times,
        output=last_output,
        correct=correct,
    )


def run_heavy(task: Task, lang: Language) -> HeavyResult | None:
    if task.heavy_args is None:
        return None
    cmd = make_run_cmd(lang, task, task.heavy_args)
    elapsed, stdout, stderr = time_once(cmd)
    last_output = stdout.splitlines()[0] if stdout else ""
    if stderr and not stdout:
        last_output = f"ERROR: {stderr}"
    correct = verify_output(task, lang, last_output, task.heavy_args)
    return HeavyResult(
        task=task.name,
        language=lang.name,
        args=" ".join(task.heavy_args),
        time=elapsed,
        output=last_output,
        correct=correct,
    )


def fmt_ms(seconds: float) -> str:
    if seconds < 1.0:
        return f"{seconds * 1000:.1f}ms"
    return f"{seconds:.2f}s"


def print_e2e_table(results: list[E2EResult]) -> None:
    print("\n" + "=" * 78)
    print("END-TO-END (wall clock per invocation, includes startup)")
    print("=" * 78)
    # group by task
    tasks_seen: dict[str, list[E2EResult]] = {}
    for r in results:
        tasks_seen.setdefault(r.task, []).append(r)
    for task_name, group in tasks_seen.items():
        args = group[0].args
        print(f"\n  {task_name}  (args: {args}, {group[0].runs} runs each)")
        print(f"  {'language':<12} {'min':>10} {'median':>10} {'stdev':>10}  {'output':>12}  ok")
        print(f"  {'-'*12} {'-'*10} {'-'*10} {'-'*10}  {'-'*12}  --")
        # sort by median (fastest first)
        for r in sorted(group, key=lambda x: x.median):
            ok = "yes" if r.correct else "NO"
            print(f"  {r.language:<12} {fmt_ms(r.min):>10} {fmt_ms(r.median):>10} {fmt_ms(r.stdev):>10}  {r.output:>12}  {ok}")


def print_heavy_table(results: list[HeavyResult]) -> None:
    if not results:
        return
    print("\n" + "=" * 78)
    print("HEAVY-WORK (single run, large input — startup negligible)")
    print("=" * 78)
    tasks_seen: dict[str, list[HeavyResult]] = {}
    for r in results:
        tasks_seen.setdefault(r.task, []).append(r)
    for task_name, group in tasks_seen.items():
        args = group[0].args
        print(f"\n  {task_name}  (args: {args})")
        print(f"  {'language':<12} {'time':>10}  {'output':>12}  ok")
        print(f"  {'-'*12} {'-'*10}  {'-'*12}  --")
        for r in sorted(group, key=lambda x: x.time):
            ok = "yes" if r.correct else "NO"
            print(f"  {r.language:<12} {fmt_ms(r.time):>10}  {r.output:>12}  {ok}")


def write_csv(e2e: list[E2EResult], heavy: list[HeavyResult]) -> None:
    RESULTS_DIR.mkdir(exist_ok=True)
    # e2e CSV
    with open(RESULTS_DIR / "e2e.csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["task", "language", "args", "runs", "min_s", "median_s", "stdev_s", "output", "correct"])
        for r in e2e:
            w.writerow([r.task, r.language, r.args, r.runs, f"{r.min:.6f}", f"{r.median:.6f}", f"{r.stdev:.6f}", r.output, r.correct])
    # heavy CSV
    if heavy:
        with open(RESULTS_DIR / "heavy.csv", "w", newline="") as f:
            w = csv.writer(f)
            w.writerow(["task", "language", "args", "time_s", "output", "correct"])
            for r in heavy:
                w.writerow([r.task, r.language, r.args, f"{r.time:.6f}", r.output, r.correct])
    print(f"\n[results] CSV written to {RESULTS_DIR}/")


# --------------------------------------------------------------------------- #
# CLI
# --------------------------------------------------------------------------- #

def main() -> None:
    global HNL

    parser = argparse.ArgumentParser(description="handl vs bun/node/python/rust/c/go/bash/ruby/java/erlang/julia/csharp benchmark")
    parser.add_argument("--runs", type=int, default=10, help="number of end-to-end runs per combo (default 10)")
    parser.add_argument("--task", choices=[t.name for t in TASKS], help="run only this task")
    parser.add_argument("--mode", choices=["e2e", "heavy", "both"], default="both", help="measurement mode (default both)")
    parser.add_argument("--build", action="store_true", help="build hnl (release) + rust/c/java/csharp binaries before running")
    args = parser.parse_args()

    # find hnl
    HNL = find_hnl()
    if not HNL:
        print("[error] hnl binary not found. Run: cargo build -p handl-host-cli", file=sys.stderr)
        sys.exit(1)
    build_type = "release" if "release" in HNL else "debug"
    print(f"[info] hnl: {HNL} ({build_type})")

    if args.build:
        if build_type != "release":
            build_hnl_release()
            HNL = find_hnl()
        tasks_to_run = [t for t in TASKS if (args.task is None or t.name == args.task)]
        for task in tasks_to_run:
            print(f"[build] compiling rust/c/java/csharp for {task.name}...")
            build_compiled(task)
    else:
        # still need to build rust/c/java/csharp if not already built
        tasks_to_run = [t for t in TASKS if (args.task is None or t.name == args.task)]
        for task in tasks_to_run:
            for lang in LANGUAGES:
                if lang.build_cmd is None:
                    continue
                if lang.runner == "java":
                    classfile = BIN_DIR / f"java_{task.name}" / "java.class"
                    if not classfile.exists():
                        print(f"[build] {lang.name} class missing for {task.name}, building...")
                        build_compiled(task)
                        break
                elif lang.runner == "csharp":
                    assembly = f"cs_{task.name.replace('-', '')}"
                    dll = BIN_DIR / f"csharp_{task.name}" / f"{assembly}.dll"
                    if not dll.exists():
                        print(f"[build] {lang.name} assembly missing for {task.name}, building...")
                        build_compiled(task)
                        break
                else:
                    binary = BIN_DIR / lang.binary_template.format(task=task.name)
                    if not binary.exists():
                        print(f"[build] {lang.name} binary missing for {task.name}, building...")
                        build_compiled(task)
                        break

    # run benchmarks
    e2e_results: list[E2EResult] = []
    heavy_results: list[HeavyResult] = []

    for task in tasks_to_run:
        print(f"\n[bench] {task.name} — e2e args={task.e2e_args}, heavy args={task.heavy_args}")
        if args.mode in ("e2e", "both"):
            for lang in LANGUAGES:
                print(f"  [e2e] {lang.name} ({args.runs} runs)...", end=" ", flush=True)
                r = run_e2e(task, lang, args.runs)
                e2e_results.append(r)
                status = "ok" if r.correct else "MISMATCH"
                print(f"median={fmt_ms(r.median)} [{status}]")
        if args.mode in ("heavy", "both"):
            for lang in LANGUAGES:
                if task.heavy_args is None:
                    continue
                print(f"  [heavy] {lang.name}...", end=" ", flush=True)
                r = run_heavy(task, lang)
                if r:
                    heavy_results.append(r)
                    status = "ok" if r.correct else "MISMATCH"
                    print(f"time={fmt_ms(r.time)} [{status}]")

    # output
    if e2e_results:
        print_e2e_table(e2e_results)
    if heavy_results:
        print_heavy_table(heavy_results)
    write_csv(e2e_results, heavy_results)


if __name__ == "__main__":
    main()
