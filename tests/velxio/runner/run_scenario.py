import argparse
import asyncio
import os
import sys

import yaml

from .scenario import ScenarioError, run_scenario
from .ws_client import SimSession


def _build_parser():
    parser = argparse.ArgumentParser(description="Run a Velxio simulation scenario")
    parser.add_argument("--server", required=True, help="e.g. ws://localhost:3080")
    parser.add_argument("--firmware", required=True, help="merged firmware image")
    parser.add_argument("--diagram", required=True, help="Wokwi diagram.json")
    parser.add_argument("--scenario", required=True, help="scenario YAML")
    parser.add_argument("--timeout", type=float, default=15.0, help="per-step timeout (s)")
    return parser


async def _run(args, scenario, client_id):
    session = SimSession(args.server, client_id)
    try:
        await session.connect()
        await session.boot(args.firmware)
        name = scenario.get("name", os.path.basename(args.scenario))
        print(f"scenario: {name}")
        await run_scenario(session, scenario, args.diagram, timeout=args.timeout)
        print("RESULT: PASS")
        return 0
    except ScenarioError as exc:
        print(f"RESULT: FAIL ({exc})")
        return 1
    except Exception as exc:
        print(f"RESULT: FAIL ({type(exc).__name__}: {exc})")
        return 1
    finally:
        await session.stop()


def main(argv=None):
    args = _build_parser().parse_args(argv)

    for path in (args.firmware, args.diagram, args.scenario):
        if not os.path.exists(path):
            print(f"configuration error: missing {path}", file=sys.stderr)
            return 2

    with open(args.scenario, "r", encoding="utf-8") as handle:
        scenario = yaml.safe_load(handle) or {}

    client_id = f"test-{os.path.basename(args.scenario)}-{os.getpid()}"
    return asyncio.run(_run(args, scenario, client_id))


if __name__ == "__main__":
    sys.exit(main())
