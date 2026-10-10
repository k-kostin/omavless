"""Pure host transport constructor. No SSH, guest execution or filesystem I/O."""
import shlex


def create_delivery_command(source):
    """Return ONE remote-command argument for guest.sh's exec ssh ... "$@".

    OpenSSH joins remote argv with spaces before the remote shell parses it.
    Passing Python's argv separately therefore loses the protection around -c
    source. The operator supplies reviewed, bounded create-only Python source;
    this function only quotes it and never invokes the resulting command.
    """
    if type(source) is not str or not source or '\0' in source:
        raise ValueError('fixed_delivery_source_refused')
    try:
        raw = source.encode('utf-8')
    except UnicodeEncodeError:
        raise ValueError('fixed_delivery_source_refused') from None
    if len(raw) > 32768:
        raise ValueError('fixed_delivery_source_refused')
    return (shlex.join(('/usr/bin/python3', '-I', '-B', '-c', source)),)
