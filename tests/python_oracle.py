"""Independent CPython oracle for generated, trusted test expressions only."""

import json
import ast
import struct
import sys
from typing import TypedDict


class Observation(TypedDict):
	source: str
	kind: str
	type: str
	value: str
	bits: str


class Case(TypedDict):
	source: str
	bindings: dict[str, str]


def observe(case: Case) -> Observation:
	source: str = case["source"]
	bindings: dict[str, object] = {
		name: ast.literal_eval(literal) for name, literal in case["bindings"].items()
	}
	try:
		value: object = eval(source, {"__builtins__": {}}, bindings)
	except (ArithmeticError, TypeError) as error:
		return {
			"source": source,
			"kind": "error",
			"type": type(error).__name__,
			"value": "",
			"bits": "",
		}
	return {
		"source": source,
		"kind": "value",
		"type": type(value).__name__,
		"value": repr(value),
		"bits": struct.pack(">d", value).hex() if isinstance(value, float) else "",
	}


def main() -> None:
	cases: list[Case] = json.load(sys.stdin)
	json.dump([observe(case) for case in cases], sys.stdout)


if __name__ == "__main__":
	main()
