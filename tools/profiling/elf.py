import mmap
import pathlib
import struct


class Image:
    def __init__(self, path):
        self.path = pathlib.Path(path)
        self.file = self.path.open('rb')
        self.data = mmap.mmap(self.file.fileno(), 0, access=mmap.ACCESS_READ)
        self.entry = struct.unpack_from('<Q', self.data, 24)[0]
        offset = struct.unpack_from('<Q', self.data, 32)[0]
        size, count = struct.unpack_from('<HH', self.data, 54)
        self.segments = []
        for index in range(count):
            kind, flags, file_offset, va, _, file_size, memory_size, _ = struct.unpack_from(
                '<II6Q', self.data, offset + index * size)
            if kind == 1:
                self.segments.append((va, memory_size, file_offset, file_size, flags))

    def read(self, address, length):
        for va, _, offset, size, flags in self.segments:
            if flags & 1 and va <= address and address + length <= va + size:
                begin = offset + address - va
                return self.data[begin:begin + length]
        return None


def discover(directory):
    for path in pathlib.Path(directory).rglob('*'):
        if path.is_file():
            try:
                with path.open('rb') as source:
                    header = source.read(6)
                if header == b'\x7fELF\x02\x01':
                    yield Image(path)
            except OSError:
                continue
