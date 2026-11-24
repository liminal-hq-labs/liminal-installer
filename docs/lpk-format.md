# The Liminal Pack format (`.LPK`)

A Liminal Pack holds the files of an installer in chunks of at most 32 KiB, each stored or deflated, with a directory of names, sizes, times and CRC-32s. One reader serves the DOS, Win16 and Win32 runtimes: it is `no_std`, allocates nothing, uses no `unsafe`, and needs one decoded-chunk buffer of at most 32 KiB, a 256-byte read buffer and about 1.5 KiB of stack. One pack may span a set of disks. This page is the specification; the crate [`li-lpk`](../crates/li-lpk) follows it, and where the two differ the crate is wrong.

This is **version 1.0**. The format is experimental, so it may change until the project graduates, but every change goes through the compatibility rule below.

## Table of contents

- [Goals](#goals)
- [Conventions](#conventions)
- [Files and names](#files-and-names)
- [Layout of a volume](#layout-of-a-volume)
- [Header](#header)
- [Footer](#footer)
- [Directory](#directory)
- [Chunks](#chunks)
- [Compression](#compression)
- [Spanning](#spanning)
- [Shared packs](#shared-packs)
- [Self-extracting stubs](#self-extracting-stubs)
- [Checksums](#checksums)
- [Versions and compatibility](#versions-and-compatibility)
- [What a reader must do](#what-a-reader-must-do)
- [Limits](#limits)
- [A worked example](#a-worked-example)
- [Why these choices](#why-these-choices)
- [Tools](#tools)

## Goals

- **Small readers.** A 16-bit runtime cannot allocate more than a segment. A file is read in chunks of at most 32 KiB, and a decoded chunk is the only large buffer. The unpacker's code is about 6.5 KB on a 386 (see [Why these choices](#why-these-choices)).
- **Every failure is a typed error.** A pack is untrusted input: a truncated file, a flipped bit or random bytes produce an error from a fixed list, never a panic and never wrong bytes handed to the caller.
- **8.3 first.** Every file has an upper-case 8.3 path that DOS can create, and may also have a long name for systems that can use it.
- **Spanning by design.** A pack can be written as a set of volumes, one per disk, and the reader asks the caller for each next volume.
- **Findable.** A self-extracting stub finds the pack from the last bytes of its own file.
- **One format for every runtime.** It replaces the DOS runtime's `SETUP.DAT` payload layout, and it is the pack that Quill's planned `.QPK` was going to be. It takes its container ideas from Lt. Fizz 2's `.FAP`, which stays Lt. Fizz 2's own format.

Not goals: random access to the middle of a file, encryption, signing and patching. Signing is done over the whole file by the feed, as for every other artefact.

## Conventions

- All integers are little-endian. Offsets and sizes are `u32`, so a volume and the sum of the files' sizes are each under 4 GiB.
- Offsets in a volume count from the first byte of that volume's header. A pack that sits inside a larger file (a stub in front of it) is addressed from its own start.
- A *reserved* field is written as zero. A reader ignores it unless this page says otherwise.
- Names are `/`-separated relative paths. They never start with `/`, never have an empty part, `.` or `..`, never contain a control character or any of `\ : * ? " < > |`. A reader refuses a directory with a name that breaks this ([`BadName`](#what-a-reader-must-do)).

## Files and names

| Name | Is |
|---|---|
| `NAME.LPK` | A pack, or the first volume of a set |
| `NAME.L02` to `NAME.L99` | Volumes 2 to 99 of a set, when volumes are separate files |

Table 1. File names.

The extension is a hint for people and tools; a reader trusts the header. A set on floppy disks has one volume per disk, each named `NAME.LPK` or `NAME.Lnn` as it is written; the volume number is in the header, so a mislabelled disk is caught. The tools write more than 99 volumes only when each is named explicitly.

## Layout of a volume

```text
volume 1    header | directory | chunks … | [next-volume marker] | footer
volume n>1  header | chunks … | [next-volume marker] | footer
```

Every volume is a complete, self-checking file. The directory is in volume 1 only. Chunk data follows in stream order: the files in directory order, each file's chunks in order, and a set moves from one volume to the next only at a chunk boundary.

## Header

48 bytes at offset 0 of every volume. A later minor version may make it larger; `header_size` says where the data starts.

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | Magic: `4C 50 4B 1A` (`LPK` then 0x1A, which stops `TYPE` printing the binary) |
| 4 | 1 | Major version: 1 |
| 5 | 1 | Minor version: 0 |
| 6 | 2 | `header_size`: 48 in version 1.0, at most 1024 |
| 8 | 2 | Flags. The low byte is *must understand*: a reader refuses a pack with any of those bits set that it does not know (none are defined in 1.0). The high byte is advisory and ignored |
| 10 | 2 | Volume number, from 1 |
| 12 | 2 | Volume count, at least the volume number |
| 14 | 1 | Chunk size as a power of two, 9 to 15: a chunk decodes to at most `1 << n` bytes. 15 is 32 KiB |
| 15 | 1 | Reserved |
| 16 | 4 | Set id: the same in every volume of a set. The packer's default is the CRC-32 of the directory |
| 20 | 4 | File count: the number of directory entries |
| 24 | 4 | Directory offset, volume 1 only (0 elsewhere); at least `header_size` |
| 28 | 4 | Directory length in bytes, volume 1 only (0 elsewhere). It is 0 exactly when the file count is 0 |
| 32 | 4 | CRC-32 of the directory, volume 1 only (0 elsewhere) |
| 36 | 4 | Total size: the sum of every file's size, the same in every volume |
| 40 | 4 | Volume length: bytes from the start of this header to the end of this volume's footer |
| 44 | 4 | CRC-32 of header bytes 0 to 43 |

Table 2. The header.

The reader checks, in this order: the magic, the major version, the header checksum, the must-understand flags, `header_size`, the volume and count, the chunk size, the volume length (at least `header_size` plus the footer), and in volume 1 that the directory lies between the header and the footer and that the file count fits the directory length.

## Footer

The last 32 bytes of every volume. It exists so a stub can find the pack from the end of its file and so a cut-short volume is noticed before any data is read.

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | Magic: `4C 50 4B 45` (`LPKE`) |
| 4 | 4 | Pack length: equal to the header's volume length. A stub starts the pack at `file length − pack length` |
| 8 | 4 | Set id, equal to the header's |
| 12 | 2 | Volume number, equal to the header's |
| 14 | 2 | Volume count, equal to the header's |
| 16 | 4 | The header's CRC-32 (bytes 44 to 47 of the header) |
| 20 | 8 | Reserved |
| 28 | 4 | CRC-32 of footer bytes 0 to 27 |

Table 3. The footer.

A reader reads the footer at `volume length − 32` whenever it opens a volume and refuses the volume with `BadFooter` when the magic, the checksum or any repeated field disagrees.

## Directory

The directory is a run of entries, one per file or empty directory, in stream order. An entry is a fixed 26-byte part followed by two names and any bytes a later minor version adds.

| Offset | Size | Field |
|---|---|---|
| 0 | 2 | Entry length: 26 plus the two names, plus any extension; at most 512. A reader skips to the next entry by this length |
| 2 | 2 | Flags (see below) |
| 4 | 4 | Size in bytes after unpacking |
| 8 | 4 | CRC-32 of the unpacked bytes (0 for an empty file) |
| 12 | 2 | DOS time: hours in bits 15–11, minutes 10–5, seconds ÷ 2 in 4–0 |
| 14 | 2 | DOS date: years since 1980 in bits 15–9, month 8–5, day 4–0 |
| 16 | 1 | DOS attributes: 1 read-only, 2 hidden, 4 system, 0x10 directory, 0x20 archive |
| 17 | 1 | Targets: a mask of 1 DOS, 2 Win16, 4 Win32, 8 POSIX; 0 means every target |
| 18 | 2 | Volume of the file's first chunk (ignored when the size is 0) |
| 20 | 4 | Offset of the file's first chunk header in that volume (ignored when the size is 0) |
| 24 | 1 | Short name length, 1 to 96 |
| 25 | 1 | Long name length, 0 to 255 |
| 26 | short | Short name: the upper-case 8.3 path, `/` between parts |
| … | long | Long name: the path in UTF-8, or empty when it is the same as the short name |

Table 4. A directory entry.

**Short names.** Each part is 1 to 8 characters, optionally a dot and 1 to 3 more, drawn from `A–Z 0–9 _ - ! # $ % & ' ( ) @ ^ { } ~`. There is at most one dot in a part. Short names are unique ignoring case; the packer refuses a duplicate. An installer on DOS creates files and folders by their short names.

**Long names** are what a system that can use them should create. The reader checks them for safety (the name rules above) but not for UTF-8; a tool that turns them into file names does that check.

**Flags.** Bits 0 to 11 are advisory: a reader that does not know a bit ignores it. Bits 12 to 15 are *must understand*, and an entry with any set is refused with `UnsupportedFeature`.

| Bit | Name | Meaning |
|---|---|---|
| 0x0001 | `EXECUTABLE` | Make the file executable where the system has the notion |
| 0x0002 | `REPLACE_IF_NEWER` | An existing file with a later time is kept |
| 0x0004 | `SHARED` | A file shared between products; do not remove it while another product uses it |
| 0x0008 | `DIRECTORY` | An empty directory to create; its size is 0 and it has no chunks |
| 0xF000 | must understand | None defined in 1.0 |

Table 5. Entry flags.

A directory entry's names follow the same rules; a file's parent directories are made from its path and need no entry of their own.

**Order.** The entries are in stream order: the chunks of entry *k* come before those of entry *k + 1* in the data, and a set's volumes are in order. Unpacking in directory order therefore moves through a disk set strictly forwards. The reader does not require this; a caller that unpacks in another order will ask for earlier volumes again.

**Reading the directory.** A runtime reads the whole directory once, into a buffer of `directory length` bytes, and keeps it. That is how a disk set avoids being swapped back to disk 1 for each file. On a 64 KB segment a directory of about 1 400 files is the limit.

## Chunks

A file's data is a run of chunks. Each chunk is a 12-byte header followed by its stored bytes, and chunks are packed one after another with no padding.

| Offset | Size | Field |
|---|---|---|
| 0 | 1 | Method: 0 stored, 1 deflate, 0xFF next-volume marker |
| 1 | 1 | Reserved |
| 2 | 2 | Decoded length, 1 to `1 << chunk size`. 0 in a marker |
| 4 | 2 | Stored length, at most `1 << chunk size`. For method 0 it equals the decoded length. 0 in a marker |
| 6 | 2 | Argument: 0, except in a marker, where it is the number of the volume the stream continues in |
| 8 | 4 | CRC-32 of chunk header bytes 0 to 7 followed by the stored bytes |

Table 6. A chunk header.

Every chunk except a file's last decodes to exactly `1 << chunk size` bytes (the packer's rule; a reader accepts any length in range). The decoded lengths of a file's chunks add up to its size, and the reader refuses a chunk that would pass it. A file of size 0 has no chunks.

A chunk is decoded into the caller's window and checked before any of it is handed on, so a damaged chunk is never delivered, and the CRC-32 of the file (in the directory) is checked after its last chunk.

The next-volume marker is described under [Spanning](#spanning). A reader refuses any other method with `UnsupportedMethod`, and any chunk whose header and stored bytes would leave the volume (past the footer) with `BadChunk`.

## Compression

Each chunk is one **raw DEFLATE** stream ([RFC 1951](https://www.rfc-editor.org/rfc/rfc1951): no zlib or gzip wrapper) that **starts and ends within the chunk and refers to nothing outside it**. Because a chunk's matches only look back into its own output, the decoded-chunk buffer is also the window: a 16-bit unpacker needs one buffer of at most 32 KiB and nothing else large. The packer stores a chunk (method 0) whenever deflate would not make it smaller, so incompressible data costs the 12-byte header and nothing more.

A decoder must refuse, with `Inflate(…)`: a block of type 3; a stored block whose length and complement disagree; an over-subscribed code, a literal/length count over 286, a distance count over 30, or a dynamic block with no end-of-block code; a code the tables do not have; a distance before the start of the chunk; output past the chunk's decoded length; a stream that ends before it; stored bytes left over after the last block; and input that ends inside the stream. An incomplete code is accepted, as zlib does for a lone distance code, and is refused only if the stream uses a bit pattern it lacks.

Independent chunks cost compression. Against one deflate stream for the whole file, at level 9 and including the 12-byte headers, measured on this repository's documents (197 KB of text) and on 3 MB of a debug executable:

| Chunk size | Text | Executable |
|---|---|---|
| Whole file | 73 276 bytes | 936 384 bytes |
| 32 KiB | +7.3 % | +2.7 % |
| 16 KiB | +14.0 % | +4.6 % |
| 4 KiB | +30.5 % | +12.2 % |

Table 7. The price of independent chunks.

The header's chunk size lets a pack trade the other way for a runtime with less memory: a Win16 build that can spare only 8 KiB sets 13.

## Spanning

A set is a list of volumes with the same set id, volume count, chunk size and total size. Rules for writing:

1. A chunk is never split: it lies wholly in one volume. A volume therefore holds an integral number of chunks.
2. When the next chunk of a file does not fit in the volume with room left for a marker and the footer, and the file already has a chunk in this volume, the packer writes a **next-volume marker** (method 0xFF, argument *n* + 1) after the last chunk and starts volume *n* + 1 with the chunk. When the chunk is a file's first, no marker is written: the entry's volume and offset point at the new volume.
3. A new volume starts with its header and then the chunk, so the first chunk header of a volume is at `header_size`.
4. Volume 1 carries the directory. Its size limit can be smaller than the others so a stub or a README shares the first disk.
5. A volume has at least one chunk, and every volume is at most the limit it was given (header, chunks, marker and footer included).

Rules for reading, which the reader follows when it needs a chunk:

- It calls `Source::open_volume(n)` and checks the volume's header and footer. `WrongVolume` means the header's volume number, set id, volume count or chunk size is not the set's; the caller can prompt again and retry.
- At a marker it checks the argument is the current volume plus one, opens that volume and continues at its first chunk. Two markers in a row are `BadChunk`.
- A `Source` that cannot supply a volume answers `NoVolume`, or `Cancelled` when the person declines. The reader returns that as `Source(…)`, and the caller decides what to show.
- Disk swapping is the caller's concern: the reader never reads a volume it was not asked for.

A DOS or Windows runtime that installs from a disk set reads the directory from disk 1, then unpacks in directory order, and the `Source` prompts when `open_volume` is called. A set for several systems is one set per system, each its own pack; the **targets** mask lets one pack hold files for more than one system when the disks are shared.

## Shared packs

Several installers on one disk or CD (a `WIN31\` and a `WIN95\` program beside a DOS one, say) often carry the same files. A **shared pack** holds those files once. This is a convention for writers and runtimes, not a change to the format: a shared pack is an ordinary version 1.0 pack, read by the same reader, and the writer still writes the lowest minor version that can express it.

- **Name.** The pack is `COMMON.LPK` (and `COMMON.L02` and so on if it spans), in the folder the runtimes agree on, beside or above the programs that read it.
- **Entry names.** Entries are `Cnnnnnnn` in the short name, where *nnnnnnn* is a seven-digit decimal number from 0000001, and the long name holds the real path, as for a Win32 pack ([Directory](#directory)). A consumer finds an entry by that name and never by its position, so the order of the shared pack and the order of its consumers' projects do not have to agree.
- **Target mask.** An entry's mask is the union of the target bits of the programs that use it, so a file used by DOS, Windows 3.1 and Windows 95 carries 7. A program reading an entry whose mask does not include its own bit has been given the wrong pack or a damaged project; a reader may check it with `Reader::begin_for`.
- **Chunk size.** The pack's chunk size is that of the smallest window among its consumers, because each of them decodes one chunk at a time into its own window. A set read by a Windows 3.1 program uses 4 KiB (`chunk_log2` 12).
- **Set id.** The header's set id is what ties a consumer to its shared pack. Each consumer's project records the set id of the shared pack it expects, and refuses a `COMMON.LPK` with another one (`WrongVolume`) before it writes anything. That catches a pack from another build or another product's disk.
- **Reading.** A consumer's own pack (`SETUP.LPK`) still holds its project and the files only it needs. Uninstalling reads neither pack, since the receipt records each file.

The golden file `crates/li-lpk/tests/golden/common.lpk` is a shared pack of this shape (three entries, 4 KiB chunks, mask 7). It is rewritten, with the other golden files, by `LI_UPDATE_GOLDEN=1 cargo test -p li-lpk --test golden`.

## Self-extracting stubs

A stub is an executable with a pack appended. It reads the last 32 bytes of its own file, parses the footer (`Footer::parse`: magic and checksum), and starts the pack at `file length − pack length`. A `Source` for the stub adds that start to every offset. For a set, the stub is on disk 1 and the later volumes are separate files; give the first volume a smaller limit to leave room for the stub.

## Checksums

CRC-32 (polynomial 0xEDB88320, the zip and PNG one, initial value and final XOR 0xFFFFFFFF) throughout. A 16-entry table is enough.

| Where | Covers |
|---|---|
| Header | Header bytes 0 to 43 |
| Footer | Footer bytes 0 to 27, and it repeats the header's checksum so a header from another volume or another edit is caught |
| Directory | The whole directory, checked when it is read |
| Chunk | The chunk header's first 8 bytes and the stored bytes |
| File | The unpacked bytes, checked after the last chunk |

Table 8. What each checksum covers.

Every byte of a volume is under some checksum except the bytes after `header_size` that a later minor version adds, and any single flipped bit is detected (the tests flip every bit of a pack and of a three-volume set).

## Versions and compatibility

The version is a major and a minor number in the header.

- **A reader accepts any minor version of the major it knows**, and refuses another major with `UnsupportedVersion` before it looks at anything else.
- **A minor version may add** fields that an older reader can safely ignore: bytes after the 48-byte header (found by `header_size`), bytes at the end of a directory entry (found by the entry length), new advisory flag bits, and use of reserved fields. It never changes the meaning of an existing field.
- **A change an older reader cannot ignore** is made with a *must-understand* bit (header flag bits 0 to 7, entry flag bits 12 to 15) so the older reader refuses it cleanly, or with a new chunk method, which it refuses as `UnsupportedMethod`. A change that cannot be made that way is a new major version.
- A writer writes the lowest minor version that can express the pack.

Within a major version a pack written by any release is read by every later release. The golden files in `crates/li-lpk/tests/golden/` are read by the test suite on every run, so a change that breaks them fails.

## What a reader must do

The API (in `li-lpk`) is a `Reader` plus two traits, with the source passed to each call so the same device can be used for other work between chunks:

```rust
trait Source { fn open_volume(&mut self, volume: u16) -> Result<(), SourceError>;
               fn read_at(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), SourceError>; }
trait Sink   { fn write(&mut self, data: &[u8]) -> Result<(), SinkError>; }

Reader::open(src)                                  // volume 1: header and footer
reader.read_directory(src, buf)                    // checks the directory CRC-32
reader.directory(bytes)                            // the same directory again, from the kept buffer
reader.begin(src, &entry) -> Extract               // then step until None
reader.begin_for(src, &entry, target) -> Extract   // the same, refusing an entry not for `target`
reader.step(src, &mut extract, window) -> Option<usize>
reader.extract(src, &entry, window, sink)          // begin and step, in pieces of 4 KiB
```

**Memory.** The `Reader` is about 350 bytes including its 256-byte read buffer. A decode uses about 1.5 KiB of stack (two Huffman tables and the code-length scratch). The window is the chunk size, at most 32 KiB. The directory is the caller's buffer. Nothing is allocated, so on Win16 the window and directory can be static or in one global block, and the 32-bit runtimes can use the heap.

**Code size.** The reader's path (open, directory, names, unpack, inflate, CRC-32) built for 32-bit x86 with `opt-level = "z"`, link-time optimization and `panic = "abort"` measures about 6.6 KB of code and 0.35 KB of tables; see [Why these choices](#why-these-choices).

**Errors.** Every failure is an `Error`; none panics, and a failed read leaves the reader usable for another file.

| Error | Meaning |
|---|---|
| `BadMagic`, `UnsupportedVersion`, `UnsupportedFeature` | Not an LPK, a major version unknown, a must-understand bit unknown |
| `BadHeader`, `HeaderChecksum`, `BadFooter`, `Truncated` | A header field out of range, the header or footer damaged, the volume cut short |
| `WrongVolume` | The volume handed over is not the next of this set |
| `DirectoryChecksum`, `BadEntry`, `BadName` | The directory damaged, an entry malformed, a name unsafe or not 8.3 |
| `WrongTarget` | `begin_for` was given an entry whose target mask does not include the reader's own bit |
| `BadChunk`, `ChunkChecksum`, `UnsupportedMethod`, `Inflate(…)` | A chunk header or its data damaged |
| `SizeMismatch`, `FileChecksum` | The chunks do not add up to the size, the file does not match its CRC-32 |
| `BufferTooSmall` | The window or directory buffer is smaller than the pack needs |
| `Source(…)`, `Sink` | The device failed or the person cancelled; the sink refused |

Table 9. Errors.

## Limits

| Thing | Limit |
|---|---|
| Volume, and total size of the files | Under 4 GiB each |
| Volumes in a set | 65 535 (the file-naming convention names 99) |
| A chunk | 512 bytes to 32 KiB, decoded |
| A directory entry | 512 bytes; short name 96, long name 255 |
| Files | As many as fit a directory a runtime can hold in memory |

Table 10. Limits.

## A worked example

One file, `HI.TXT` (long name `hi.txt`), holding `Hello, LPK!` and a carriage return and line feed, with the DOS epoch as its time, in one volume. The 13 bytes do not deflate smaller, so the chunk is stored.

```text
0000  4c 50 4b 1a 01 00 30 00 00 00 01 00 01 00 0f 00
0010  7a 82 74 dd 01 00 00 00 30 00 00 00 26 00 00 00
0020  7a 82 74 dd 0d 00 00 00 8f 00 00 00 ab d8 be a7
0030  26 00 00 00 0d 00 00 00 c9 9e c8 4b 00 00 21 00
0040  20 00 01 00 56 00 00 00 06 06 48 49 2e 54 58 54
0050  68 69 2e 74 78 74 00 00 0d 00 0d 00 00 00 ed 2e
0060  2d 20 48 65 6c 6c 6f 2c 20 4c 50 4b 21 0d 0a 4c
0070  50 4b 45 8f 00 00 00 7a 82 74 dd 01 00 01 00 ab
0080  d8 be a7 00 00 00 00 00 00 00 00 87 fb 41 fb
```

| Bytes | Is |
|---|---|
| 0x00–0x2F | The header: version 1.0, `header_size` 48, volume 1 of 1, chunk size 2^15, set id `dd74827a`, 1 file, directory at 48 for 38 bytes, total size 13, volume length 143 (0x8F), the header's CRC-32 `a7bed8ab` |
| 0x30–0x55 | The directory entry: length 38, no flags, size 13, CRC-32 `4bc89ec9`, time 0, date 0x0021 (1980-01-01), archive attribute, every target, first chunk in volume 1 at 0x56, names of 6 and 6 bytes, `HI.TXT` and `hi.txt` |
| 0x56–0x61 | The chunk header: method 0 (stored), decoded and stored length 13, argument 0, CRC-32 `202d2eed` |
| 0x62–0x6E | The 13 stored bytes |
| 0x6F–0x8E | The footer: `LPKE`, pack length 143, the set id, volume 1 of 1, the header's CRC-32, 8 reserved bytes and its own CRC-32 |

Table 11. The example, byte by byte.

A test (`crates/li-lpk/tests/golden.rs`) reads this page, rebuilds the dump and compares it with what the packer writes, so the example cannot drift from the code.

## Why these choices

**Deflate, in independent chunks.** The Fizz 2 `.FAP` format already uses raw deflate with a 2.3 KB decoder and ships LZSS and LZ4 as the smaller alternatives it measured (about 24 % bigger output); the Quill planner's `.QPK` proposal chose chunked deflate of at most 32 KiB for the same reason as here, a 32 KiB window on Win16. The two agree, so LPK takes raw deflate and the 32 KiB chunk rule. A smaller custom codec would save a kilobyte of code and lose a quarter of the compression on every disk, and deflate has encoders and decoders on every host. The Fizz 2 inflater reads a whole stored section from memory into a whole output buffer, which suits a game that loads a level, but an installer cannot hold a 32 KiB chunk twice on Win16, so `li-lpk` has its own decoder of the same kind (a canonical-code, bit-at-a-time decoder in the style of `tinf` and `puff`) that pulls its input through a 256-byte buffer. It is checked against zlib's and miniz's output, against every bit flip and prefix of real streams, and against random bytes. `miniz_oxide`, which the DOS runtime used before, needs `alloc` and a decoder state of its own; replacing it with this reader made `SETUP.EXE` 8 192 bytes smaller (161 280 to 153 088), so the runtime no longer links it.

**What was taken from `.FAP` and `.QPK`.** From `.FAP`: the magic that ends in 0x1A, a version and a header size so the header can grow, must-be-zero flags so an unknown feature is refused, a directory with offsets, stored and decoded lengths and a stored-chunk fallback, and every section checked on its own. From `.QPK`: entries that name files (8.3, a DOS date and time, a target mask), and chunked deflate that lets the unpacker work in one 32 KiB buffer.

**What differs.** CRC-32 instead of FNV-1a, because the receipts, the zip tools and `liminst` already use CRC-32 and a file's checksum can then be checked against a `zip -T` listing. No 16-byte alignment, which would waste an average of 8 bytes per chunk on a disk where every byte counts and buys nothing on a 386. No compatibility number: a schema hash belongs to the application that reads its sections, and an installer pack carries files. Variable-length entries, for long names. A footer, for stubs and for noticing a cut-short disk. Spanning.

**Reader size.** The reader's path measured 6.8 KB of code in total on 32-bit x86 (`opt-level = "z"`, `panic = "abort"`, link-time optimization, an `i586-unknown-linux-gnu` link of a probe that opens the pack, reads the directory and unpacks every file), which is 6.6 KB without the probe's own glue and a stand-in `memcpy`. That is over the 6 KB aimed for by about 10 %. About 0.5 KB is name validation, which is the price of refusing a path that escapes the install folder, and the rest is the header and footer checks, the chunk loop and the decoder. The tables take 0.35 KB. Win16 code16 builds need their own measurement.

## Tools

`liminst lpk` is the command line, and the crate's `host` module is what it is built on (see D49).

```sh
liminst lpk pack DIR -o SETUP.LPK [--volume-size 1457664 | floppy-1440] [--first-volume-size N] [--chunk-log2 15] [--target dos] [--fixed-time]
liminst lpk list SETUP.LPK
liminst lpk unpack SETUP.LPK -o DIR [--short-names]
liminst lpk verify SETUP.LPK
```

`pack` gives a name that is not 8.3 a generated short name (`LONGFI~1.TXT`) and keeps the original as the long name, writes empty folders as directory entries, and sets `EXECUTABLE` for programs. `--fixed-time` stores the DOS epoch so the same folder always packs to the same bytes. `verify` reads every volume and checks every chunk, every file and the totals; it exits with 2 on the first failure and says which.
