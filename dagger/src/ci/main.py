"""CI pipeline for handl-lang/platform — thin orchestration over handl-ci.

All container plumbing (toolchain image, workspace mount, dev-link +
self-patch, cargo/sccache caches, token auth) lives in the shared
handl-ci module (github.com/handl-lang/ci — see dagger.json deps).
This module only adds platform-specific gates:

    dogfood — scripts/dogfood-gate.sh against a sibling handl-lang/cli
    ci      — the whole gate: tests + coverage gate + dogfood, concurrently

Everything else (test/test-shard/doctests/coverage-*/doctor) delegates
straight through to handl-ci so the interface stays stable for
scripts/ci-watch.sh.
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

REPO = "platform"


@object_type
class Ci:
    # -- delegates to handl-ci ----------------------------------------------

    @function
    async def doctor(
        self,
        source: Source,
        token: Optional[dagger.Secret] = None,
        sccache_conf: Optional[dagger.Secret] = None,
    ) -> str:
        """Smoke-test the workspace container (delegates to handl-ci)."""
        return await dag.handl_ci().doctor(
            source=source, repo=REPO, token=token, sccache_conf=sccache_conf)

    @function
    async def test_shard(
        self,
        source: Source,
        shard: int = 1,
        shards: int = 1,
        jobs: int = 2,
        token: Optional[dagger.Secret] = None,
        sccache_conf: Optional[dagger.Secret] = None,
    ) -> str:
        """One nextest partition — delegates to handl-ci."""
        return await dag.handl_ci().test_shard(
            source=source, shard=shard, shards=shards, jobs=jobs,
            repo=REPO, token=token, sccache_conf=sccache_conf)

    @function
    async def doctests(
        self,
        source: Source,
        token: Optional[dagger.Secret] = None,
        sccache_conf: Optional[dagger.Secret] = None,
    ) -> str:
        """Workspace doctests — delegates to handl-ci."""
        return await dag.handl_ci().doctests(
            source=source, repo=REPO, token=token, sccache_conf=sccache_conf)

    @function
    async def test(
        self,
        source: Source,
        shards: int = 1,
        jobs: int = 2,
        token: Optional[dagger.Secret] = None,
        sccache_conf: Optional[dagger.Secret] = None,
    ) -> str:
        """Full test gate — delegates to handl-ci."""
        return await dag.handl_ci().test(
            source=source, shards=shards, jobs=jobs,
            repo=REPO, token=token, sccache_conf=sccache_conf)

    @function
    async def coverage_report(
        self,
        source: Source,
        token: Optional[dagger.Secret] = None,
        sccache_conf: Optional[dagger.Secret] = None,
    ) -> dagger.Directory:
        """coverage/ directory (html + lcov) — delegates to handl-ci."""
        return await dag.handl_ci().coverage_report(
            source=source, repo=REPO, token=token, sccache_conf=sccache_conf)

    @function
    async def coverage_gate(
        self,
        source: Source,
        token: Optional[dagger.Secret] = None,
        sccache_conf: Optional[dagger.Secret] = None,
    ) -> str:
        """Blocking tooling-coverage gate — delegates to handl-ci."""
        return await dag.handl_ci().coverage_gate(
            source=source, repo=REPO, token=token, sccache_conf=sccache_conf)

    # -- platform-specific ---------------------------------------------------

    @function
    async def dogfood(
        self,
        source: Source,
        token: Optional[dagger.Secret] = None,
        cli_source: Optional[dagger.Directory] = None,
        sccache_conf: Optional[dagger.Secret] = None,
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
        hc = dag.handl_ci()
        ws = hc.workspace(source=source, repo=REPO, token=token,
                          sccache_conf=sccache_conf)
        ws = hc.with_sibling(ctr=ws, name="cli", source=cli_source, main_repo=REPO)
        script = "./scripts/dogfood-gate.sh"
        if jobs:
            script = f"HNL_DOGFOOD_JOBS={jobs} {script}"
        return await hc.run_script(
            ctr=ws, script=script,
            authed=token is not None,
            sccache_env=sccache_conf is not None,
        ).stdout()

    @function
    async def ci(
        self,
        source: Source,
        shards: int = 1,
        jobs: int = 2,
        token: Optional[dagger.Secret] = None,
        cli_source: Optional[dagger.Directory] = None,
        sccache_conf: Optional[dagger.Secret] = None,
        dogfood_jobs: int = 0,
        sequential: bool = False,
    ) -> str:
        """The whole gate: tests + doctests, coverage gate, dogfood.

        Concurrent by default; `sequential` runs stages one by one for
        memory-constrained boxes (11 GiB can't hold three rust builds).
        dogfood runs only when a cli source or token is provided.
        """
        hc = dag.handl_ci()
        named = {
            "test": hc.test(source=source, shards=shards, jobs=jobs,
                         repo=REPO, token=token, sccache_conf=sccache_conf),
            "coverage-gate": hc.coverage_gate(
                source=source, repo=REPO, token=token, sccache_conf=sccache_conf),
        }
        if cli_source is not None or token is not None:
            named["dogfood"] = self.dogfood(source=source, token=token, cli_source=cli_source,
                             sccache_conf=sccache_conf, jobs=dogfood_jobs)
        if sequential:
            results = {}
            for name, coro in named.items():
                try:
                    results[name] = await coro
                except Exception as e:  # noqa: BLE001 — report all stages
                    results[name] = e
        else:
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
