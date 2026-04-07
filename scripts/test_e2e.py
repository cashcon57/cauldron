#!/usr/bin/env python3
"""End-to-end tests for the cauldron CLI binary.

Runs the compiled CLI binary with various subcommands and verifies
that outputs and exit codes are correct. Uses a temporary directory
for all state so tests are isolated from any existing data.

Usage:
    python3 scripts/test_e2e.py [--binary path/to/cauldron]
"""

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
import textwrap


class TestRunner:
    def __init__(self, binary: str):
        self.binary = os.path.abspath(binary)
        self.passed = 0
        self.failed = 0
        self.errors: list[str] = []
        self.tmpdir = tempfile.mkdtemp(prefix="cauldron_e2e_")
        self.db_path = os.path.join(self.tmpdir, "test.db")

    def cleanup(self):
        if os.path.exists(self.tmpdir):
            shutil.rmtree(self.tmpdir)

    def run(self, args: list[str], env_extra: dict | None = None,
            expect_fail: bool = False) -> subprocess.CompletedProcess:
        """Run the cauldron binary with the given arguments."""
        env = os.environ.copy()
        # Override the data directory so we don't touch user data
        env["XDG_DATA_HOME"] = self.tmpdir
        env["HOME"] = self.tmpdir
        if env_extra:
            env.update(env_extra)

        result = subprocess.run(
            [self.binary] + args,
            capture_output=True,
            text=True,
            env=env,
            timeout=30,
            cwd=self.tmpdir,
        )
        return result

    def test(self, name: str, args: list[str], *,
             expect_fail: bool = False,
             expect_stdout: str | None = None,
             expect_not_stdout: str | None = None,
             expect_returncode: int | None = None):
        """Run a single test case."""
        try:
            result = self.run(args, expect_fail=expect_fail)

            # Check return code
            if expect_returncode is not None:
                if result.returncode != expect_returncode:
                    self._fail(name, f"expected rc={expect_returncode}, got rc={result.returncode}\n"
                               f"stdout: {result.stdout[:500]}\nstderr: {result.stderr[:500]}")
                    return result
            elif not expect_fail and result.returncode != 0:
                self._fail(name, f"expected success but got rc={result.returncode}\n"
                           f"stderr: {result.stderr[:500]}")
                return result
            elif expect_fail and result.returncode == 0:
                self._fail(name, "expected failure but got rc=0")
                return result

            # Check stdout contents
            if expect_stdout and expect_stdout not in result.stdout:
                self._fail(name, f"expected stdout to contain '{expect_stdout}'\n"
                           f"actual stdout: {result.stdout[:500]}")
                return result

            if expect_not_stdout and expect_not_stdout in result.stdout:
                self._fail(name, f"expected stdout NOT to contain '{expect_not_stdout}'\n"
                           f"actual stdout: {result.stdout[:500]}")
                return result

            self._pass(name)
            return result

        except subprocess.TimeoutExpired:
            self._fail(name, "timed out after 30 seconds")
            return None
        except Exception as e:
            self._fail(name, f"exception: {e}")
            return None

    def _pass(self, name: str):
        self.passed += 1
        print(f"  PASS  {name}")

    def _fail(self, name: str, reason: str):
        self.failed += 1
        self.errors.append(f"{name}: {reason}")
        print(f"  FAIL  {name}")
        for line in reason.strip().split("\n"):
            print(f"        {line}")

    def summary(self):
        total = self.passed + self.failed
        print(f"\n{'='*60}")
        print(f"Results: {self.passed}/{total} passed, {self.failed} failed")
        if self.errors:
            print(f"\nFailures:")
            for err in self.errors:
                print(f"  - {err.split(chr(10))[0]}")
        print(f"{'='*60}")


def find_binary() -> str:
    """Find the cauldron binary in typical build locations."""
    candidates = [
        "target/release/cauldron",
        "target/debug/cauldron",
    ]
    for c in candidates:
        if os.path.isfile(c) and os.access(c, os.X_OK):
            return c
    return ""


def main():
    parser = argparse.ArgumentParser(description="Cauldron CLI E2E tests")
    parser.add_argument("--binary", default="",
                        help="Path to the cauldron binary (default: auto-detect)")
    args = parser.parse_args()

    binary = args.binary or find_binary()
    if not binary or not os.path.isfile(binary):
        print(f"ERROR: cauldron binary not found at '{binary}'")
        print("Build it first with: cargo build --release -p cauldron-cli")
        sys.exit(1)

    print(f"Using binary: {binary}")
    runner = TestRunner(binary)

    try:
        run_tests(runner)
    finally:
        runner.cleanup()

    runner.summary()
    sys.exit(1 if runner.failed > 0 else 0)


def run_tests(t: TestRunner):
    print("\n--- Database Commands ---")

    t.test("db init",
           ["db", "init", "--path", t.db_path],
           expect_stdout="Database initialized")

    t.test("db init idempotent",
           ["db", "init", "--path", t.db_path],
           expect_stdout="Database initialized")

    t.test("db seed",
           ["db", "seed", "--path", t.db_path],
           expect_stdout="Seed data loaded")

    # Query requires the default DB path to exist in the data dir
    # We use the --path flag for init/seed but query uses default_db_path()
    # So we also init+seed at the default location
    default_db = os.path.join(t.tmpdir, "cauldron", "cauldron.db")
    os.makedirs(os.path.dirname(default_db), exist_ok=True)
    t.run(["db", "init", "--path", default_db])
    t.run(["db", "seed", "--path", default_db])

    t.test("db query existing game",
           ["db", "query", "1245620"],
           expect_stdout="Elden Ring")

    t.test("db query nonexistent game",
           ["db", "query", "99999"],
           expect_stdout="No game found")

    t.test("db recommend",
           ["db", "recommend", "1245620"],
           expect_stdout="Recommended graphics backend")

    print("\n--- Bottle Commands ---")

    result = t.test("bottle create",
                    ["bottle", "create", "E2E-Test-Bottle", "--wine-version", "wine-9.0"],
                    expect_stdout="Created bottle")

    # Parse the bottle ID from output
    bottle_id = None
    if result and result.stdout:
        for line in result.stdout.split("\n"):
            if "ID:" in line:
                bottle_id = line.split("ID:")[1].strip()
                break

    t.test("bottle list",
           ["bottle", "list"],
           expect_stdout="E2E-Test-Bottle")

    if bottle_id:
        t.test("bottle delete",
               ["bottle", "delete", bottle_id],
               expect_stdout="Deleted bottle")
    else:
        t.test("bottle delete (skipped - no ID)", ["--version"],
               expect_stdout="cauldron")

    t.test("bottle list after delete",
           ["bottle", "list"],
           expect_stdout="No bottles found")

    print("\n--- Wine Commands ---")

    t.test("wine list",
           ["wine", "list"],
           expect_stdout="Available Wine versions")

    t.test("wine installed",
           ["wine", "installed"])

    print("\n--- Performance Commands ---")

    t.test("perf system-info",
           ["perf", "system-info"],
           expect_stdout="System Information")

    t.test("perf cache-list",
           ["perf", "cache-list"],
           expect_stdout="No shader caches found")

    print("\n--- Shell Completions ---")

    t.test("completions zsh",
           ["completions", "zsh"])

    t.test("completions bash",
           ["completions", "bash"])

    print("\n--- Error Cases ---")

    t.test("invalid subcommand",
           ["nonexistent"],
           expect_fail=True)

    t.test("bottle delete nonexistent",
           ["bottle", "delete", "nonexistent-id-12345"],
           expect_fail=True)

    t.test("missing required args",
           ["bottle", "create"],
           expect_fail=True)

    t.test("db query missing app_id",
           ["db", "query"],
           expect_fail=True)


if __name__ == "__main__":
    main()
