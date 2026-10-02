import asyncio

from .diagram_map import DiagramMap


class ScenarioError(Exception):
    pass


def _parse_duration(value):
    text = str(value).strip().lower()
    if text.endswith("ms"):
        return float(text[:-2]) / 1000.0
    if text.endswith("s"):
        return float(text[:-1])
    return float(text) / 1000.0


async def _wait_serial(session, needle, timeout):
    loop = asyncio.get_running_loop()
    deadline = loop.time() + timeout
    while loop.time() < deadline:
        if session.serial_contains(needle):
            return
        await asyncio.sleep(0.05)
    raise ScenarioError(f"serial never contained {needle!r}")


async def _write_serial(session, payload):
    if isinstance(payload, str):
        data = payload.encode("utf-8")
    else:
        data = bytes(int(byte) for byte in payload)
    await session.write_serial(data)


async def _set_control(session, diagram, step):
    part_id = step["part-id"]
    control = step["control"]
    value = step["value"]

    if control == "pressed":
        pin = diagram.board_pin_for_part(part_id)
        if not str(pin).isdigit():
            raise ScenarioError(f"part '{part_id}' resolves to non-GPIO pin {pin!r}")
        await session.set_pin(int(pin), 0 if int(value) == 1 else 1)
    elif control == "value":
        pin = diagram.board_pin_for_part(part_id)
        if not str(pin).isdigit():
            raise ScenarioError(f"part '{part_id}' resolves to non-ADC pin {pin!r}")
        await session.set_adc(int(pin), int(value))
    else:
        raise ScenarioError(f"unsupported control {control!r}")


async def _expect_pin(session, diagram, step, timeout):
    part_id = step["part-id"]
    pin = str(step["pin"])
    expected = int(step["expected"])
    resolved = pin if diagram.is_board(part_id) else diagram.board_pin_for_part(part_id, pin)

    known = session.pin_states.get(resolved)
    if known == expected:
        return
    if known is None and expected == 0:
        return

    loop = asyncio.get_running_loop()
    deadline = loop.time() + timeout
    while loop.time() < deadline:
        if session.pin_states.get(resolved) == expected:
            return
        await asyncio.sleep(0.02)
    raise ScenarioError(
        f"pin {resolved} expected {expected}, last seen {session.pin_states.get(resolved)}"
    )


async def run_scenario(session, scenario, diagram_path, timeout=15.0):
    diagram = DiagramMap(diagram_path)
    steps = scenario.get("steps", [])

    for index, step in enumerate(steps):
        if not isinstance(step, dict) or not step:
            raise ScenarioError(f"step {index} is not a valid mapping")
        key = next(iter(step))
        try:
            if key == "delay":
                await asyncio.sleep(_parse_duration(step[key]))
            elif key == "wait-serial":
                await _wait_serial(session, step[key], timeout)
            elif key == "write-serial":
                await _write_serial(session, step[key])
            elif key == "set-control":
                await _set_control(session, diagram, step[key])
            elif key == "expect-pin":
                await _expect_pin(session, diagram, step[key], timeout)
            else:
                raise ScenarioError(f"unsupported step {key!r}")
        except ScenarioError:
            print(f"  [{index}] FAIL {key}")
            raise
        print(f"  [{index}] PASS {key}")
