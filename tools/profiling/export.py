#!/usr/bin/env python3
"""Export Arca VMM stack snapshots as time-weighted Linux perf.data call graphs."""
import argparse
import bisect
import collections
import hashlib
import gzip
import json
import pathlib
import struct
import subprocess
import sys

from elf import Image, discover

MASK = 0xFFFFFFFFFFFFFFFF
V2 = struct.Struct('<7Q16Q32s4096s48Q')


class Unwinder:
    def __init__(self, image, cache):
        self.image = image
        digest = hashlib.sha256(image.data).hexdigest()
        path = cache / (digest + '.unwind-v2.json')
        if not path.exists():
            subprocess.run([sys.executable, str(pathlib.Path(__file__).with_name('unwind.py')),
                            str(image.path), str(path)], check=True, stdout=sys.stderr)
        self.rows = json.loads(path.read_text())
        self.starts = [row[0] for row in self.rows]
        self.span_starts = [[span[0] for span in row[3]] for row in self.rows]

    def state(self, ip):
        index = bisect.bisect_right(self.starts, ip) - 1
        if index < 0 or ip >= self.rows[index][1]:
            return None
        row = self.rows[index]
        span = bisect.bisect_right(self.span_starts[index], ip) - 1
        if span < 0:
            return None
        return row[3][span][1], row[4], row[3][span][2]

    def chain(self, ip, sp, bp, raw):
        origin = sp
        chain = [ip]
        reason = 'depth limit'
        for _ in range(47):
            state = self.state(chain[-1] - (len(chain) > 1))
            if state is None:
                reason = 'no unwind state'
                break
            depth, saved_bp, bp_depth = state
            if depth is None:
                if bp_depth is None:
                    reason = 'unsupported unwind state'
                    break
                return_address = bp + bp_depth
            else:
                return_address = sp + depth
            offset = return_address - origin
            if not 0 <= offset <= len(raw) - 8:
                reason = 'snapshot boundary'
                break
            return_ip = struct.unpack_from('<Q', raw, offset)[0]
            next_sp, next_bp = return_address + 8, bp
            if saved_bp is not None:
                saved_offset = return_address + saved_bp - origin
                if 0 <= saved_offset <= len(raw) - 8:
                    next_bp = struct.unpack_from('<Q', raw, saved_offset)[0]
            if not any(va <= return_ip - 1 < va + size and flags & 1
                       for va, _, _, size, flags in self.image.segments):
                reason = 'end of image stack'
                break
            if next_sp <= sp:
                reason = 'non-increasing stack'
                break
            chain.append(return_ip)
            sp, bp = next_sp, next_bp
        return chain, reason


def extend_chain(chain, recorded):
    for length in range(min(len(chain), len(recorded)), 0, -1):
        suffix = chain[-length:]
        matches = [index for index in range(len(recorded) - length + 1)
                   if recorded[index:index + length] == suffix]
        if len(matches) == 1:
            return chain + recorded[matches[0] + length:]
        if matches:
            break
    return chain


def read_samples(path):
    opener = gzip.open if path.suffix == '.gz' else open
    with opener(path, 'rb') as source:
        header = source.read(8)
        if header != b'ARCASMP2':
            raise ValueError(f'Unknown stack snapshot format: {path}')
        while data := source.read(V2.size):
            if len(data) != V2.size:
                raise ValueError(f'Truncated snapshot: {path}')
            row = V2.unpack(data)
            requested, acquired, mode, ip, sp, bp, depth = row[:7]
            if depth > 48:
                raise ValueError(f'Invalid snapshot depth: {depth}')
            code, raw = row[23:25]
            yield requested, acquired, mode, ip, sp, bp, code, raw, list(row[25:25 + depth])


def write_perf(output, images, samples, cores):
    def record(stream, kind, payload):
        size = (8 + len(payload) + 7) & ~7
        stream.write(struct.pack('<IHH', kind, 2, size))
        stream.write(payload)
        stream.write(b'\0' * (size - 8 - len(payload)))

    attr = bytearray(128)
    sample_type = 1 | 2 | 4 | 32 | 128 | 256
    struct.pack_into('<IIQQQQQ', attr, 0, 1, 128, 0, 10_000_000, sample_type, 0, 0)
    with output.open('w+b') as stream:
        stream.write(b'\0' * 248)
        used = {sample[2] for sample in samples if sample[2] is not None}
        for image_index, image in enumerate(images):
            if image_index not in used:
                continue
            pid = 1000 + image_index
            for core in cores:
                record(stream, 3, struct.pack('<II', pid, 10000 + image_index * 1024 + core)
                       + f'vCPU {core}'.encode() + b'\0')
            for va, size, offset, _, flags in image.segments:
                if flags & 1:
                    record(stream, 1, struct.pack('<IIQQQ', pid, pid, va & ~4095,
                           ((va % 4096 + size + 4095) // 4096) * 4096, offset & ~4095)
                           + str(image.path.resolve()).encode() + b'\0')
        unknown_pid = 1000 + len(images)
        for core in cores:
            record(stream, 3, struct.pack('<II', unknown_pid, 10000 + len(images) * 1024 + core)
                   + f'vCPU {core}'.encode() + b'\0')
        for time, core, image_index, chain, weight, _ in sorted(samples, key=lambda sample: (sample[0], sample[1])):
            if image_index is None:
                image_index, chain = len(images), chain or [0]
            pid, tid = 1000 + image_index, 10000 + image_index * 1024 + core
            payload = struct.pack('<QIIQIIQQ', chain[0], pid, tid, time, core, 0, weight, len(chain))
            payload += struct.pack('<' + 'Q' * len(chain), *chain)
            record(stream, 9, payload)
        data_size = stream.tell() - 248
        name = b'arca-vcpu-wall-ns\0'
        name += b'\0' * (-len(name) % 64)
        description = struct.pack('<II', 1, 128) + attr + struct.pack('<II', 0, len(name)) + name
        stream.write(struct.pack('<QQ', 248 + data_size + 16, len(description)))
        stream.write(description)
        stream.seek(0)
        stream.write(struct.pack('<8s12Q', b'PERFILE2', 104, 144, 104, 144,
                                 248, data_size, 0, 0, 1 << 12, 0, 0, 0))
        stream.write(attr)
        stream.write(struct.pack('<QQ', 0, 0))


def export(args):
    args.unwind_cache.mkdir(parents=True, exist_ok=True)
    images = [Image(args.kernel)]
    seen = {hashlib.sha256(images[0].data).digest()}
    for directory in args.images:
        for image in discover(directory):
            digest = hashlib.sha256(image.data).digest()
            if image.entry and digest not in seen:
                seen.add(digest)
                images.append(image)
    identity_cache, unwinders = {}, {}
    counts, stops, coverage = collections.Counter(), collections.Counter(), collections.Counter()
    lags, samples = [], []
    begin, end = args.begin, args.end
    if begin is None and (args.capture / 'measurement.json').exists():
        measurement = json.loads((args.capture / 'measurement.json').read_text())
        begin, end = measurement['window_start_ns'], measurement['window_end_ns']
    begin = begin or 0
    end = end or MASK

    def unwinder(index):
        if index not in unwinders:
            unwinders[index] = Unwinder(images[index], args.unwind_cache)
        return unwinders[index]

    for path in sorted(args.capture.rglob('core-*-stacks.bin*')):
        core = int(path.name.split('-')[1])
        decoded = {}
        for requested, acquired, mode, ip, sp, bp, code, raw, recorded in read_samples(path):
            if not begin <= requested <= end:
                continue
            lags.append(acquired - requested)
            if mode == 0:
                image_index = 0
                chain, reason = unwinder(0).chain(ip, sp, bp, raw)
                # Frame pointers captured in the stopped guest can extend past
                # the bounded stack snapshot. Join only at an identical frame.
                valid = []
                for index, address in enumerate(recorded):
                    if images[0].read(address - (index > 0), 1) is None:
                        break
                    valid.append(address)
                chain = extend_chain(chain, valid)
                category = 'idle' if images[0].read(ip - 1, 1) == b'\xf4' else 'kernel'
            else:
                key = (ip, code)
                if key not in identity_cache:
                    matches = [i for i, image in enumerate(images) if i and image.read(ip, 32) == code]
                    if len(matches) > 1:
                        coverage['ambiguous image matches'] += 1
                    identity_cache[key] = matches[0] if len(matches) == 1 else None
                image_index = identity_cache[key]
                if image_index is None:
                    chain, reason, category = [ip], 'unresolved image', 'unresolved'
                else:
                    chain, reason = unwinder(image_index).chain(ip, sp, bp, raw)
                    category = 'user'
            stops[reason] += 1
            coverage[f'{category} depth {min(len(chain), 10)}'] += 1
            decoded[requested] = (image_index, chain, category)
        pulses = list(struct.iter_unpack('<QB', path.with_name(f'core-{core}-pulses.bin').read_bytes()))
        for i, (time, phase) in enumerate(pulses):
            if not begin <= time <= end:
                continue
            next_time = pulses[i + 1][0] if i + 1 < len(pulses) else time + 10_000_000
            weight = max(0, min(end, next_time) - time)
            if phase == 0:
                image_index, chain, category = None, [], 'host or off-CPU'
            else:
                image_index, chain, category = decoded.get(time, (None, [], 'missing sample'))
            if args.active_only and category in ['idle', 'host or off-CPU', 'missing sample']:
                continue
            counts[category] += weight
            samples.append((time, core, image_index, chain, weight, category))
    cores = sorted({sample[1] for sample in samples})
    write_perf(args.output, images, samples, cores)
    summary = {
        'event': 'arca-vcpu-wall-ns', 'samples': len(samples), 'weights_ns': dict(counts),
        'unwind_stops': dict(stops), 'coverage': dict(coverage),
        'images': [str(image.path.resolve()) for image in images],
        'used_images': [str(images[index].path.resolve()) for index in sorted(
            {sample[2] for sample in samples if sample[2] is not None})],
        'capture_lag_ns': {str(p): sorted(lags)[min(len(lags) - 1, int(len(lags) * p / 100))]
                           for p in [50, 95, 99, 100]} if lags else {},
    }
    args.output.with_suffix('.summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps({'samples': len(samples), 'weights_ns': dict(counts)}, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=pathlib.Path)
    parser.add_argument('output', type=pathlib.Path)
    parser.add_argument('--kernel', type=pathlib.Path, required=True)
    parser.add_argument('--images', type=pathlib.Path, action='append', default=[])
    parser.add_argument('--unwind-cache', type=pathlib.Path, required=True)
    parser.add_argument('--active-only', action='store_true', help='exclude halted and missing/host-state intervals')
    parser.add_argument('--begin', type=int, help='inclusive CLOCK_MONOTONIC timestamp in ns')
    parser.add_argument('--end', type=int, help='exclusive CLOCK_MONOTONIC timestamp in ns')
    export(parser.parse_args())
