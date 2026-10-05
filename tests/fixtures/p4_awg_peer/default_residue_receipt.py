"""Inert finite P4 channel receipt contract; no runner, build or effect authority.

Only settled known-zero output from a separately reviewed one-shot artifact may
later be supplied. Parsing synthetic events here proves no engine execution.
"""
import json
import math
import re

PACKAGE = 'github.com/amnezia-vpn/amneziawg-go/v3/device'
SELECTORS = {'reject': 'TestP4DefaultElapsedRejectAndResidue',
             'exhaust': 'TestP4DefaultElapsedExhaustionKeepsResidue'}
OUTPUT_LIMIT = 2 * 1024 * 1024
EVENT_LIMIT = 64
LINE_LIMIT = 65536
NS = 1_000_000_000
COMMON_TIMES = {'quiet_ns', 'client_zero_age_ns', 'server_zero_age_ns',
                'client_removed_age_ns', 'server_removed_age_ns', 'teardown_ns', 'body_ns'}
COMMON_TRUE = {'baseline', 'keys_retained_539', 'previous_next_empty', 'removal',
               'teardown_complete', 'defaults_unchanged'}
TIMES = {'reject': COMMON_TIMES | {'early_age_ns', 'late_age_ns'},
         'exhaust': COMMON_TIMES | {'trigger_age_ns', 'exhaustion_ns', 'retry_min_ns',
                                   'retry_max_ns', 'target_min_ns', 'target_max_ns'}}
TRUE = {'reject': COMMON_TRUE | {'original_packet', 'next_receive_entry', 'expired_no_tun'},
        'exhaust': COMMON_TRUE | {'genuine_staged', 'real_timer_chain', 'partial_retained',
                                  'prearmed_preserved'}}
COUNTS = {'reject': {}, 'exhaust': {'chain_h1': 20, 'retries': 19, 'expirations': 20, 'attempts': 19}}
RECEIPT = re.compile(r'    default_residue_cases_test\.go:[1-9][0-9]{0,5}: p4_residue_receipt (.+)\n\Z')


def require(value):
    if not value:
        raise ValueError('fixed_residue_receipt_refused')


def pairs(rows):
    value = {}
    for key, item in rows:
        require(key not in value)
        value[key] = item
    return value


def constant(_):
    raise ValueError('fixed_residue_receipt_refused')


def receipt(output, case):
    require(type(case) is str and case in SELECTORS and type(output) is str and len(output) <= 4096)
    match = RECEIPT.fullmatch(output)
    require(match is not None)
    parts = match[1].split(' ')
    require(all(re.fullmatch(r'[a-z][a-z0-9_]{0,31}=[a-z0-9]{1,16}', part) for part in parts))
    value = pairs(tuple(part.split('=', 1)) for part in parts)
    require(set(value) == TIMES[case] | TRUE[case] | set(COUNTS[case]) | {'case', 'network_fds'})
    require(value['case'] == case and value['network_fds'] == 'false')
    require(all(value[field] == 'true' for field in TRUE[case]))
    for field in TIMES[case] | set(COUNTS[case]):
        require(re.fullmatch(r'[1-9][0-9]{0,11}', value[field]) is not None)
        value[field] = int(value[field])
    require(all(value[field] == expected for field, expected in COUNTS[case].items()))
    require(0 < value['teardown_ns'] <= 5 * NS and 540 * NS <= value['body_ns'] <= 570 * NS)
    for side in ('client', 'server'):
        zero, removed = value[side + '_zero_age_ns'], value[side + '_removed_age_ns']
        require(540 * NS <= zero <= removed <= 550 * NS and removed <= value['body_ns'])
    if case == 'reject':
        require(177 * NS <= value['early_age_ns'] < 179 * NS)
        require(181 * NS <= value['late_age_ns'] < 183 * NS)
        require(1.5 * NS <= value['quiet_ns'] <= 3 * NS)
    else:
        require(181 * NS <= value['trigger_age_ns'] < 184 * NS)
        require(100 * NS <= value['exhaustion_ns'] <= 130 * NS)
        require(5 * NS <= value['retry_min_ns'] <= value['retry_max_ns'] <= 8 * NS)
        require(5 * NS <= value['target_min_ns'] <= value['target_max_ns'] <= 5 * NS + 333_000_000)
        require(5.5 * NS <= value['quiet_ns'] <= 6 * NS)
        require(value['trigger_age_ns'] + value['exhaustion_ns'] + value['quiet_ns'] < value['client_zero_age_ns'])
    return value


def elapsed(value):
    require(type(value) in (int, float) and math.isfinite(value) and 0 < value <= 570)
    return value


def verify_events(raw, case):
    require(type(raw) is bytes and len(raw) <= OUTPUT_LIMIT and raw.endswith(b'\n'))
    require(type(case) is str and case in SELECTORS)
    lines = raw.splitlines()
    require(1 <= len(lines) <= EVENT_LIMIT and all(0 < len(line) <= LINE_LIMIT for line in lines))
    selected = SELECTORS[case]
    run = case_pass = package_pass = starts = 0
    observed = None
    case_elapsed = None
    for line in lines:
        try:
            event = json.loads(line.decode('utf-8', 'strict'), object_pairs_hook=pairs, parse_constant=constant)
        except (ValueError, UnicodeError, RecursionError):
            raise ValueError('fixed_residue_receipt_refused') from None
        require(type(event) is dict and event.get('Package') == PACKAGE)
        require(set(event) <= {'Time', 'Action', 'Package', 'Test', 'Elapsed', 'Output'})
        if 'Time' in event:
            require(type(event['Time']) is str and len(event['Time']) <= 64)
        action = event.get('Action')
        require(type(action) is str and action in ('start', 'run', 'output', 'pass'))
        if action == 'start':
            require(starts == run == case_pass == package_pass == 0 and set(event) <= {'Time', 'Action', 'Package'})
            starts += 1
        elif action == 'run':
            require(event.get('Test') == selected and run == case_pass == package_pass == 0)
            require(set(event) <= {'Time', 'Action', 'Package', 'Test'})
            run += 1
        elif action == 'output':
            require('Elapsed' not in event and type(event.get('Output')) is str and len(event['Output']) <= 4096)
            output = event['Output']
            if 'Test' in event:
                require(event['Test'] == selected and run == 1 and case_pass == package_pass == 0)
                if 'p4_residue_receipt' in output:
                    require(observed is None)
                    observed = receipt(output, case)
                else:
                    require(output == '=== RUN   ' + selected + '\n' or re.fullmatch(
                        r'--- PASS: ' + selected + r' \([0-9]{1,3}(?:\.[0-9]{1,3})?s\)\n', output))
            else:
                require(case_pass == 1 and package_pass == 0 and output == 'PASS\n')
        elif 'Test' in event:
            require(event['Test'] == selected and run == 1 and case_pass == package_pass == 0 and observed is not None)
            require('Output' not in event)
            case_elapsed = elapsed(event.get('Elapsed'))
            # Go JSON elapsed is rounded; <=10ms tolerance only for rounding.
            # The source receipt itself is sampled AFTER both actual teardowns.
            require(case_elapsed + .01 >= observed['body_ns'] / NS)
            case_pass += 1
        else:
            require(case_pass == 1 and package_pass == 0 and 'Output' not in event)
            if 'Elapsed' in event:
                require(elapsed(event['Elapsed']) + .01 >= case_elapsed)
            package_pass += 1
    require(run == case_pass == package_pass == 1 and observed is not None)
    return observed
