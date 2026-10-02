"""The Rust-to-driver protocol uses UTF-8 independently of the host's code page."""

from __future__ import annotations

import io
import json
import sys
from typing import cast
import unittest
from unittest.mock import patch

from terminal_protocol import Capture, Request, Run, main


class ProtocolEncoding(unittest.TestCase):
	def test_unicode_request_survives_non_utf8_standard_input(self) -> None:
		request: Request = {
			"binary": "中文 二进制.exe",
			"columns": 80,
			"rows": 24,
			"runs": [{"args": ["中文 进度.json"], "keys": ["真", "假"]}],
		}
		data: bytes = json.dumps(request, ensure_ascii=False).encode("utf-8")
		for encoding in ("ascii", "cp1252", "gbk", "utf-8"):
			with self.subTest(encoding=encoding):
				calls: list[tuple[str, Run, int, int]] = []

				def capture(binary: str, run: Run, columns: int, rows: int) -> Capture:
					calls.append((binary, run, columns, rows))
					return {
						"raw": "真 / 假",
						"visible": "真 / 假",
						"exit_code": 0,
						"terminal_before": "7",
						"terminal_after": "7",
					}

				output: io.StringIO = io.StringIO()
				with io.TextIOWrapper(io.BytesIO(data), encoding=encoding) as source:
					with patch.object(sys, "stdin", source), patch.object(
						sys, "stdout", output
					):
						main(capture)
				self.assertEqual(
					calls, [(request["binary"], request["runs"][0], 80, 24)]
				)
				# The response's ASCII JSON is safe even on a non-UTF-8 stdout pipe.
				response: dict[str, list[Capture]] = cast(
					"dict[str, list[Capture]]",
					json.loads(output.getvalue().encode("ascii")),
				)
				self.assertEqual(response["runs"][0]["visible"], "真 / 假")


if __name__ == "__main__":
	unittest.main()
