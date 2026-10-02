import asyncio
import sys
import uuid

sys.path.insert(0, "/workspace")
from tests.velxio.runner.ws_client import SimSession


async def main():
    s = SimSession("ws://localhost", "diag-" + uuid.uuid4().hex)
    await s.connect()
    await s.boot("rust/dist/firmware.merged.bin")
    for _ in range(200):
        if "Found 5" in s.serial:
            break
        await asyncio.sleep(0.1)
    await s.set_pin(4, 0)
    await asyncio.sleep(0.6)
    await s.set_pin(4, 1)
    await asyncio.sleep(12.0)
    print("=== SERIAL ===")
    print(s.serial.encode("utf-8", "replace").decode("ascii", "replace"))
    print("=== END SERIAL ===")
    await s.stop()


asyncio.run(main())
