// SPDX-License-Identifier: MIT
// Private developer fixture. No resolver replacement or settings writes.
const {Gio, GLib, GObject} = imports.gi;
const ByteArray = imports.byteArray;
let phase = 'input';
try {
    const input = new Gio.UnixInputStream({fd: 0, close_fd: false});
    const bytes = input.read_bytes(8193, null).toArray();
    if (bytes.length > 8192) throw new Error();
    const request = JSON.parse(ByteArray.toString(bytes));
    const keys = ['target', 'proxy', 'scheme', 'environment', 'challenge', 'send'];
    if (Object.keys(request).sort().join(',') !== keys.sort().join(',')) throw new Error();
    for (const port of [request.target, request.proxy]) {
        if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error();
    }
    if (!['none', 'http', 'socks5'].includes(request.scheme) || typeof request.send !== 'boolean'
        || !/^[a-z0-9-]{1,80}$/.test(request.challenge)) throw new Error();
    const selected = ['http_proxy', 'HTTP_PROXY', 'https_proxy', 'HTTPS_PROXY', 'ftp_proxy', 'FTP_PROXY',
        'all_proxy', 'ALL_PROXY', 'no_proxy', 'NO_PROXY'];
    if (Object.keys(request.environment).sort().join(',') !== selected.sort().join(',')) throw new Error();
    phase = 'environment';
    for (const key of selected) {
        const expected = request.environment[key];
        if (expected !== null && typeof expected !== 'string') throw new Error();
        if (GLib.getenv(key) !== expected) throw new Error();
    }
    phase = 'resolver';
    const resolver = Gio.ProxyResolver.get_default();
    const name = GObject.type_name(resolver.constructor.$gtype);
    if (!resolver.is_supported() || name !== 'GLibproxyResolver') throw new Error();
    const uri = `http://127.0.0.1:${request.target}/`;
    const proxies = resolver.lookup(uri, null);
    const expected = request.scheme === 'none' ? 'direct://' : `${request.scheme}://127.0.0.1:${request.proxy}`;
    if (proxies.length !== 1 || proxies[0] !== expected) throw new Error();
    if (request.send) {
        phase = 'connection';
        const client = new Gio.SocketClient({timeout: 2});
        if (!client.get_enable_proxy() || client.get_proxy_resolver() !== resolver) throw new Error();
        phase = 'dial';
        const connection = client.connect_to_uri(uri, request.target, null);
        try {
            const remote = connection.get_remote_address();
            if (remote.get_address().to_string() !== '127.0.0.1'
                || remote.get_port() !== (request.scheme === 'none' ? request.target : request.proxy)) throw new Error();
            phase = 'http';
            const payload = `GET /${request.challenge} HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n`;
            connection.get_output_stream().write_all(ByteArray.fromString(payload), null);
            let response = '';
            while (true) {
                const data = connection.get_input_stream().read_bytes(4097, null).toArray();
                if (data.length === 0) break;
                response += ByteArray.toString(data);
                if (response.length > 4096) throw new Error();
            }
            const expectedResponse = `HTTP/1.0 200 OK\r\nContent-Length: ${request.challenge.length}\r\nConnection: close\r\n\r\n${request.challenge}`;
            if (response !== expectedResponse) throw new Error();
        } finally {
            connection.close(null);
        }
    }
    print(JSON.stringify({ok: true, resolver: name, selection: request.scheme, sent: request.send}));
} catch (_) {
    // GError/input may contain private environment or ephemeral URLs. Never emit it.
    print(JSON.stringify({ok: false, phase}));
}
