#!/usr/bin/env python3
"""Remove the optional DataCount section from a validated WASM module."""

from pathlib import Path
import sys


def read_uleb(data: bytes, offset: int) -> tuple[int, int]:
    value = 0
    shift = 0
    while True:
        if offset >= len(data) or shift >= 35:
            raise ValueError("invalid WASM section length")
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            return value, offset
        shift += 7


def strip_datacount(source: bytes) -> bytes:
    if source[:8] != b"\x00asm\x01\x00\x00\x00":
        raise ValueError("input is not a WebAssembly 1.0 binary")

    result = bytearray(source[:8])
    offset = 8
    data_count_sections = 0
    while offset < len(source):
        section_start = offset
        section_id = source[offset]
        section_size, payload_start = read_uleb(source, offset + 1)
        section_end = payload_start + section_size
        if section_end > len(source):
            raise ValueError("WASM section extends past end of input")
        if section_id == 12:
            data_count_sections += 1
            if data_count_sections > 1:
                raise ValueError("WASM module has multiple DataCount sections")
        else:
            result.extend(source[section_start:section_end])
        offset = section_end

    return bytes(result)


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit(
            "usage: strip-wasm-datacount.py INPUT.wasm OUTPUT.wasm"
        )
    source_path, output_path = map(Path, sys.argv[1:])
    output_path.write_bytes(strip_datacount(source_path.read_bytes()))


if __name__ == "__main__":
    main()
