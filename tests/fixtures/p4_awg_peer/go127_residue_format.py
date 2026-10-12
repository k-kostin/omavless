"""Pure exact Go 1.27 frame adapter for the frozen eight-event P4 grammar.

No CLI, capture access, execution, receipt issuance or effect authority. Raw
captures stay raw; only three validated frame fields are removed from a new
in-memory encoding supplied to the unchanged strict residue parser.
"""
import json

FORMAT = 'go1.27-residue-eight-events-frame-2-4-6-v1'
FRAME_POSITIONS = frozenset((2, 4, 6))
OUTPUT_LIMIT = 2 * 1024 * 1024
LINE_LIMIT = 65536
FIELDS = frozenset(('Time', 'Action', 'Package', 'Test', 'Elapsed', 'Output', 'OutputType'))


def require(value):
    if not value:
        raise ValueError('fixed_go127_residue_format_refused')


def pairs(rows):
    value = {}
    for key, item in rows:
        require(key not in value)
        value[key] = item
    return value


def constant(_):
    raise ValueError('fixed_go127_residue_format_refused')


def normalize(raw):
    require(type(raw) is bytes and len(raw) <= OUTPUT_LIMIT and raw.endswith(b'\n'))
    lines = raw.split(b'\n')[:-1]
    require(len(lines) == 8 and all(0 < len(line) <= LINE_LIMIT
            and all(byte >= 32 for byte in line) for line in lines))
    normalized = []
    for position, line in enumerate(lines):
        try:
            event = json.loads(line.decode('utf-8', 'strict'), object_pairs_hook=pairs, parse_constant=constant)
        except (ValueError, UnicodeError, RecursionError):
            raise ValueError('fixed_go127_residue_format_refused') from None
        require(type(event) is dict and set(event) <= FIELDS)
        if position in FRAME_POSITIONS:
            require(type(event.get('OutputType')) is str and event['OutputType'] == 'frame')
            del event['OutputType']  # Only this exact validated field is removed.
        else:
            require('OutputType' not in event)
        normalized.append(json.dumps(event, separators=(',', ':'), ensure_ascii=False,
                                     allow_nan=False).encode('utf-8') + b'\n')
    result = b''.join(normalized)
    require(len(result) <= OUTPUT_LIMIT)
    return result
