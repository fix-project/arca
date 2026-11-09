"""Conservative x86-64 CFA rules for frame-pointer and fixed-stack code."""
import collections
import json
import pathlib
import re
import subprocess
import sys


def advance(state, operation, operands):
    sp, bp = state
    if operation.startswith('push'):
        return (sp + 8 if sp is not None else None), bp
    if operation.startswith('pop'):
        return (None if operands == '%rsp' or sp is None else sp - 8), (None if operands == '%rbp' else bp)
    if operation.startswith('leave'):
        return (bp - 8 if bp is not None else None), None
    if operation.startswith('enter'):
        return None, None
    if operation.startswith(('cmp', 'test')):
        return sp, bp
    if operation.startswith(('xchg', 'xadd')):
        return (None if '%rsp' in operands else sp), (None if '%rbp' in operands else bp)
    destination = operands.rsplit(',', 1)[-1].strip()
    if destination not in ('%rsp', '%rbp', '%esp', '%ebp'):
        return sp, bp
    value = None
    immediate = re.fullmatch(r'\$(0x[0-9a-f]+|[0-9]+),\s*%rsp', operands)
    move = re.fullmatch(r'%(rsp|rbp),\s*%(rsp|rbp)', operands)
    address = re.fullmatch(r'(-?(?:0x[0-9a-f]+|[0-9]+))\(%(rsp|rbp)\),\s*%(rsp|rbp)', operands)
    if immediate and sp is not None:
        amount = int(immediate[1], 0)
        if operation.startswith('sub'):
            value = sp + amount
        elif operation.startswith('add'):
            value = sp - amount
    elif move and operation.startswith('mov'):
        value = sp if move[1] == 'rsp' else bp
    elif address and operation.startswith('lea'):
        origin = sp if address[2] == 'rsp' else bp
        value = origin - int(address[1], 0) if origin is not None else None
    if destination in ('%rsp', '%esp'):
        return value, bp
    return sp, value


def function_rows(name, instructions):
    indexes = {instruction[0]: i for i, instruction in enumerate(instructions)}
    states = {instructions[0][0]: (0, None)}
    pending = collections.deque([instructions[0][0]])
    while pending:
        ip = pending.popleft()
        index = indexes[ip]
        _, operation, operands = instructions[index]
        after = advance(states[ip], operation, operands)
        targets = []
        if operation.startswith('j'):
            target = re.match(r'(?:0x)?([0-9a-f]+)\b', operands)
            if target and int(target[1], 16) in indexes:
                targets.append(int(target[1], 16))
            falls_through = operation not in ('jmp', 'jmpq')
        else:
            falls_through = not operation.startswith(('ret', 'ud2', 'hlt', 'sysret', 'iret'))
        if falls_through and index + 1 < len(instructions):
            targets.append(instructions[index + 1][0])
        for target in targets:
            if target not in states:
                states[target] = after
                pending.append(target)
            else:
                merged = tuple(a if a == b else None for a, b in zip(states[target], after))
                if merged != states[target]:
                    states[target] = merged
                    pending.append(target)
    spans, previous = [], object()
    for ip, _, _ in instructions:
        state = states.get(ip, (None, None))
        if state != previous:
            spans.append([ip, *state])
            previous = state
    saved = None
    for ip, operation, operands in instructions[:20]:
        if operation.startswith(('call', 'j')):
            break
        sp, _ = states.get(ip, (None, None))
        if saved is None and operation.startswith('push') and operands == '%rbp' and sp is not None:
            saved = -(sp + 8)
    return [instructions[0][0], instructions[-1][0] + 16, name, spans, saved]


def build(image, output):
    process = subprocess.Popen(['llvm-objdump', '-d', '--no-show-raw-insn',
                                '--print-imm-hex', str(image)], stdout=subprocess.PIPE, text=True)
    result, instructions, name = [], [], ''
    for line in process.stdout:
        symbol = re.match(r'^([0-9a-f]+) <(.*)>:', line)
        if symbol:
            if instructions:
                result.append(function_rows(name, instructions))
            name, instructions = symbol[2], []
            continue
        instruction = re.match(r'^\s*([0-9a-f]+):\s+(\S+)(?:\s+(.*))?$', line)
        if instruction:
            operands = (instruction[3] or '').split('#', 1)[0].strip()
            instructions.append((int(instruction[1], 16), instruction[2], operands))
    if instructions:
        result.append(function_rows(name, instructions))
    if process.wait() != 0:
        raise RuntimeError('llvm-objdump failed')
    output.write_text(json.dumps(result, separators=(',', ':')))
    print(image, len(result), 'unwind entries', file=sys.stderr)


if __name__ == '__main__':
    build(pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]))
