#!/usr/bin/env python3
# Prints the code and data sizes of linked Win16 ELF programs against the 64 KB segment limits.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/size_report.py <elf>...
#
# Reads the section headers (`.text` is the code segment, `.data` the data segment with the static heap in it) and prints each program's
# size, what is left of the 64 KB limit, and whether it fits. The stack and local heap the packer adds to the data segment are not counted here.
import os
import struct
import sys

LIMIT = 0x10000


def sections(path):
	with open(path, "rb") as f:
		d = f.read()
	shoff, = struct.unpack_from("<I", d, 32)
	shentsize, shnum, shstrndx = struct.unpack_from("<HHH", d, 46)
	secs = []
	for i in range(shnum):
		name, typ, flags, addr, off, size = struct.unpack_from("<IIIIII", d, shoff + i * shentsize)
		secs.append((name, off, size))
	stroff = secs[shstrndx][1]
	out = {}
	for name, off, size in secs:
		end = d.index(b"\0", stroff + name)
		out[d[stroff + name : end].decode()] = size
	return out


def main():
	if len(sys.argv) < 2:
		sys.exit(__doc__)
	print(f"{'program':<10} {'code':>8} {'of 64 KB':>9} {'data':>8}  fits")
	for path in sys.argv[1:]:
		if not os.path.exists(path):
			continue
		s = sections(path)
		code, data = s.get(".text", 0), s.get(".data", 0)
		fits = "yes" if code < LIMIT else f"NO, {code - LIMIT + 1} bytes over"
		print(f"{os.path.basename(path):<10} {code:>8} {code * 100 // LIMIT:>8}% {data:>8}  {fits}")


main()
