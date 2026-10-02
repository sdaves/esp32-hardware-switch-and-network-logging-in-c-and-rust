import json


class DiagramMap:
    def __init__(self, path):
        with open(path, "r", encoding="utf-8") as handle:
            self.doc = json.load(handle)

        self.parts = {part["id"]: part for part in self.doc.get("parts", [])}
        self.connections = self.doc.get("connections", [])
        self.board_ids = {
            part_id
            for part_id, part in self.parts.items()
            if str(part.get("type", "")).startswith("board-")
        }
        if not self.board_ids:
            raise ValueError("diagram.json declares no board part")
        self.board_id = sorted(self.board_ids)[0]

        self._part_pins = {}
        self._build()

    def _build(self):
        for connection in self.connections:
            if len(connection) < 2:
                continue
            self._record(connection[0], connection[1])
            self._record(connection[1], connection[0])

    def _record(self, board_end, other_end):
        board_id, board_pin = self._split(board_end)
        other_id, other_pin = self._split(other_end)
        if board_id not in self.board_ids or other_id in self.board_ids:
            return
        if self._is_power_pin(board_pin):
            return
        self._part_pins.setdefault(other_id, {})[other_pin] = board_pin

    @staticmethod
    def _is_power_pin(pin):
        upper = str(pin).upper()
        return (
            upper.startswith("GND")
            or upper.startswith("VIN")
            or upper.startswith("3V3")
            or upper.startswith("5V")
            or upper == "EN"
        )

    @staticmethod
    def _split(end):
        part_id, _, pin = end.partition(":")
        return part_id, pin

    def is_board(self, part_id):
        return part_id in self.board_ids

    def board_pin_for_part(self, part_id, pin=None):
        pins = self._part_pins.get(part_id)
        if not pins:
            raise KeyError(f"part '{part_id}' is not wired to the board")
        if pin is not None and pin in pins:
            return pins[pin]
        if len(pins) == 1:
            return next(iter(pins.values()))
        for candidate in pins.values():
            if candidate.isdigit():
                return candidate
        return next(iter(pins.values()))
