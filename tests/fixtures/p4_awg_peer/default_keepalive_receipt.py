"""Pure finite ordinary default keepalive receipt; no capture/effect authority."""
import json
import math
import re

PACKAGE = 'github.com/amnezia-vpn/amneziawg-go/v3/device'
SELECTORS = {'keepalive': 'TestP4DefaultSendKeepalive'}
OUTPUT_LIMIT = 2 * 1024 * 1024
EVENT_LIMIT = 64
LINE_LIMIT = 65536
NS = 1_000_000_000
TIMES = {'scheduled_ns', 'keepalive_ns', 'consume_ns', 'receive_ns', 'quiet_ns',
         'teardown_ns', 'body_ns'}
TRUE = {'baseline', 'default_keepalive', 'actual_zero_payload', 'actual_receive',
        'idle_cancelled', 'corrupt_refused', 'stable_keys', 'no_extra_h1',
        'quiet_no_tun', 'teardown_complete', 'defaults_unchanged'}
RECEIPT = re.compile(r'    default_send_keepalive_test\.go:[1-9][0-9]{0,5}: p4_keepalive_receipt (.+)\n\Z')


def require(value):
    if not value:
        raise ValueError('fixed_keepalive_receipt_refused')


def pairs(rows):
    value = {}
    for key, item in rows:
        require(key not in value)
        value[key] = item
    return value


def constant(_):
    raise ValueError('fixed_keepalive_receipt_refused')


def receipt(output, case):
    require(case == 'keepalive' and type(output) is str and len(output) <= 4096)
    match = RECEIPT.fullmatch(output)
    require(match is not None)
    parts = match[1].split(' ')
    require(all(re.fullmatch(r'[a-z][a-z0-9_]{0,31}=[a-z0-9]{1,16}', part) for part in parts))
    value = pairs(tuple(part.split('=', 1)) for part in parts)
    require(set(value) == TIMES | TRUE | {'case', 'network_fds'})
    require(value['case'] == case and value['network_fds'] == 'false')
    require(all(value[field] == 'true' for field in TRUE))
    for field in TIMES:
        require(re.fullmatch(r'(?:0|[1-9][0-9]{0,11})', value[field]) is not None)
        value[field] = int(value[field])
    require(value['scheduled_ns'] == 10 * NS)
    require(10 * NS <= value['keepalive_ns'] <= 13 * NS)
    require(0 <= value['consume_ns'] <= value['receive_ns'] <= 2 * NS)
    require(1.5 * NS <= value['quiet_ns'] <= 3 * NS)
    require(0 < value['teardown_ns'] <= 5 * NS and 10 * NS <= value['body_ns'] <= 30 * NS)
    require(value['keepalive_ns'] + value['receive_ns'] + value['quiet_ns'] + value['teardown_ns'] <= value['body_ns'])
    return value


def elapsed(value):
    require(type(value) in (int, float) and math.isfinite(value) and 0 < value <= 30)
    return value


def verify_events(raw, case):
    require(type(raw) is bytes and len(raw) <= OUTPUT_LIMIT and raw.endswith(b'\n'))
    require(type(case) is str and case in SELECTORS)
    lines = raw.split(b'\n')[:-1]
    require(1 <= len(lines) <= EVENT_LIMIT and all(0 < len(line) <= LINE_LIMIT
            and all(byte >= 32 for byte in line) for line in lines))
    selected = SELECTORS[case]
    # Closed eight-event grammar: no optional/missing/duplicated banners or
    # counters whose zero state can accidentally satisfy a final predicate.
    require(len(lines) == 8)
    actions = ('start', 'run', 'output', 'output', 'output', 'pass', 'output', 'pass')
    observed = None
    case_elapsed = None
    banner_elapsed = None
    for position, line in enumerate(lines):
        try:
            event = json.loads(line.decode('utf-8', 'strict'), object_pairs_hook=pairs, parse_constant=constant)
        except (ValueError, UnicodeError, RecursionError):
            raise ValueError('fixed_keepalive_receipt_refused') from None
        require(type(event) is dict and event.get('Package') == PACKAGE)
        require(set(event) <= {'Time', 'Action', 'Package', 'Test', 'Elapsed', 'Output'})
        if 'Time' in event:
            require(type(event['Time']) is str and len(event['Time']) <= 64)
        require(event.get('Action') == actions[position])
        fields = {'Action', 'Package'}
        if position in (1, 2, 3, 4, 5):
            fields.add('Test')
            require(event.get('Test') == selected)
        if position in (2, 3, 4, 6):
            fields.add('Output')
            require(type(event.get('Output')) is str and len(event['Output']) <= 4096)
        if position in (5, 7):
            fields.add('Elapsed')
        require(set(event) - {'Time'} == fields)
        if position == 2:
            require(event['Output'] == '=== RUN   ' + selected + '\n')
        elif position == 3:
            observed = receipt(event['Output'], case)
        elif position == 4:
            match = re.fullmatch(r'--- PASS: ' + selected +
                                 r' \(([0-9]{1,3}(?:\.[0-9]{1,3})?)s\)\n', event['Output'])
            require(match is not None)
            banner_elapsed = elapsed(float(match[1]))
            require(banner_elapsed + .01 >= observed['body_ns'] / NS)
        elif position == 5:
            case_elapsed = elapsed(event.get('Elapsed'))
            # Go JSON elapsed is rounded; <=10ms tolerance only for rounding.
            # The source receipt itself is sampled AFTER both actual teardowns.
            require(case_elapsed + .01 >= observed['body_ns'] / NS)
            require(abs(banner_elapsed - case_elapsed) <= .01)
        elif position == 6:
            require(event['Output'] == 'PASS\n')
        elif position == 7:
            require(elapsed(event['Elapsed']) + .01 >= case_elapsed)
    return observed
