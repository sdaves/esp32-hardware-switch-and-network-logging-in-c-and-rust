import asyncio
import base64
import json

import websockets


class SimSession:
    def __init__(self, server, client_id):
        self.url = f"{server}/api/simulation/ws/{client_id}"
        self.ws = None
        self.events = asyncio.Queue()
        self.serial = ""
        self.pin_states = {}
        self._reader_task = None

    async def connect(self):
        self.ws = await websockets.connect(self.url, max_size=None)
        self._reader_task = asyncio.create_task(self._reader())

    async def _reader(self):
        try:
            async for raw in self.ws:
                message = json.loads(raw)
                self._handle(message)
                await self.events.put(message)
        except websockets.ConnectionClosed:
            return

    def _handle(self, message):
        message_type = message.get("type")
        data = message.get("data") or {}
        if message_type == "serial_output":
            self.serial += data.get("data", "")
        elif message_type == "gpio_change":
            self.pin_states[str(data.get("pin"))] = int(data.get("state", 0))

    async def boot(self, firmware_path):
        with open(firmware_path, "rb") as firmware:
            encoded = base64.b64encode(firmware.read()).decode("ascii")
        await self._send(
            "start_esp32",
            {
                "board": "esp32-s3",
                "firmware_b64": encoded,
                "wifi_enabled": False,
            },
        )

    async def set_pin(self, pin, state):
        await self._send("esp32_gpio_in", {"pin": int(pin), "state": int(state)})

    async def set_adc(self, channel, millivolts):
        await self._send(
            "esp32_adc_set",
            {"channel": int(channel), "millivolts": int(millivolts)},
        )

    async def write_serial(self, data, uart=0):
        await self._send("esp32_serial_input", {"bytes": list(data), "uart": uart})

    async def next_event(self, timeout):
        return await asyncio.wait_for(self.events.get(), timeout)

    def serial_contains(self, needle):
        return needle in self.serial

    async def stop(self):
        try:
            if self.ws is not None:
                await self._send("stop_esp32", {})
        except Exception:
            pass
        finally:
            if self._reader_task is not None:
                self._reader_task.cancel()
                self._reader_task = None
            if self.ws is not None:
                try:
                    await self.ws.close()
                except Exception:
                    pass
                self.ws = None

    async def _send(self, message_type, data):
        if self.ws is None:
            raise RuntimeError("session is not connected")
        await self.ws.send(json.dumps({"type": message_type, "data": data}))
