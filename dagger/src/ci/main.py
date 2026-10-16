"""CI pipeline for handl-lang/platform — one definition, runs anywhere.

`dagger call ci` executes the same pipeline on a laptop, a VPS, or any
orchestrator. Mirrors the retired GHA workflows (.github/workflows):

    test      → cargo nextest --workspace + cargo test --doc --workspace
    coverage  → scripts/coverage.sh --summary + --tooling-gate (blocking)
    dogfood   → scripts/dogfood-gate.sh against a sibling handl-lang/cli

Layout inside the container: the workspace mounts at /work/platform and
sibling repos (e.g. cli) mount as /work/<repo>, so scripts/dev-link.sh can
rewrite the `[patch]` block to the checkouts actually present — the rest
of the cross-repo git deps resolve over https (float on `main` policy).

Private deps: every handl-lang sibling repo is private, so cargo needs a
GitHub token. Pass `--token env://HANDL_TOKEN` (or any secret ref). The
token is mounted as a secret and baked into nothing — the `insteadOf`
rewrite runs inside the same exec that needs it.
"""

import asyncio
from typing import Annotated, Optional

import dagger
from dagger import DefaultPath, Ignore, dag, function, object_type

# Repo root as the default source, minus directories that must never be
# uploaded to the engine (target/ alone is ~8.5 GB).
Source = Annotated[
    dagger.Directory,
    DefaultPath("."),
    Ignore(["target", ".git", "coverage", "node_modules",
            "dagger/.venv", "dagger/sdk"]),
]

# Mirrors the env block the GHA workflows carried (debug=0 keeps the
# workspace target small enough to fit modest runners).
CARGO_ENV = {
    "CARGO_INCREMENTAL": "0",
    "CARGO_TERM_COLOR": "always",
    "CARGO_PROFILE_DEV_DEBUG": "0",
    "CARGO_PROFILE_TEST_DEBUG": "0",
    # Cargo must shell out to git so the insteadOf credential rewrite and
    # ssh/config auth both apply to private git deps.
    "CARGO_NET_GIT_FETCH_WITH_CLI": "true",
}

# Rewrites https://github.com/* fetches through the mounted token. Runs
# inside each exec (not a build layer) so the token never persists in an
# image layer or the cache.
_AUTH_GIT_CONFIG = (
    'git config --global url."https://x-access-token:$(cat /run/secrets/hl-token)'
    '@github.com/".insteadOf "https://github.com/" && '
)


@object_type
class Ci:
    # -- container plumbing ------------------------------------------------

    def _base(self) -> dagger.Container:
        """Pinned toolchain image: stable rust + nextest + llvm-cov."""
        return (
            dag.container()
            .from_("rust:slim")  # stable toolchain, same as old rustup step
            .with_exec(["apt-get", "update"])
            .with_exec([
                "apt-get", "install", "-y", "--no-install-recommends",
                "git", "curl", "ca-certificates", "python3",
                "build-essential", "pkg-config", "libssl-dev",
            ])
            .with_exec(["sh", "-c",
                        "curl -LsSf https://get.nexte.st/latest/linux"
                        " | tar zxf - -C /usr/local/cargo/bin"])
            .with_exec(["rustup", "component", "add", "llvm-tools-preview"])
            .with_exec(["sh", "-c",
                        "curl -LsSf https://github.com/taiki-e/cargo-llvm-cov"
                        "/releases/latest/download/"
                        "cargo-llvm-cov-x86_64-unknown-linux-musl.tar.gz"
                        " | tar xzf - -C /usr/local/cargo/bin"])
        )

    def _workspace(
        self,
        source: dagger.Directory,
        token: Optional[dagger.Secret] = None,
        siblings: Optional[dict] = None,
    ) -> dagger.Container:
        """Base image + source at /work/platform + cargo caches + dev-link.

        dev-link.sh rewrites Cargo.toml's patch block to only the sibling
        checkouts present under /work; all other handl-lang git deps then
        resolve from `main` over https (authenticated when token is set).
        """
        ctr = self._base().with_mounted_directory("/work/platform", source)
        for name, directory in (siblings or {}).items():
            ctr = ctr.with_mounted_directory(f"/work/{name}", directory)
        ctr = ctr.with_workdir("/work/platform")
        for key, value in CARGO_ENV.items():
            ctr = ctr.with_env_variable(key, value)
        if token is not None:
            ctr = ctr.with_mounted_secret("/run/secrets/hl-token", token)
        return (
            ctr
            .with_mounted_cache("/usr/local/cargo/registry",
                                dag.cache_volume("hl-cargo-registry"))
            .with_mounted_cache("/usr/local/cargo/git",
                                dag.cache_volume("hl-cargo-git"))
            .with_mounted_cache("/work/platform/target",
                                dag.cache_volume("hl-platform-target"))
            .with_exec(["bash", "scripts/dev-link.sh", "/work/platform"])
        )

    def _exec(self, ctr: dagger.Container, script: str, authed: bool) -> dagger.Container:
        """Run a shell script, prefixed with the git-auth rewrite if needed."""
        if authed:
            script = _AUTH_GIT_CONFIG + script
        return ctr.with_exec(["bash", "-c", script])

    # -- pipeline functions --------------------------------------------------

    @function
    async def doctor(
        self,
        source: Source,
        token: Optional[dagger.Secret] = None,
    ) -> str:
        """Smoke-test the workspace container: toolchain versions + dev-link.

        Cheap end-to-end check of everything except compiling Rust —
        image build, source mount, dev-link patch rewrite, git auth.
        """
        ws = self._workspace(source, token)
        return await self._exec(
            ws,
            "rustc --version && cargo nextest --version"
            " && cargo llvm-cov --version"
            " && { grep -c 'patch\\.' Cargo.toml || true; }",
            token is not None,
        ).stdout()

    @function
    async def test_shard(
        self,
        source: Source,
        shard: int = 1,
        shards: int = 1,
        jobs: int = 2,
        token: Optional[dagger.Secret] = None,
    ) -> str:
        """Run one nextest partition (count:shard/shards) over the workspace.

        -j is deliberately low: the heavy suites (cranelift JIT,
        standalone_bootstrap) are memory-bound — scale out via `shards`,
        not up via `jobs`.
        """
        ws = self._workspace(source, token)
        return await self._exec(
            ws,
            "cargo nextest run --workspace "
            f"-j {jobs} --partition count:{shard}/{shards}",
            token is not None,
        ).stdout()

    @function
    async def doctests(
        self,
        source: Source,
        token: Optional[dagger.Secret] = None,
    ) -> str:
        """Run workspace doctests (nextest does not cover them)."""
        ws = self._workspace(source, token)
        return await self._exec(
            ws, "cargo test --doc --workspace", token is not None
        ).stdout()

    @function
    async def test(
        self,
        source: Source,
        shards: int = 1,
        jobs: int = 2,
        token: Optional[dagger.Secret] = None,
    ) -> str:
        """Run the full test gate: `shards` parallel nextest partitions + doctests."""
        tasks = [
            self.test_shard(source, shard=i + 1, shards=shards,
                            jobs=jobs, token=token)
            for i in range(shards)
        ]
        tasks.append(self.doctests(source, token))
        results = await asyncio.gather(*tasks, return_exceptions=True)
        failures = [r for r in results if isinstance(r, BaseException)]
        if failures:
            raise RuntimeError(
                f"{len(failures)} of {len(results)} test stages failed:\n"
                + "\n".join(str(f) for f in failures)
            )
        return f"{shards} shard(s) + doctests passed"

    @function
    async def coverage_report(
        self,
        source: Source,
        token: Optional[dagger.Secret] = None,
    ) -> dagger.Directory:
        """Run coverage and return the coverage/ directory (html + lcov.info).

        Export locally with: dagger call coverage-report export --path=coverage
        Upload to codecov from the host afterwards if wanted (uploader is
        standalone — no CI vendor lock needed).
        """
        ws = self._workspace(source, token)
        await self._exec(
            ws,
            "./scripts/coverage.sh --summary"
            " && cargo llvm-cov --workspace --lcov"
            " --output-path coverage/lcov.info",
            token is not None,
        ).sync()
        return ws.directory("/work/platform/coverage")

    @function
    async def coverage_gate(
        self,
        source: Source,
        token: Optional[dagger.Secret] = None,
    ) -> str:
        """Blocking gate: 100% line coverage on the tooling plugin crates."""
        ws = self._workspace(source, token)
        await self._exec(
            ws, "./scripts/coverage.sh --tooling-gate", token is not None
        ).sync()
        return "tooling coverage gate passed"

    @function
    async def dogfood(
        self,
        source: Source,
        token: Optional[dagger.Secret] = None,
        cli_source: Optional[dagger.Directory] = None,
        jobs: int = 0,
    ) -> str:
        """Run scripts/dogfood-gate.sh with a sibling handl-lang/cli checkout.

        Locally: `--cli-source ../cli` uses your checkout. On a cloud box
        omit it and pass `--token` — cli@main is fetched via the token.
        `jobs` overrides HNL_DOGFOOD_JOBS (0 = container nproc).
        """
        if cli_source is None:
            if token is None:
                raise ValueError(
                    "dogfood needs cli sources: pass --cli-source ../cli"
                    " locally or --token env://... to fetch cli@main"
                )
            cli_source = (
                dag.git("https://github.com/handl-lang/cli",
                        http_auth_token=token)
                .branch("main")
                .tree(discard_git_dir=True)
            )
        ws = (
            self._workspace(source, token, siblings={"cli": cli_source})
            .with_mounted_cache("/work/cli/target",
                                dag.cache_volume("hl-cli-target"))
            .with_exec(["bash", "scripts/dev-link.sh", "/work/cli"])
        )
        script = "./scripts/dogfood-gate.sh"
        if jobs:
            script = f"HNL_DOGFOOD_JOBS={jobs} {script}"
        return await self._exec(ws, script, token is not None).stdout()

    @function
    async def ci(
        self,
        source: Source,
        shards: int = 1,
        jobs: int = 2,
        token: Optional[dagger.Secret] = None,
        cli_source: Optional[dagger.Directory] = None,
        dogfood_jobs: int = 0,
    ) -> str:
        """The whole gate, concurrently: tests + doctests, coverage gate, dogfood.

        dogfood runs only when a cli source or token is provided.
        """
        named = {
            "test": self.test(source, shards, jobs, token),
            "coverage-gate": self.coverage_gate(source, token),
        }
        if cli_source is not None or token is not None:
            named["dogfood"] = self.dogfood(
                source, token, cli_source, dogfood_jobs
            )
        results = dict(
            zip(named, await asyncio.gather(
                *named.values(), return_exceptions=True))
        )
        failures = {
            name: res for name, res in results.items()
            if isinstance(res, BaseException)
        }
        for name in failures:
            print(f"--- {name} FAILED ---\n{failures[name]}\n")
        if failures:
            raise RuntimeError(f"ci failed: {', '.join(failures)}")
        return "ci passed: " + ", ".join(results)
