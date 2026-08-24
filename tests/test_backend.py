import importlib.util
import io
import http.server
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import threading
import time
import urllib.parse
import urllib.error
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("omavless_backend", ROOT / "backend.py")
backend = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = backend
SPEC.loader.exec_module(backend)

REALITY_URI = (
    "vless://11111111-1111-4111-8111-111111111111@example.com:443"
    "?type=tcp&security=reality&encryption=none&flow=xtls-rprx-vision"
    "&sni=example.org&fp=firefox"
    "&pbk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    "&sid=0123456789abcdef&spx=%2F#Example"
)


class BackendTests(unittest.TestCase):
    def make_env(self, home: Path, systemctl_body: str = "#!/bin/sh\nexit 3\n"):
        runtime = home / "runtime"
        runtime.mkdir(mode=0o700)
        fake_systemctl = home / "systemctl"
        fake_systemctl.write_text(systemctl_body, encoding="utf-8")
        fake_systemctl.chmod(0o755)
        env = os.environ.copy()
        env.update({
            "OMAVLESS_HOME": str(home),
            "XDG_RUNTIME_DIR": str(runtime),
            "OMAVLESS_SYSTEMCTL": str(fake_systemctl),
        })
        return env, runtime

    def paths_for(self, home: Path, runtime: Path | None = None):
        runtime = runtime or home / "runtime"
        config_dir = home / ".config" / "omavless"
        return backend.Paths(
            home, config_dir, config_dir / "profiles.json",
            config_dir / "route-template.yaml", config_dir / "config.yaml",
            home / ".config" / "systemd" / "user" / backend.SERVICE,
            home / ".config" / "omarchy" / "omavless",
            home / ".local" / "state" / "omarchy" / "vless-last", runtime,
        )

    def test_manifest_matches_plugin_entrypoint_and_release_version(self):
        manifest = json.loads((ROOT / "manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["schemaVersion"], 1)
        self.assertEqual(manifest["id"], "kdk.omavless")
        self.assertEqual(manifest["name"], "OmaVLESS")
        self.assertEqual(manifest["barWidget"]["displayName"], "OmaVLESS")
        self.assertEqual(manifest["version"], "0.7.0")
        self.assertEqual(backend.PLUGIN_VERSION, manifest["version"])
        self.assertEqual(backend.USER_AGENT, "OmaVLESS/0.7.0")
        self.assertEqual(manifest["entryPoints"]["barWidget"], "Panel.qml")
        panel = (ROOT / "Panel.qml").read_text(encoding="utf-8")
        self.assertIn('moduleName: "kdk.omavless"', panel)

    def test_pointer_hover_never_scrolls_profile_lists(self):
        panel = (ROOT / "Panel.qml").read_text(encoding="utf-8")
        subscription_hover = panel.split(
            "function setSubscriptionCursor(index) {", 1
        )[1].split("function scrollSubscriptionCursorIntoView()", 1)[0]
        config_hover = panel.split(
            "function setConfigCursor(index) {", 1
        )[1].split("function selectedRow()", 1)[0]
        self.assertNotIn("scrollSubscriptionCursorIntoView()", subscription_hover)
        self.assertNotIn("scrollCursorIntoView()", config_hover)
        self.assertIn("if (!pointerSelectingConfig) scrollCursorIntoView()", panel)

    def test_reality_uri_maps_to_mihomo(self):
        parsed = backend.parse_vless(REALITY_URI)
        self.assertEqual(parsed["network"], "tcp")
        self.assertEqual(parsed["security"], "reality")
        self.assertEqual(parsed["spider_x"], "/")
        yaml = backend.proxy_yaml({"name": "Example", "uri": REALITY_URI})
        for expected in (
            "type: vless", "flow: \"xtls-rprx-vision\"", "tls: true",
            "client-fingerprint: \"firefox\"", "reality-opts:",
            "short-id: \"0123456789abcdef\"",
        ):
            self.assertIn(expected, yaml)
        self.assertNotIn("spider-x:", yaml)

    def test_vless_preview_is_useful_without_returning_reusable_secrets(self):
        preview = backend.preview_vless(REALITY_URI)
        self.assertEqual(preview["server"], "example.com")
        self.assertEqual(preview["port"], 443)
        self.assertEqual(preview["transport"], "tcp")
        self.assertEqual(preview["security"], "reality")
        self.assertEqual(preview["sni"], "example.org")
        self.assertFalse(preview["insecure"])
        self.assertEqual(preview["credentialHint"], "••••1111")
        self.assertEqual(preview["suggestedName"], "Example")
        self.assertEqual(
            preview["compatibilityNote"],
            backend.REALITY_SPX_COMPATIBILITY_NOTE,
        )
        encoded = json.dumps(preview, ensure_ascii=False)
        self.assertNotIn("11111111-1111-4111-8111-111111111111", encoded)
        self.assertNotIn("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", encoded)
        self.assertNotIn("vless://", encoded)
        self.assertNotIn("spx", encoded.lower().replace("(spx)", ""))

    def test_reality_fields_match_current_mihomo_schema(self):
        base, fragment = REALITY_URI.split("#", 1)
        without_spx = base.replace("&spx=%2F", "") + "#" + fragment
        self.assertEqual(backend.preview_vless(without_spx)["compatibilityNote"], "")

        bad_public_key = REALITY_URI.replace(
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "not-a-public-key"
        )
        with self.assertRaisesRegex(backend.BackendError, "public key"):
            backend.parse_vless(bad_public_key)
        with self.assertRaisesRegex(backend.BackendError, "short ID"):
            backend.parse_vless(REALITY_URI.replace("0123456789abcdef", "abc"))
        with self.assertRaisesRegex(backend.BackendError, "short ID"):
            backend.parse_vless(REALITY_URI.replace("0123456789abcdef", "not-hex"))

        unsupported = base + "&mldsa65Verify=secret-verifier#" + fragment
        with self.assertRaisesRegex(backend.BackendError, "not supported by Mihomo") as failure:
            backend.parse_vless(unsupported)
        self.assertNotIn("secret-verifier", str(failure.exception))

    def test_ws_transport_maps_options(self):
        uri = (
            "vless://11111111-1111-4111-8111-111111111111@example.com:443"
            "?type=ws&security=tls&sni=example.com&host=cdn.example.com&path=%2Fedge"
        )
        yaml = backend.proxy_yaml({"name": "WS", "uri": uri})
        self.assertIn("network: ws", yaml)
        self.assertIn("ws-opts:", yaml)
        self.assertIn("path: \"/edge\"", yaml)
        self.assertIn("Host: \"cdn.example.com\"", yaml)

    def test_vless_vision_udp443_variant_maps_to_mihomo_flow(self):
        uri = REALITY_URI.replace(
            "flow=xtls-rprx-vision", "flow=xtls-rprx-vision-udp443"
        )
        parsed = backend.parse_vless(uri)
        self.assertEqual(parsed["flow"], "xtls-rprx-vision-udp443")
        self.assertEqual(parsed["mihomo_flow"], "xtls-rprx-vision")
        yaml = backend.proxy_yaml({"name": "Vision UDP443", "uri": uri})
        self.assertIn('flow: "xtls-rprx-vision"', yaml)
        self.assertNotIn("xtls-rprx-vision-udp443", yaml)

    def test_vless_packet_encoding_is_bounded_to_mihomo_values(self):
        base, fragment = REALITY_URI.split("#", 1)
        for value in ("xudp", "packetaddr"):
            uri = f"{base}&packetEncoding={value}#{fragment}"
            parsed = backend.parse_vless(uri)
            self.assertEqual(parsed["packet_encoding"], value)
            self.assertIn(
                f'packet-encoding: "{value}"',
                backend.proxy_yaml({"name": value, "uri": uri}),
            )
        with self.assertRaisesRegex(backend.BackendError, "packet encoding"):
            backend.parse_vless(f"{base}&packetEncoding=made-up#{fragment}")

    def test_vless_xhttp_mode_is_normalized_and_strict(self):
        prefix = (
            "vless://11111111-1111-4111-8111-111111111111@example.com:443"
            "?type=xhttp&security=tls&sni=example.com&path=%2Fedge"
        )
        for value in ("auto", "stream-one", "stream-up", "packet-up"):
            uri = f"{prefix}&mode={value}#XHTTP"
            self.assertEqual(backend.parse_vless(uri)["mode"], value)
            self.assertIn(
                f'mode: "{value}"',
                backend.proxy_yaml({"name": value, "uri": uri}),
            )
        self.assertEqual(
            backend.parse_vless(f"{prefix}&mode=STREAM-UP#XHTTP")["mode"],
            "stream-up",
        )
        with self.assertRaisesRegex(backend.BackendError, "XHTTP mode"):
            backend.parse_vless(f"{prefix}&mode=made-up#XHTTP")

    def test_bounded_xhttp_extra_maps_only_current_mihomo_fields(self):
        extra = {
            "headers": {"X-Trace": "edge", "X-Secret": "private-token"},
            "xPaddingBytes": "100-1000",
            "xPaddingObfsMode": True,
            "xPaddingKey": "padding",
            "xPaddingHeader": "Referer",
            "xPaddingPlacement": "queryInHeader",
            "xPaddingMethod": "tokenish",
            "uplinkHTTPMethod": "put",
            "sessionPlacement": "cookie",
            "sessionKey": "session_id",
            "sessionTable": "Base62",
            "sessionLength": "16-32",
            "seqPlacement": "header",
            "seqKey": "X-Seq",
            "uplinkDataPlacement": "header",
            "uplinkDataKey": "X-Data",
            "uplinkChunkSize": 4096,
            "noGRPCHeader": True,
            "scMaxEachPostBytes": 1000000,
            "scMinPostsIntervalMs": "15-30",
            "xmux": {
                "maxConcurrency": "16-32",
                "maxConnections": 0,
                "cMaxReuseTimes": 96,
                "hMaxRequestTimes": "400-600",
                "hMaxReusableSecs": 1800,
                "hKeepAlivePeriod": 0,
            },
        }
        encoded_extra = urllib.parse.quote(json.dumps(extra, separators=(",", ":")))
        uri = (
            "vless://11111111-1111-4111-8111-111111111111@example.com:443"
            "?type=xhttp&security=tls&sni=example.com&path=%2Fedge"
            f"&mode=packet-up&extra={encoded_extra}#Advanced"
        )
        node = backend.parse_vless(uri)
        self.assertTrue(node["xhttp_extra"])
        yaml = backend.proxy_yaml({"name": "Advanced", "uri": uri})
        for expected in (
            '"X-Trace": "edge"', '"X-Secret": "private-token"',
            'x-padding-bytes: "100-1000"', "x-padding-obfs-mode: true",
            'uplink-http-method: "PUT"', 'session-placement: "cookie"',
            'session-length: "16-32"', 'uplink-chunk-size: "4096"',
            "no-grpc-header: true", 'sc-min-posts-interval-ms: "15-30"',
            "reuse-settings:", 'max-concurrency: "16-32"',
            'c-max-reuse-times: "96"', "h-keep-alive-period: 0",
        ):
            self.assertIn(expected, yaml)
        preview = backend.preview_vless(uri)
        self.assertTrue(preview["advancedXhttp"])
        public = json.dumps(preview, ensure_ascii=False)
        self.assertNotIn("private-token", public)
        self.assertNotIn("X-Secret", public)

    def test_xhttp_extra_accepts_benign_full_xray_defaults(self):
        extra = {
            "host": "ignored.example",
            "path": "/ignored",
            "mode": "stream-one",
            "headers": None,
            "xPaddingBytes": "100-1000",
            "noSSEHeader": False,
            "scMaxBufferedPosts": 0,
            "scStreamUpServerSecs": "0",
            "serverMaxHeaderBytes": 0,
            "xmux": {
                "maxConcurrency": "0", "maxConnections": "0",
                "cMaxReuseTimes": "0", "hMaxRequestTimes": "0",
                "hMaxReusableSecs": "0", "hKeepAlivePeriod": 0,
            },
            "downloadSettings": None,
            "extra": None,
        }
        uri = (
            "vless://11111111-1111-4111-8111-111111111111@example.com:443"
            "?type=xhttp&security=tls&sni=example.com&path=%2Fquery-wins&mode=auto"
            "&extra=" + urllib.parse.quote(json.dumps(extra)) + "#Defaults"
        )
        yaml = backend.proxy_yaml({"name": "Defaults", "uri": uri})
        self.assertIn('path: "/query-wins"', yaml)
        self.assertIn('mode: "auto"', yaml)
        self.assertNotIn("ignored.example", yaml)
        self.assertNotIn("reuse-settings:", yaml)

    def test_xhttp_download_settings_map_second_tls_endpoint(self):
        extra = {
            "xPaddingBytes": "100-1000",
            "downloadSettings": {
                "address": "download.example.com",
                "port": 8443,
                "network": "xhttp",
                "security": "tls",
                "tlsSettings": {
                    "serverName": "download-sni.example.com",
                    "alpn": ["h2", "http/1.1"],
                    "fingerprint": "chrome",
                    "allowInsecure": False,
                },
                "xhttpSettings": {
                    "path": "/down",
                    "host": "download-host.example.com",
                    "mode": "stream-up",
                    "headers": {"X-Download": "private-download-value"},
                    "extra": {
                        "xmux": {
                            "maxConnections": 4,
                            "hMaxRequestTimes": "600-900",
                            "hKeepAlivePeriod": 0,
                        }
                    },
                },
                "sockopt": {},
            },
        }
        uri = (
            "vless://11111111-1111-4111-8111-111111111111@upload.example.com:443"
            "?type=xhttp&security=tls&sni=upload.example.com&path=%2Fup&mode=stream-up"
            "&extra=" + urllib.parse.quote(json.dumps(extra, separators=(",", ":")))
            + "#Split"
        )
        yaml = backend.proxy_yaml({"name": "Split", "uri": uri})
        for expected in (
            "download-settings:", 'server: "download.example.com"', "port: 8443",
            "tls: true", 'servername: "download-sni.example.com"',
            'alpn: ["h2", "http/1.1"]', 'client-fingerprint: "chrome"',
            "skip-cert-verify: false", 'path: "/down"',
            'host: "download-host.example.com"',
            '"X-Download": "private-download-value"',
            "reuse-settings:", 'max-connections: "4"',
        ):
            self.assertIn(expected, yaml)
        preview = backend.preview_vless(uri)
        self.assertTrue(preview["advancedXhttp"])
        public = json.dumps(preview, ensure_ascii=False)
        self.assertNotIn("download.example.com", public)
        self.assertNotIn("private-download-value", public)

    def test_xhttp_download_reality_is_strict_and_spx_is_only_explained(self):
        extra = {
            "downloadSettings": {
                "address": "203.0.113.7",
                "port": 443,
                "network": "xhttp",
                "security": "reality",
                "realitySettings": {
                    "publicKey": "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8",
                    "shortId": "a1b2c3d4",
                    "serverName": "reality-download.example.com",
                    "fingerprint": "firefox",
                    "spiderX": "/private-spider-path",
                },
                "xhttpSettings": {"path": "/down", "mode": "packet-up"},
            }
        }
        uri = (
            "vless://11111111-1111-4111-8111-111111111111@upload.example.com:443"
            "?type=xhttp&security=tls&sni=upload.example.com&mode=packet-up&extra="
            + urllib.parse.quote(json.dumps(extra, separators=(",", ":"))) + "#Reality-down"
        )
        yaml = backend.proxy_yaml({"name": "Reality down", "uri": uri})
        self.assertIn("download-settings:", yaml)
        self.assertIn("reality-opts:", yaml)
        self.assertIn(
            'public-key: "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8"', yaml
        )
        self.assertIn('short-id: "a1b2c3d4"', yaml)
        self.assertNotIn("spider-x", yaml)
        preview = backend.preview_vless(uri)
        self.assertEqual(
            preview["compatibilityNote"], backend.REALITY_SPX_COMPATIBILITY_NOTE
        )
        public = json.dumps(preview, ensure_ascii=False)
        self.assertNotIn("private-spider-path", public)
        self.assertNotIn("AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8", public)

    def test_xhttp_download_rejects_unrepresentable_nested_policy(self):
        prefix = (
            "vless://11111111-1111-4111-8111-111111111111@upload.example.com:443"
            "?type=xhttp&security=tls&sni=upload.example.com&mode=stream-up&extra="
        )

        def uri_for(download: dict[str, object], *, mode: str = "stream-up") -> str:
            base = prefix.replace("mode=stream-up", f"mode={mode}")
            return base + urllib.parse.quote(json.dumps({"downloadSettings": download})) + "#Bad"

        cases = (
            ({"network": "grpc"}, "network must be xhttp"),
            ({"address": "bad host/name"}, "address has an invalid format"),
            ({"port": 70000}, "port is invalid"),
            ({"sockopt": {"dialerProxy": "secret"}}, "sockopt is not imported"),
            ({"security": "none", "tlsSettings": {"serverName": "example.com"}},
             "conflict with security none"),
            ({"security": "tls", "tlsSettings": {"certificate": "secret"}},
             "tlsSettings contains unsupported"),
            ({"xhttpSettings": {"mode": "packet-up"}}, "modes must match"),
            ({"xhttpSettings": {"extra": {"xPaddingBytes": "10-20"}}},
             "cannot be overridden independently"),
            ({"xhttpSettings": {"headers": {"X-A": "one"},
                                "extra": {"headers": {"X-A": "two"}}}},
             "headers conflict"),
            ({"security": "reality", "realitySettings": {
                "publicKey": "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8",
                "mldsa65Verify": "private-verifier",
            }}, "ML-DSA verification"),
        )
        for download, error in cases:
            with self.subTest(error=error), self.assertRaisesRegex(backend.BackendError, error):
                backend.parse_vless(uri_for(download))
        with self.assertRaisesRegex(backend.BackendError, "stream-one"):
            backend.parse_vless(uri_for({}, mode="stream-one"))

    def test_xhttp_extra_rejects_ambiguous_or_unbounded_input(self):
        prefix = (
            "vless://11111111-1111-4111-8111-111111111111@example.com:443"
            "?type=xhttp&security=tls&sni=example.com&mode=packet-up&extra="
        )

        def uri_for(raw: str) -> str:
            return prefix + urllib.parse.quote(raw) + "#Bad"

        cases = (
            ('{"unknown":"secret"}', "unsupported fields"),
            ('{"xPaddingBytes":"1","xPaddingBytes":"2"}', "duplicate fields"),
            (json.dumps({"headers": {"Host": "override.example"}}), "header name"),
            (json.dumps({"headers": {"X-Test": "ok\r\nInjected: yes"}}), "header value"),
            (json.dumps({"extra": {"extra": {}}}), "recursive"),
            (json.dumps({"noSSEHeader": True}), "server-only"),
            (json.dumps({"xmux": {"maxConcurrency": 2, "maxConnections": 3}}),
             "mutually exclusive"),
            (json.dumps({"sessionPlacement": "path", "seqPlacement": "header"}),
             "seq placement"),
        )
        for raw, error in cases:
            with self.subTest(error=error), self.assertRaisesRegex(backend.BackendError, error):
                backend.parse_vless(uri_for(raw))

        deep: dict[str, object] = {}
        cursor = deep
        for _ in range(backend.MAX_XHTTP_EXTRA_DEPTH + 1):
            child: dict[str, object] = {}
            cursor["nested"] = child
            cursor = child
        with self.assertRaisesRegex(backend.BackendError, "nested too deeply"):
            backend.parse_vless(uri_for(json.dumps(deep)))

        oversized = json.dumps({"headers": {"X-Test": "x" * 1100}})
        with self.assertRaisesRegex(backend.BackendError, "header value"):
            backend.parse_vless(uri_for(oversized))

        non_xhttp = REALITY_URI.replace(
            "#Example", "&extra=" + urllib.parse.quote('{"xPaddingBytes":"100-1000"}') + "#Example"
        )
        with self.assertRaisesRegex(backend.BackendError, "requires the XHTTP transport"):
            backend.parse_vless(non_xhttp)

    def test_legacy_store_with_unsupported_xhttp_extra_remains_readable(self):
        raw_extra = urllib.parse.quote(json.dumps({"futureField": "private-value"}))
        uri = (
            "vless://11111111-1111-4111-8111-111111111111@example.com:443"
            "?type=xhttp&security=tls&sni=example.com&mode=auto"
            f"&extra={raw_extra}#Legacy"
        )
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            paths.store.write_text(json.dumps({
                "version": 1, "activeId": "", "lastId": profile_id,
                "profiles": [{"id": profile_id, "name": "Legacy", "uri": uri}],
            }), encoding="utf-8")
            loaded = backend.load_store(paths)
            self.assertEqual(loaded["profiles"][0]["name"], "Legacy")
            with self.assertRaisesRegex(backend.BackendError, "unsupported fields"):
                backend.proxy_yaml(loaded["profiles"][0])

    def test_non_xhttp_mode_query_cannot_leak_into_generated_yaml(self):
        base, fragment = REALITY_URI.split("#", 1)
        parsed = backend.parse_vless(f"{base}&mode=provider-metadata#{fragment}")
        self.assertEqual(parsed["mode"], "")
        self.assertNotIn(
            "mode:",
            backend.proxy_yaml({
                "name": "TCP", "uri": f"{base}&mode=provider-metadata#{fragment}"
            }),
        )

    def test_unsupported_transport_options_are_not_silently_changed(self):
        with self.assertRaisesRegex(backend.BackendError, "Unsupported VLESS transport"):
            backend.parse_vless(REALITY_URI.replace("type=tcp", "type=httpupgrade"))
        base, fragment = REALITY_URI.split("#", 1)
        with self.assertRaisesRegex(backend.BackendError, "TCP header type"):
            backend.parse_vless(base + "&headerType=http#" + fragment)
        with self.assertRaisesRegex(backend.BackendError, "requires the TCP transport"):
            backend.parse_vless(REALITY_URI.replace("type=tcp", "type=ws"))

    def test_template_replaces_only_top_level_proxies(self):
        source = "port: 7890\nproxy-groups:\n- name: P\n  proxies:\n  - old\nproxies:\n- name: old\nrules:\n- MATCH,P\n"
        result = backend.template_from_config(source)
        self.assertIn("proxy-groups:\n- name: P\n  proxies:\n  - old", result)
        self.assertIn("proxies:\n{{OMAVLESS_PROXY}}\nrules:", result)
        self.assertNotIn("- name: old", result)

    def test_routing_status_distinguishes_mode_from_policy(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)

            basic = """mode: rule
proxies:
{{OMAVLESS_PROXY}}
rules:
  - IP-CIDR,127.0.0.0/8,DIRECT,no-resolve
  - IP-CIDR,10.0.0.0/8,DIRECT,no-resolve
  - IP-CIDR,172.16.0.0/12,DIRECT,no-resolve
  - IP-CIDR,192.168.0.0/16,DIRECT,no-resolve
  - MATCH,PROXY
"""
            paths.template.write_text(basic, encoding="utf-8")
            self.assertEqual(backend.routing_status(paths, False), {
                "mode": "rule", "source": "basic", "preset": "",
                "configured": False, "ruleCount": 5, "providerCount": 0,
                "customRuleCount": 0, "rulesUpdatedAt": 0,
                "ruleUpdateAvailable": False,
            })

            roscomvpn = """# omavless-routing-profile: roscomvpn-default
mode: rule
rule-providers:
  category-ru:
    type: http
    url: https://example.invalid/roscomvpn-geosite/category-ru.mrs
  direct-ips:
    type: http
    url: https://example.invalid/roscomvpn-geoip/direct.mrs
proxies:
{{OMAVLESS_PROXY}}
rules:
  - RULE-SET,category-ru,DIRECT
  - MATCH,PROXY
"""
            paths.template.write_text(roscomvpn, encoding="utf-8")
            self.assertEqual(backend.routing_status(paths, False), {
                "mode": "rule", "source": "roscomvpn",
                "preset": "roscomvpn-default", "configured": False,
                "ruleCount": 2, "providerCount": 2,
                "customRuleCount": 0, "rulesUpdatedAt": 0,
                "ruleUpdateAvailable": False,
            })
            selected = backend.routing_status(
                paths, False, {"routingPreset": "roscomvpn-default"}
            )
            self.assertEqual(selected["preset"], "roscomvpn-default")
            self.assertTrue(selected["configured"])

            paths.template.write_text(roscomvpn.replace("mode: rule", "mode: global"), encoding="utf-8")
            state = backend.routing_status(paths, False)
            self.assertEqual(state["mode"], "global")
            self.assertEqual(state["source"], "roscomvpn")

    def test_external_tun_detection_excludes_the_own_running_device(self):
        with tempfile.TemporaryDirectory() as temp:
            net = Path(temp)
            for name in ("Meta", "tun-v2ray", "eth0"):
                (net / name).mkdir()
            (net / "Meta" / "tun_flags").write_text("0001", encoding="utf-8")
            (net / "tun-v2ray" / "tun_flags").write_text("0001", encoding="utf-8")
            self.assertEqual(
                backend.external_tun_interfaces("Meta", True, net), ["tun-v2ray"]
            )

    def test_vpn_process_detection_is_bounded_and_excludes_own_core(self):
        with tempfile.TemporaryDirectory() as temp:
            proc = Path(temp)
            for pid, name in (("10", "mihomo"), ("11", "v2rayN"), ("12", "bash")):
                (proc / pid).mkdir()
                (proc / pid / "comm").write_text(name + "\n", encoding="utf-8")
            self.assertEqual(backend.vpn_process_labels(proc, own_pid=10), ["V2RayN"])

    def test_systemd_uptime_uses_monotonic_clock_and_is_bounded(self):
        completed = subprocess.CompletedProcess([], 0, "2500000\n", "")
        with mock.patch.object(backend, "systemctl", return_value=completed), \
             mock.patch.object(backend.time, "clock_gettime", return_value=12.5):
            self.assertEqual(backend.service_uptime_seconds("x.service", True), 10)
        self.assertEqual(backend.service_uptime_seconds("x.service", False), 0)

    def test_status_reports_the_effective_routing_config_without_secrets(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            paths.template.write_text(
                "mode: rule\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,PROXY\n",
                encoding="utf-8",
            )
            with mock.patch.object(backend, "service_active", return_value=False):
                payload = json.loads(backend.status_text(paths))
            self.assertEqual(payload["routing"], {
                "mode": "rule", "source": "custom", "preset": "",
                "configured": False, "ruleCount": 1, "providerCount": 0,
                "customRuleCount": 0, "rulesUpdatedAt": 0,
                "ruleUpdateAvailable": False,
            })
            self.assertEqual(payload["capabilities"]["core"], "mihomo")
            self.assertEqual(payload["capabilities"]["protocols"], ["vless"])
            self.assertEqual(payload["coreSetup"], {
                "installed": False, "tunReady": False, "path": "",
            })
            self.assertEqual(payload["startup"], {
                "enabled": False, "configured": True, "target": "last",
                "profileId": "", "mode": "rule",
            })
            self.assertFalse(payload["onboardingComplete"])
            self.assertIn("uptimeSeconds", payload)
            self.assertIn("conflicts", payload)
            self.assertNotIn("vless://", json.dumps(payload))

    def test_routing_mode_rewrite_is_explicit_and_preserves_policy(self):
        source = "# profile\nmode: rule # selected\nrules:\n  - MATCH,PROXY\n"
        self.assertEqual(
            backend.template_with_mode(source, "global"),
            "# profile\nmode: global # selected\nrules:\n  - MATCH,PROXY\n",
        )
        self.assertEqual(
            backend.template_with_mode("rules:\n  - MATCH,PROXY\n", "direct"),
            "mode: direct\nrules:\n  - MATCH,PROXY\n",
        )
        with self.assertRaisesRegex(backend.BackendError, "Unsupported routing mode"):
            backend.template_with_mode(source, "invalid")
        with self.assertRaisesRegex(backend.BackendError, "more than one"):
            backend.template_with_mode("mode: rule\nmode: direct\n", "global")

    def test_custom_rule_values_are_canonical_and_bounded(self):
        self.assertEqual(backend.canonical_custom_rule_value("suffix", "*.Example.COM."),
                         "example.com")
        self.assertEqual(backend.canonical_custom_rule_value("domain", "пример.рф"),
                         "xn--e1afmkfd.xn--p1ai")
        self.assertEqual(backend.canonical_custom_rule_value("ipcidr", "192.0.2.7/24"),
                         "192.0.2.0/24")
        with self.assertRaisesRegex(backend.BackendError, "without a scheme"):
            backend.canonical_custom_rule_value("domain", "https://example.com/path")
        with self.assertRaisesRegex(backend.BackendError, "valid domain"):
            backend.canonical_custom_rule_value("domain", "localhost")

    def test_private_runtime_config_strips_public_controllers_and_prepends_custom_rules(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            store = backend.empty_store()
            store["customRules"] = [
                {"id": "22222222-2222-4222-8222-222222222222",
                 "kind": "suffix", "value": "example.com", "action": "direct"},
                {"id": "33333333-3333-4333-8333-333333333333",
                 "kind": "ipcidr", "value": "2001:db8::/32", "action": "reject"},
            ]
            source = """external-controller: 0.0.0.0:9090
external-controller-cors:
  allow-origins:
    - '*'
secret: exposed
rules:
  - RULE-SET,base,PROXY
  - MATCH,PROXY
"""
            result = backend.private_runtime_config(paths, source, store)
            self.assertIn(f'external-controller-unix: "{backend.controller_socket(paths)}"', result)
            self.assertNotIn("0.0.0.0", result)
            self.assertNotIn("exposed", result)
            self.assertNotIn("allow-origins", result)
            self.assertLess(result.index("DOMAIN-SUFFIX,example.com,DIRECT"),
                            result.index("RULE-SET,base,PROXY"))
            self.assertIn("IP-CIDR6,2001:db8::/32,REJECT-DROP,no-resolve", result)

    def test_custom_rule_mutations_reconnect_and_keep_values_out_of_public_status(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            store = backend.empty_store()
            store["profiles"] = [{"id": profile_id, "name": "Example", "uri": REALITY_URI}]
            store["activeId"] = profile_id
            backend.save_store(paths, store)
            with mock.patch.object(backend, "service_active", return_value=True), \
                 mock.patch.object(backend, "connect_profile") as reconnect:
                rule = backend.save_custom_rule(paths, "suffix", "proxy", "Private.Example")
            reconnect.assert_called_once_with(paths, profile_id)
            self.assertEqual(rule["value"], "private.example")
            explicit = backend.custom_rules_text(paths)
            self.assertIn("private.example", explicit)
            with mock.patch.object(backend, "service_active", return_value=False):
                public = backend.status_text(paths)
            self.assertNotIn("private.example", public)
            self.assertIn('"customRuleCount":1', public)
            with mock.patch.object(backend, "service_active", return_value=False):
                backend.delete_custom_rule(paths, rule["id"])
            self.assertEqual(backend.load_store(paths)["customRules"], [])

    def test_route_check_explains_mode_custom_and_disconnected_results(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            store = backend.empty_store()
            store["customRules"] = [{
                "id": "22222222-2222-4222-8222-222222222222",
                "kind": "suffix", "value": "example.com", "action": "direct",
            }]
            backend.save_store(paths, store)
            paths.template.write_text(
                "mode: rule\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,PROXY\n",
                encoding="utf-8",
            )
            with mock.patch.object(backend, "service_active", return_value=False):
                direct = backend.route_check(paths, "www.example.com")
                unknown = backend.route_check(paths, "openai.com")
            self.assertEqual(direct["outcome"], "direct")
            self.assertEqual(direct["source"], "custom")
            self.assertEqual(direct["rulePayload"], "example.com")
            self.assertEqual(unknown["outcome"], "unknown")
            paths.template.write_text(
                "mode: global\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,PROXY\n",
                encoding="utf-8",
            )
            with mock.patch.object(backend, "service_active", return_value=False):
                global_result = backend.route_check(paths, "openai.com")
            self.assertEqual(global_result["outcome"], "vpn")
            self.assertEqual(global_result["ruleType"], "MODE")

    def test_live_route_check_reports_the_rule_without_proxy_names(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            paths.config.write_text("mixed-port: 7890\n", encoding="utf-8")
            backend.controller_socket(paths).touch()
            probe = mock.Mock()
            controller_responses = [
                (200, {"rules": [{
                    "index": 0, "type": "RULE-SET", "payload": "github", "proxy": "PROXY",
                    "extra": {"hitCount": 3, "hitAt": 100},
                }]}),
                (200, {"connections": []}),
                (200, {"connections": [{
                    "id": "new", "metadata": {"host": "example.com", "destinationIP": ""},
                    "rule": "RuleSet", "rulePayload": "github", "chains": ["Secret node", "PROXY"],
                }]}),
                (200, {"rules": [{
                    "index": 0, "type": "RULE-SET", "payload": "github", "proxy": "PROXY",
                    "extra": {"hitCount": 4, "hitAt": 101},
                }]}),
            ]
            with mock.patch.object(backend, "controller_json", side_effect=controller_responses), \
                 mock.patch.object(backend.socket, "create_connection", return_value=probe):
                result = backend.live_route_match(paths, "example.com", False)
            self.assertEqual(result, {
                "outcome": "vpn", "ruleType": "RuleSet", "rulePayload": "github",
                "target": "PROXY", "source": "live",
            })
            probe.sendall.assert_called_once()
            probe.close.assert_called_once_with()
            self.assertNotIn("Secret node", json.dumps(result))

    def test_live_route_check_uses_rule_hit_for_an_immediate_reject(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            paths.config.write_text("mixed-port: 7890\n", encoding="utf-8")
            backend.controller_socket(paths).touch()
            probe = mock.Mock()
            controller_responses = [
                (200, {"rules": [{
                    "index": 4, "type": "DOMAIN-SUFFIX", "payload": "ads.example",
                    "proxy": "REJECT-DROP", "extra": {"hitCount": 9, "hitAt": 100},
                }]}),
                (200, {"connections": []}),
                (200, {"connections": []}),
                (200, {"rules": [{
                    "index": 4, "type": "DOMAIN-SUFFIX", "payload": "ads.example",
                    "proxy": "REJECT-DROP", "extra": {"hitCount": 10, "hitAt": 101},
                }]}),
            ]
            with mock.patch.object(backend, "controller_json", side_effect=controller_responses), \
                 mock.patch.object(backend.socket, "create_connection", return_value=probe):
                result = backend.live_route_match(paths, "ads.example", False)
            self.assertEqual(result, {
                "outcome": "block", "ruleType": "DOMAIN-SUFFIX",
                "rulePayload": "ads.example", "target": "REJECT", "source": "live",
            })

    def test_remote_rule_refresh_updates_timestamp_only_after_every_http_provider(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            backend.save_store(paths, backend.empty_store())
            backend.controller_socket(paths).touch()
            providers = {"providers": {
                "one": {"vehicleType": "HTTP"},
                "two": {"vehicleType": "HTTP"},
                "local": {"vehicleType": "File"},
            }}
            with mock.patch.object(backend, "service_active", return_value=True), \
                 mock.patch.object(backend, "controller_json", return_value=(200, providers)), \
                 mock.patch.object(backend, "controller_request", return_value=(204, {})) as request:
                result = backend.refresh_rule_providers(paths)
            self.assertEqual(result["updated"], 2)
            self.assertGreater(result["updatedAt"], 0)
            self.assertEqual(request.call_count, 2)
            self.assertEqual(backend.load_store(paths)["rulesUpdatedAt"], result["updatedAt"])

    def test_set_routing_mode_reconnects_and_rolls_back_on_failure(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            previous = "mode: rule\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,PROXY\n"
            paths.template.write_text(previous, encoding="utf-8")
            paths.store.write_text(json.dumps({
                "version": 1, "activeId": profile_id, "lastId": profile_id,
                "profiles": [{"id": profile_id, "name": "Example", "uri": REALITY_URI}],
            }), encoding="utf-8")
            with mock.patch.object(backend, "service_active", return_value=True), \
                 mock.patch.object(backend, "connect_profile") as reconnect:
                backend.set_routing_mode(paths, "global")
            reconnect.assert_called_once_with(paths, profile_id)
            self.assertIn("mode: global", paths.template.read_text(encoding="utf-8"))

            paths.template.write_text(previous, encoding="utf-8")
            with mock.patch.object(backend, "service_active", return_value=True), \
                 mock.patch.object(backend, "connect_profile", side_effect=backend.BackendError("nope")):
                with self.assertRaisesRegex(backend.BackendError, "nope"):
                    backend.set_routing_mode(paths, "direct")
            self.assertEqual(paths.template.read_text(encoding="utf-8"), previous)

    def test_set_routing_mode_refuses_an_untracked_running_service(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            previous = "mode: rule\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,PROXY\n"
            paths.template.write_text(previous, encoding="utf-8")
            paths.store.write_text(json.dumps({
                "version": 1, "activeId": "", "lastId": "", "profiles": [],
            }), encoding="utf-8")
            with mock.patch.object(backend, "service_active", return_value=True):
                with self.assertRaisesRegex(backend.BackendError, "without an active profile"):
                    backend.set_routing_mode(paths, "global")
            self.assertEqual(paths.template.read_text(encoding="utf-8"), previous)

    def test_bundled_roscomvpn_template_has_a_complete_self_consistent_policy(self):
        text = (ROOT / "templates" / "default.yaml").read_text(encoding="utf-8")
        self.assertEqual(text.count(backend.PROFILE_MARKER), 1)
        providers = backend.yaml_top_level_block(text, "rule-providers")
        rules = backend.yaml_top_level_block(text, "rules")
        self.assertEqual(backend.yaml_mapping_count(providers), 23)
        self.assertEqual(backend.yaml_sequence_count(rules), 27)
        self.assertIn("# omavless-routing-profile: roscomvpn-default", text)
        self.assertIn("RULE-SET,category-ru,DIRECT", text)
        self.assertIn("RULE-SET,youtube,PROXY", text)
        self.assertIn("MATCH,PROXY", text)
        defined = {
            match.group(1)
            for line in providers
            if (match := re.match(r"^  ([A-Za-z0-9_-]+):\s*$", line))
        }
        referenced = {
            match.group(1)
            for line in rules
            if (match := re.search(r"RULE-SET,([^,]+),", line))
        }
        self.assertEqual(referenced - defined, set())

    def test_country_templates_are_mihomo_native_and_self_consistent(self):
        expected = {
            "china.yaml": ("china-cn-direct", 5, 6, "MetaCubeX/meta-rules-dat"),
            "iran.yaml": ("iran-ir-direct", 7, 8, "Chocolate4U/Iran-clash-rules"),
        }
        for filename, (preset, provider_count, rule_count, source) in expected.items():
            with self.subTest(filename=filename):
                text = (ROOT / "templates" / filename).read_text(encoding="utf-8")
                self.assertEqual(text.count(backend.PROFILE_MARKER), 1)
                providers = backend.yaml_top_level_block(text, "rule-providers")
                rules = backend.yaml_top_level_block(text, "rules")
                self.assertEqual(backend.yaml_mapping_count(providers), provider_count)
                self.assertEqual(backend.yaml_sequence_count(rules), rule_count)
                self.assertIn(f"# omavless-routing-profile: {preset}", text)
                self.assertIn(source, text)
                self.assertIn("format: mrs", text)
                self.assertIn("MATCH,PROXY", text)
                defined = {
                    match.group(1)
                    for line in providers
                    if (match := re.match(r"^  ([A-Za-z0-9_-]+):\s*$", line))
                }
                referenced = {
                    match.group(1)
                    for line in rules
                    if (match := re.search(r"RULE-SET,([^,]+),", line))
                }
                self.assertEqual(referenced - defined, set())

    def test_settings_preset_change_preserves_the_current_mode(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            current = backend.bundled_template("roscomvpn-default")
            paths.template.write_text(
                backend.template_with_mode(current, "global"), encoding="utf-8"
            )
            paths.store.write_text(json.dumps(backend.empty_store()), encoding="utf-8")
            with mock.patch.object(backend, "service_active", return_value=False):
                backend.use_bundled_template(paths, "china-cn-direct", keep_mode=True)
            updated = paths.template.read_text(encoding="utf-8")
            store = json.loads(paths.store.read_text(encoding="utf-8"))
            self.assertEqual(backend.yaml_top_level_scalar(updated, "mode"), "global")
            self.assertIn("# omavless-routing-profile: china-cn-direct", updated)
            self.assertEqual(store["routingPreset"], "china-cn-direct")

    def test_first_routing_preset_selection_activates_rule_mode(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            current = backend.template_with_mode(
                backend.bundled_template("roscomvpn-default"), "direct"
            )
            paths.template.write_text(current, encoding="utf-8")
            paths.store.write_text(json.dumps(backend.empty_store()), encoding="utf-8")
            with mock.patch.object(backend, "service_active", return_value=False):
                backend.use_bundled_template(paths, "iran-ir-direct")
            updated = paths.template.read_text(encoding="utf-8")
            self.assertEqual(backend.yaml_top_level_scalar(updated, "mode"), "rule")
            self.assertEqual(
                json.loads(paths.store.read_text(encoding="utf-8"))["routingPreset"],
                "iran-ir-direct",
            )

    def test_use_bundled_routing_preserves_previous_template_if_reconnect_fails(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            previous = "mode: rule\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,PROXY\n"
            paths.template.write_text(previous, encoding="utf-8")
            paths.store.write_text(json.dumps({
                "version": 1, "activeId": profile_id, "lastId": profile_id,
                "profiles": [{"id": profile_id, "name": "Example", "uri": REALITY_URI}],
            }), encoding="utf-8")
            with mock.patch.object(backend, "service_active", return_value=True), \
                 mock.patch.object(backend, "connect_profile", side_effect=backend.BackendError("nope")):
                with self.assertRaisesRegex(backend.BackendError, "nope"):
                    backend.use_bundled_template(paths, "china-cn-direct")
            self.assertEqual(paths.template.read_text(encoding="utf-8"), previous)
            restored = json.loads(paths.store.read_text(encoding="utf-8"))
            self.assertEqual(restored["routingPreset"], "roscomvpn-default")

    def test_store_is_private_and_status_has_no_uri(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            env, _ = self.make_env(home)
            imported = subprocess.run(
                [str(ROOT / "backend.sh"), "import", "Example"],
                input=REALITY_URI, text=True, env=env, capture_output=True,
            )
            self.assertEqual(imported.returncode, 0, imported.stderr)
            config_dir = home / ".config" / "omavless"
            store = config_dir / "profiles.json"
            self.assertEqual(stat.S_IMODE(config_dir.stat().st_mode), 0o700)
            self.assertEqual(stat.S_IMODE(store.stat().st_mode), 0o600)
            data = json.loads(store.read_text(encoding="utf-8"))
            self.assertEqual(data["profiles"][0]["name"], "Example")
            status_result = subprocess.run(
                [str(ROOT / "backend.sh"), "status"], env=env, text=True, capture_output=True,
            )
            self.assertEqual(status_result.returncode, 0, status_result.stderr)
            status = json.loads(status_result.stdout)
            self.assertEqual(status["version"], 1)
            self.assertEqual(status["profiles"][0]["name"], "Example")
            self.assertFalse(status["profiles"][0]["active"])
            self.assertNotIn("vless://", status_result.stdout)

    def test_v1_store_migrates_in_memory_to_subscription_capable_v2(self):
        profile_id = "22222222-2222-4222-8222-222222222222"
        migrated = backend.validate_store({
            "version": 1, "activeId": "", "lastId": profile_id,
            "profiles": [{"id": profile_id, "name": "Example", "uri": REALITY_URI}],
        })
        self.assertEqual(migrated["version"], 2)
        self.assertEqual(migrated["routingPreset"], "roscomvpn-default")
        self.assertEqual(migrated["subscriptions"], [])
        self.assertNotIn("subscriptionId", migrated["profiles"][0])

    def test_fresh_store_defers_routing_preset_choice(self):
        fresh = backend.validate_store(backend.empty_store())
        self.assertEqual(fresh["routingPreset"], "")

    def test_subscription_parser_accepts_raw_and_urlsafe_base64_lists(self):
        second = REALITY_URI.replace("example.com:443", "two.example:8443").replace(
            "#Example", "#Second"
        )
        raw = REALITY_URI + "\ntrojan://ignored\n" + second + "\n"
        profiles, skipped = backend.parse_subscription(raw)
        self.assertEqual(len(profiles), 2)
        self.assertEqual(skipped, 0)
        encoded = backend.base64.urlsafe_b64encode(raw.encode()).decode().rstrip("=")
        profiles64, skipped64 = backend.parse_subscription(encoded)
        self.assertEqual([item["key"] for item in profiles64], [item["key"] for item in profiles])
        self.assertEqual(skipped64, 0)

    def test_subscription_identity_ignores_label_and_query_order(self):
        base, _label = REALITY_URI.split("#", 1)
        parsed = urllib.parse.urlsplit(base)
        reordered = urllib.parse.urlunsplit((
            parsed.scheme, parsed.netloc, parsed.path,
            urllib.parse.urlencode(list(reversed(urllib.parse.parse_qsl(parsed.query)))),
            "Renamed by provider",
        ))
        self.assertEqual(
            backend.subscription_key(REALITY_URI), backend.subscription_key(reordered)
        )

    def test_subscription_save_is_private_and_status_never_exposes_url(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            backend.ensure_private_dir(paths.config_dir)
            secret_url = "https://provider.example/sub?token=do-not-leak"
            with mock.patch.object(backend, "fetch_subscription", return_value=REALITY_URI):
                result = backend.save_subscription(paths, "My provider", "", secret_url)
            self.assertEqual(result["total"], 1)
            self.assertEqual(stat.S_IMODE(paths.store.stat().st_mode), 0o600)
            stored = json.loads(paths.store.read_text(encoding="utf-8"))
            self.assertEqual(stored["version"], 2)
            self.assertEqual(stored["subscriptions"][0]["url"], secret_url)
            with mock.patch.object(backend, "service_active", return_value=False):
                public = backend.status_text(paths)
            self.assertNotIn(secret_url, public)
            self.assertNotIn("do-not-leak", public)
            payload = json.loads(public)
            self.assertEqual(payload["subscriptions"][0]["name"], "My provider")
            self.assertEqual(payload["profiles"][0]["sourceName"], "My provider")

    def test_subscription_cli_fetches_from_stdin_end_to_end_in_isolated_home(self):
        body = backend.base64.b64encode((REALITY_URI + "\n").encode())

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                self.send_response(200)
                self.send_header("Content-Type", "text/plain; charset=utf-8")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, _format, *_args):
                pass

        try:
            server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        except PermissionError:
            self.skipTest("sandbox does not permit a loopback integration server")
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory() as temp:
                home = Path(temp)
                env, _ = self.make_env(home)
                url = f"http://127.0.0.1:{server.server_port}/sub?token=private"
                added = subprocess.run(
                    [str(ROOT / "backend.sh"), "subscription-save", "Local provider"],
                    input=url, text=True, env=env, capture_output=True,
                )
                self.assertEqual(added.returncode, 0, added.stderr)
                self.assertEqual(json.loads(added.stdout)["total"], 1)
                public = subprocess.run(
                    [str(ROOT / "backend.sh"), "status"], env=env,
                    text=True, capture_output=True,
                )
                self.assertEqual(public.returncode, 0, public.stderr)
                self.assertNotIn("token=private", public.stdout)
                payload = json.loads(public.stdout)
                self.assertEqual(payload["subscriptions"][0]["profileCount"], 1)
                self.assertEqual(payload["profiles"][0]["sourceName"], "Local provider")
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)

    def test_subscription_fetch_is_bounded_and_redacts_bearer_url_from_errors(self):
        secret_url = "https://provider.example/sub?token=do-not-leak"
        error = urllib.error.HTTPError(secret_url, 403, "Forbidden", {}, None)
        opener = mock.Mock()
        opener.open.side_effect = error
        with mock.patch.object(backend.urllib.request, "build_opener", return_value=opener):
            with self.assertRaises(backend.BackendError) as raised:
                backend.fetch_subscription(secret_url)
        self.assertTrue(error.closed)
        self.assertIn("provider.example", str(raised.exception))
        self.assertNotIn("do-not-leak", str(raised.exception))

        class OversizedResponse:
            def __enter__(self): return self
            def __exit__(self, *_args): return False
            def geturl(self): return "https://provider.example/sub"
            def read(self, size): return b"x" * size

        opener.open.side_effect = None
        opener.open.return_value = OversizedResponse()
        with mock.patch.object(backend.urllib.request, "build_opener", return_value=opener):
            with self.assertRaisesRegex(backend.BackendError, "response is too large"):
                backend.fetch_subscription("https://provider.example/sub")

    def test_subscription_probe_is_credential_free(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            backend.ensure_private_dir(paths.config_dir)
            subscription_id = "11111111-1111-4111-8111-111111111111"
            first_id = "22222222-2222-4222-8222-222222222222"
            second_id = "33333333-3333-4333-8333-333333333333"
            second_uri = REALITY_URI.replace("example.com:443", "second.example:8443")
            paths.store.write_text(json.dumps({
                "version": 2, "activeId": "", "lastId": "",
                "subscriptions": [{
                    "id": subscription_id, "name": "Provider",
                    "url": "https://provider.example/sub?token=private", "updatedAt": 1,
                }],
                "profiles": [
                    {"id": first_id, "name": "Fast", "uri": REALITY_URI,
                     "subscriptionId": subscription_id, "subscriptionKey": "a" * 64, "missing": False},
                    {"id": second_id, "name": "Down", "uri": second_uri,
                     "subscriptionId": subscription_id, "subscriptionKey": "b" * 64, "missing": False},
                ],
            }), encoding="utf-8")
            paths.store.chmod(0o600)

            def resolve(host, _resolvers):
                return ["8.8.8.8"] if host == "example.com" else ["1.1.1.1"]

            with mock.patch.object(
                backend, "configured_working_probe_resolvers", return_value=["https://dns.example/query"]
            ), mock.patch.object(
                backend, "resolve_probe_addresses", side_effect=resolve
            ), mock.patch.object(
                backend, "run_mihomo_probe",
                return_value={"p0000a0": [40, 44, 42], "p0001a0": []},
            ) as run_probe:
                result = backend.probe_subscription(paths, subscription_id)
            self.assertEqual(result["version"], 1)
            self.assertEqual(result["subscriptionId"], subscription_id)
            self.assertEqual(result["results"], [
                {"id": first_id, "resolved": True, "reachable": True, "latencyMs": 42},
                {"id": second_id, "resolved": True, "reachable": False, "latencyMs": -1},
            ])
            public = json.dumps(result)
            self.assertNotIn("vless://", public)
            self.assertNotIn("token=private", public)
            self.assertNotIn("example.com", public)
            targets = run_probe.call_args.args[1]
            self.assertEqual([(item["alias"], item["address"]) for item in targets], [
                ("p0000a0", "8.8.8.8"), ("p0001a0", "1.1.1.1"),
            ])

    def test_subscription_probe_stream_publishes_incremental_results(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            paths = self.paths_for(home)
            backend.ensure_private_dir(paths.config_dir)
            subscription_id = "11111111-1111-4111-8111-111111111111"
            first_id = "22222222-2222-4222-8222-222222222222"
            second_id = "33333333-3333-4333-8333-333333333333"
            second_uri = REALITY_URI.replace("example.com:443", "unresolved.example:443")
            paths.store.write_text(json.dumps({
                "version": 2, "activeId": "", "lastId": "",
                "subscriptions": [{
                    "id": subscription_id, "name": "Provider",
                    "url": "https://provider.example/sub?token=private", "updatedAt": 1,
                }],
                "profiles": [
                    {"id": first_id, "name": "Fast", "uri": REALITY_URI,
                     "subscriptionId": subscription_id, "subscriptionKey": "a" * 64,
                     "missing": False},
                    {"id": second_id, "name": "No DNS", "uri": second_uri,
                     "subscriptionId": subscription_id, "subscriptionKey": "b" * 64,
                     "missing": False},
                ],
            }), encoding="utf-8")
            paths.store.chmod(0o600)

            def resolve(host, _resolvers):
                return ["8.8.8.8"] if host == "example.com" else []

            def run_probe(_paths, _targets, on_update):
                on_update({"p0000a0": [41]})
                on_update({"p0000a0": [41, 43]})
                return {"p0000a0": [41, 43]}

            output = io.StringIO()
            with mock.patch.object(
                backend, "configured_working_probe_resolvers", return_value=[]
            ), mock.patch.object(
                backend, "resolve_probe_addresses", side_effect=resolve
            ), mock.patch.object(
                backend, "run_mihomo_probe", side_effect=run_probe
            ), mock.patch("sys.stdout", output):
                backend.probe_subscription_stream(paths, subscription_id)

            events = [json.loads(line) for line in output.getvalue().splitlines()]
            self.assertEqual(events[0], {
                "version": 1, "type": "start",
                "subscriptionId": subscription_id, "total": 2,
            })
            self.assertEqual([event["latencyMs"] for event in events
                              if event.get("id") == first_id], [41, 42])
            self.assertIn({
                "version": 1, "type": "result", "subscriptionId": subscription_id,
                "id": second_id, "resolved": False, "reachable": False,
                "latencyMs": -1,
            }, events)
            self.assertEqual(events[-1], {
                "version": 1, "type": "complete", "subscriptionId": subscription_id,
                "tested": 2, "unavailable": 0, "unresolved": 1,
            })
            public = output.getvalue()
            self.assertNotIn("vless://", public)
            self.assertNotIn("token=private", public)
            self.assertNotIn("example.com", public)

    def test_probe_core_config_is_tun_free_and_pins_real_endpoint_ip(self):
        profile = {"name": "Private provider name", "uri": REALITY_URI}
        config = backend.probe_core_config([{
            "alias": "p0000a0", "address": "8.8.8.8", "profile": profile,
        }])
        self.assertIn("external-controller-unix: controller.sock", config)
        self.assertIn(f"routing-mark: {backend.PROBE_ROUTING_MARK}", config)
        self.assertIn('server: "8.8.8.8"', config)
        self.assertIn('name: "p0000a0"', config)
        self.assertNotIn("tun:", config)
        self.assertNotIn("example.com", config)
        self.assertNotIn("Private provider name", config)
        self.assertNotIn("vless://", config)

    def test_probe_api_treats_all_timeouts_as_valid_unavailable_results(self):
        samples = {"p0000a0": []}
        self.assertTrue(backend.collect_probe_response(
            samples, 504, {"message": "get delay: all proxies timeout"}
        ))
        self.assertEqual(samples, {"p0000a0": []})
        self.assertTrue(backend.collect_probe_response(
            samples, 200, {"p0000a0": 73, "unknown": 1, "DIRECT": 0}
        ))
        self.assertEqual(samples, {"p0000a0": [73]})
        self.assertFalse(backend.collect_probe_response(samples, 500, {"message": "broken"}))

    def test_probe_core_is_stopped_after_successful_checks(self):
        process = mock.Mock()
        process.poll.return_value = None
        process.wait.return_value = 0
        target = {
            "alias": "p0000a0", "address": "8.8.8.8",
            "profile": {"name": "Example", "uri": REALITY_URI},
        }
        with tempfile.TemporaryDirectory() as temp, mock.patch.object(
            backend, "find_core", return_value=Path("/usr/bin/mihomo")
        ), mock.patch.object(
            backend.subprocess, "Popen", return_value=process
        ), mock.patch.object(
            backend, "wait_probe_controller"
        ), mock.patch.object(
            backend, "controller_json",
            side_effect=[(200, {"p0000a0": value}) for value in (41, 43, 42)],
        ):
            result = backend.run_mihomo_probe(self.paths_for(Path(temp)), [target])
        self.assertEqual(result, {"p0000a0": [41, 43, 42]})
        process.terminate.assert_called_once_with()
        process.wait.assert_called_once_with(timeout=backend.PROBE_CORE_STOP_SECONDS)

    def test_probe_core_is_stopped_when_widget_terminates_probe_backend(self):
        process = mock.Mock()
        process.poll.side_effect = [None, 0]
        process.wait.return_value = 0
        target = {
            "alias": "p0000a0", "address": "8.8.8.8",
            "profile": {"name": "Example", "uri": REALITY_URI},
        }
        previous = backend.signal.getsignal(backend.signal.SIGTERM)

        def terminate_backend(_process, _socket):
            backend.signal.raise_signal(backend.signal.SIGTERM)

        with tempfile.TemporaryDirectory() as temp, mock.patch.object(
            backend, "find_core", return_value=Path("/usr/bin/mihomo")
        ), mock.patch.object(
            backend.subprocess, "Popen", return_value=process
        ), mock.patch.object(
            backend, "wait_probe_controller", side_effect=terminate_backend
        ):
            with self.assertRaises(SystemExit) as raised:
                backend.run_mihomo_probe(self.paths_for(Path(temp)), [target])
        self.assertEqual(raised.exception.code, 128 + backend.signal.SIGTERM)
        self.assertIs(backend.signal.getsignal(backend.signal.SIGTERM), previous)
        process.terminate.assert_called_once_with()

    def test_probe_aggregates_multiple_addresses_and_uses_successful_median(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            paths = self.paths_for(home)
            backend.ensure_private_dir(paths.config_dir)
            subscription_id = "11111111-1111-4111-8111-111111111111"
            profile_id = "22222222-2222-4222-8222-222222222222"
            paths.store.write_text(json.dumps({
                "version": 2, "activeId": "", "lastId": "",
                "subscriptions": [{"id": subscription_id, "name": "P", "url": "https://p.example/s", "updatedAt": 1}],
                "profiles": [{"id": profile_id, "name": "N", "uri": REALITY_URI,
                              "subscriptionId": subscription_id, "subscriptionKey": "a" * 64,
                              "missing": False}],
            }), encoding="utf-8")
            paths.store.chmod(0o600)
            with mock.patch.object(
                backend, "configured_working_probe_resolvers", return_value=[]
            ), mock.patch.object(
                backend, "resolve_probe_addresses", return_value=["8.8.8.8", "2001:4860:4860::8888"]
            ), mock.patch.object(
                backend, "run_mihomo_probe",
                return_value={"p0000a0": [90, 70], "p0000a1": [50]},
            ):
                result = backend.probe_subscription(paths, subscription_id)
            self.assertEqual(result["results"], [{
                "id": profile_id, "resolved": True, "reachable": True, "latencyMs": 70,
            }])

    def test_probe_resolution_ignores_fake_ip_answers(self):
        class Response:
            def __init__(self, body): self.body = body
            def __enter__(self): return self
            def __exit__(self, *_args): return False
            def read(self, _size): return self.body

        def answer(request, timeout):
            self.assertEqual(timeout, backend.PROBE_DNS_TIMEOUT_SECONDS)
            question = request.data[12:]
            transaction_id = backend.struct.unpack("!H", request.data[:2])[0]
            header = backend.struct.pack("!HHHHHH", transaction_id, 0x8180, 1, 2, 0, 0)
            fake = b"\xc0\x0c" + backend.struct.pack("!HHIH", 1, 1, 60, 4) \
                + backend.socket.inet_aton("198.18.0.42")
            public = b"\xc0\x0c" + backend.struct.pack("!HHIH", 1, 1, 60, 4) \
                + backend.socket.inet_aton("8.8.8.8")
            return Response(header + question + fake + public)

        backend.resolve_probe_host.cache_clear()
        backend.resolve_probe_records.cache_clear()
        with mock.patch.object(backend.urllib.request, "urlopen", side_effect=answer) as opened:
            self.assertEqual(backend.resolve_probe_host(
                "node.example", "https://77.88.8.8/dns-query"
            ), "8.8.8.8")
        request = opened.call_args.args[0]
        self.assertEqual(request.full_url, "https://77.88.8.8/dns-query")
        self.assertEqual(request.headers["Content-type"], "application/dns-message")
        self.assertNotIn("vless://", request.full_url)

    def test_probe_uses_resolver_from_private_routing_policy(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            paths = self.paths_for(home)
            backend.ensure_private_dir(paths.config_dir)
            paths.config.write_text("""dns:
  direct-nameserver:
    - https://77.88.8.8/dns-query
  nameserver:
    - https://8.8.8.8/dns-query#PROXY
""", encoding="utf-8")
            paths.config.chmod(0o600)
            self.assertEqual(backend.configured_probe_resolvers(paths), [
                "https://77.88.8.8/dns-query",
                "https://8.8.8.8/dns-query",
            ])

    def test_probe_skips_broken_resolver_and_keeps_working_fallback(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            paths = self.paths_for(home)
            backend.ensure_private_dir(paths.config_dir)
            paths.config.write_text("""dns:
  direct-nameserver:
    - https://broken.example/dns-query
  nameserver:
    - https://working.example/dns-query#PROXY
""", encoding="utf-8")
            paths.config.chmod(0o600)
            with mock.patch.object(
                backend, "probe_resolver_works",
                side_effect=lambda resolver: resolver == "https://working.example/dns-query",
            ):
                self.assertEqual(backend.configured_working_probe_resolvers(paths), [
                    "https://working.example/dns-query",
                ])

    def test_address_resolution_falls_back_to_second_resolver_and_keeps_ipv6(self):
        def records(_host, resolver, record_type):
            if resolver == "https://broken.example/dns-query":
                return ()
            return ("8.8.8.8",) if record_type == 1 else ("2001:4860:4860::8888",)

        with mock.patch.object(backend, "resolve_probe_records", side_effect=records), \
             mock.patch.object(backend, "system_probe_addresses") as system:
            self.assertEqual(backend.resolve_probe_addresses("node.example", [
                "https://broken.example/dns-query",
                "https://working.example/dns-query",
            ]), ["8.8.8.8", "2001:4860:4860::8888"])
        system.assert_not_called()

    def test_remote_subscription_requires_https_but_loopback_http_is_allowed(self):
        with self.assertRaisesRegex(backend.BackendError, "must use HTTPS"):
            backend.validate_subscription_url("http://provider.example/sub")
        self.assertEqual(
            backend.validate_subscription_url("http://127.0.0.1:8765/sub"),
            "http://127.0.0.1:8765/sub",
        )

    def test_subscription_refresh_preserves_ids_and_keeps_missing_active_profile(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            backend.ensure_private_dir(paths.config_dir)
            second = REALITY_URI.replace("example.com:443", "two.example:8443").replace(
                "#Example", "#Second"
            )
            with mock.patch.object(backend, "fetch_subscription", return_value=REALITY_URI + "\n" + second):
                backend.save_subscription(paths, "Provider", "", "https://provider.example/s")
            first = backend.load_store(paths)
            subscription_id = first["subscriptions"][0]["id"]
            active = next(profile for profile in first["profiles"] if profile["name"] == "Example")
            remaining = next(profile for profile in first["profiles"] if profile["name"] == "Second")
            first["activeId"] = active["id"]
            first["lastId"] = active["id"]
            remaining["favorite"] = True
            backend.save_store(paths, first)

            with mock.patch.object(backend, "fetch_subscription", return_value=second):
                result = backend.refresh_subscription(paths, subscription_id)
            self.assertEqual(result["added"], 0)
            self.assertEqual(result["stale"], 1)
            refreshed = backend.load_store(paths)
            by_id = {profile["id"]: profile for profile in refreshed["profiles"]}
            self.assertIn(active["id"], by_id)
            self.assertTrue(by_id[active["id"]]["missing"])
            self.assertIn(remaining["id"], by_id)
            self.assertFalse(by_id[remaining["id"]]["missing"])
            self.assertTrue(by_id[remaining["id"]]["favorite"])

    def test_profile_favorites_persist_and_are_public_only_as_a_boolean(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            backend.save_store(paths, {
                "version": 2, "activeId": "", "lastId": profile_id,
                "profiles": [{"id": profile_id, "name": "Example", "uri": REALITY_URI}],
                "subscriptions": [], "routingPreset": "roscomvpn-default",
                "customRules": [], "rulesUpdatedAt": 0,
                "startupConfigured": True,
                "startup": {"enabled": False, "target": "last", "profileId": "", "mode": "rule"},
                "onboardingComplete": True,
            })
            self.assertFalse(backend.load_store(paths)["profiles"][0]["favorite"])
            backend.set_profile_favorite(paths, profile_id, True)
            self.assertTrue(backend.load_store(paths)["profiles"][0]["favorite"])
            with mock.patch.object(backend, "service_active", return_value=False):
                status = json.loads(backend.status_text(paths))
            self.assertTrue(status["profiles"][0]["favorite"])
            self.assertNotIn("uri", status["profiles"][0])

    def test_safe_diagnostics_omit_profiles_keys_and_subscription_urls(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            subscription_id = "33333333-3333-4333-8333-333333333333"
            backend.save_store(paths, {
                "version": 2, "activeId": "", "lastId": profile_id,
                "profiles": [{
                    "id": profile_id, "name": "Secret server", "uri": REALITY_URI,
                    "favorite": True,
                }],
                "subscriptions": [{
                    "id": subscription_id, "name": "Private provider",
                    "url": "https://provider.example/bearer-token", "updatedAt": 1234,
                }],
                "routingPreset": "roscomvpn-default", "customRules": [],
                "rulesUpdatedAt": 5678, "startupConfigured": True,
                "startup": {"enabled": False, "target": "last", "profileId": "", "mode": "rule"},
                "onboardingComplete": True,
            })
            paths.template.write_text(
                (ROOT / "templates" / "default.yaml").read_text(encoding="utf-8"),
                encoding="utf-8",
            )
            with mock.patch.object(backend, "service_active", return_value=False), \
                 mock.patch.object(backend, "service_enabled", return_value=False), \
                 mock.patch.object(backend, "core_setup_status", return_value={
                     "installed": True, "tunReady": True, "path": "/secret/path/mihomo",
                 }), \
                 mock.patch.object(backend, "routing_conflicts", return_value=["private-app"]):
                text = backend.diagnostics_text(paths)
                destination = home / "diagnostics.json"
                backend.export_diagnostics(paths, str(destination))
            payload = json.loads(text)
            self.assertEqual(payload["inventory"], {
                "profiles": 1, "favorites": 1, "subscriptions": 1, "customRules": 0,
            })
            for secret in (
                profile_id, subscription_id, "11111111-1111-4111-8111-111111111111",
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "Secret server",
                "Private provider", "provider.example", "example.com", "/secret/path",
                "private-app", "vless://", "https://",
            ):
                self.assertNotIn(secret, text)
            self.assertIsNone(re.search(
                r"(?i)[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}|https?://|vless://",
                text,
            ))
            exported = json.loads(destination.read_text(encoding="utf-8"))
            exported.pop("generatedAt")
            comparable = dict(payload)
            comparable.pop("generatedAt")
            self.assertEqual(exported, comparable)
            self.assertEqual(stat.S_IMODE(destination.stat().st_mode), 0o600)

    def test_subscription_delete_refuses_an_active_managed_profile(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            backend.ensure_private_dir(paths.config_dir)
            with mock.patch.object(backend, "fetch_subscription", return_value=REALITY_URI):
                backend.save_subscription(paths, "Provider", "", "https://provider.example/s")
            store = backend.load_store(paths)
            store["activeId"] = store["profiles"][0]["id"]
            backend.save_store(paths, store)
            with mock.patch.object(backend, "service_active", return_value=True):
                with self.assertRaisesRegex(backend.BackendError, "Disconnect"):
                    backend.delete_subscription(paths, store["subscriptions"][0]["id"])

    def test_disconnect_prunes_a_stale_subscription_profile(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            backend.ensure_private_dir(paths.config_dir)
            with mock.patch.object(backend, "fetch_subscription", return_value=REALITY_URI):
                backend.save_subscription(paths, "Provider", "", "https://provider.example/s")
            store = backend.load_store(paths)
            profile_id = store["profiles"][0]["id"]
            store["profiles"][0]["missing"] = True
            store["activeId"] = profile_id
            store["lastId"] = profile_id
            backend.save_store(paths, store)
            stopped = subprocess.CompletedProcess([], 0, "", "")
            with mock.patch.object(backend, "systemctl", return_value=stopped), \
                 mock.patch.object(backend, "service_active", return_value=False):
                backend.stop_service(paths, profile_id)
            cleaned = backend.load_store(paths)
            self.assertEqual(cleaned["profiles"], [])
            self.assertEqual(cleaned["activeId"], "")
            self.assertEqual(cleaned["lastId"], "")

    def test_subscription_credentials_travel_over_stdin_not_argv(self):
        service = (ROOT / "Service.qml").read_text(encoding="utf-8")
        backend_source = (ROOT / "backend.py").read_text(encoding="utf-8")
        self.assertIn('runControl(["subscription-save"', service)
        self.assertIn('read_stdin_text(MAX_SUBSCRIPTION_URL_BYTES, "subscription URL")', backend_source)

    def test_legacy_store_template_and_last_selection_migrate(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            env, _ = self.make_env(home)
            profile_id = "22222222-2222-4222-8222-222222222222"
            legacy = home / ".config" / "omarchy" / "omavless"
            legacy.mkdir(parents=True)
            (legacy / "profiles.json").write_text(json.dumps({
                "version": 1,
                "activeId": "",
                "lastId": "",
                "profiles": [{"id": profile_id, "name": "Example", "uri": REALITY_URI}],
            }), encoding="utf-8")
            template = "mixed-port: 7890\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n- MATCH,PROXY\n"
            (legacy / "route-template.yaml").write_text(template, encoding="utf-8")
            legacy_last = home / ".local" / "state" / "omarchy" / "vless-last"
            legacy_last.parent.mkdir(parents=True)
            legacy_last.write_text("Example\n", encoding="utf-8")

            result = subprocess.run(
                [str(ROOT / "backend.sh"), "status"], env=env, text=True, capture_output=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            current = home / ".config" / "omavless"
            store = current / "profiles.json"
            self.assertEqual(stat.S_IMODE(current.stat().st_mode), 0o700)
            self.assertEqual(stat.S_IMODE(store.stat().st_mode), 0o600)
            self.assertEqual(stat.S_IMODE((current / "route-template.yaml").stat().st_mode), 0o600)
            self.assertEqual(json.loads(store.read_text(encoding="utf-8"))["lastId"], profile_id)
            self.assertEqual(json.loads(result.stdout)["lastId"], profile_id)
            self.assertFalse((legacy / "profiles.json").exists())
            self.assertFalse((legacy / "route-template.yaml").exists())
            self.assertFalse(legacy_last.exists())

    def test_migration_never_overwrites_different_current_data(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            env, _ = self.make_env(home)
            legacy = home / ".config" / "omarchy" / "omavless"
            current = home / ".config" / "omavless"
            legacy.mkdir(parents=True)
            current.mkdir(parents=True)
            old = legacy / "profiles.json"
            new = current / "profiles.json"
            old.write_text('{"profiles": [], "source": "old"}\n', encoding="utf-8")
            new.write_text('{"profiles": [], "source": "new"}\n', encoding="utf-8")

            result = subprocess.run(
                [str(ROOT / "backend.sh"), "status"], env=env, text=True, capture_output=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("exist and differ", result.stderr)
            self.assertTrue(old.exists())
            self.assertTrue(new.exists())

    def test_status_poll_is_shared_across_processes(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            counter = home / "systemctl-count"
            env, _ = self.make_env(
                home,
                '#!/bin/sh\nprintf x >> "$OMAVLESS_COUNT"\nexit 3\n',
            )
            env["OMAVLESS_COUNT"] = str(counter)
            for _ in range(2):
                result = subprocess.run(
                    [str(ROOT / "backend.sh"), "status"], env=env,
                    text=True, capture_output=True,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
            # One generated snapshot may inspect the OmaVLESS unit, a known
            # competing unit and (when another TUN exists) the service PID.
            # The important contract is that concurrent callers share that
            # one bounded batch instead of multiplying it per monitor.
            self.assertIn(counter.read_text(encoding="utf-8"), {"x", "xx", "xxx"})

    def test_json_status_preserves_punctuation_without_custom_escaping(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            paths.store.write_text(json.dumps({
                "version": 1, "activeId": profile_id, "lastId": profile_id,
                "profiles": [{"id": profile_id, "name": r"Home:Lab\Edge", "uri": REALITY_URI}],
            }), encoding="utf-8")
            with mock.patch.object(backend, "service_active", return_value=True):
                status = json.loads(backend.status_text(paths))
            self.assertEqual(status["profiles"][0]["name"], r"Home:Lab\Edge")
            self.assertEqual(status["profiles"][0]["device"], backend.TUN_DEVICE)
            self.assertTrue(status["profiles"][0]["active"])

    def test_interface_addresses_come_from_device_instead_of_fake_ip_range(self):
        ip_output = json.dumps([{"addr_info": [
            {"family": "inet", "local": "10.10.0.2", "prefixlen": 30, "scope": "global"},
            {"family": "inet6", "local": "fd00::2", "prefixlen": 64, "scope": "global"},
            {"family": "inet6", "local": "fe80::1", "prefixlen": 64, "scope": "link"},
        ]}])
        completed = subprocess.CompletedProcess([], 0, ip_output, "")
        with mock.patch.object(backend.shutil, "which", return_value="/usr/bin/ip"), \
             mock.patch.object(backend, "run", return_value=completed) as invoked:
            self.assertEqual(
                backend.interface_addresses("Meta"),
                ["10.10.0.2/30", "fd00::2/64"],
            )
        invoked.assert_called_once_with(
            ["/usr/bin/ip", "-j", "address", "show", "dev", "Meta"], check=False,
        )

    def test_details_protocol_is_versioned_json(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            paths.store.write_text(json.dumps({
                "version": 1, "activeId": profile_id, "lastId": profile_id,
                "profiles": [{"id": profile_id, "name": "Example", "uri": REALITY_URI}],
            }), encoding="utf-8")
            output = io.StringIO()
            with mock.patch.object(backend, "interface_addresses", return_value=["10.10.0.2/30"]), \
                 mock.patch.object(sys, "stdout", output):
                backend.details(paths, profile_id, "Meta")
            payload = json.loads(output.getvalue())
            self.assertEqual(payload, {
                "version": 1,
                "address": "10.10.0.2/30",
                "server": "example.com:443",
                "transport": "tcp / reality",
                "sni": "example.org",
            })

    def test_operation_lock_serializes_processes(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            env, runtime = self.make_env(home)
            # Paths.current reads the process environment, so construct the
            # test value explicitly rather than mutating global os.environ.
            config_dir = home / ".config" / "omavless"
            paths = backend.Paths(
                home, config_dir, config_dir / "profiles.json",
                config_dir / "route-template.yaml", config_dir / "config.yaml",
                home / ".config" / "systemd" / "user" / backend.SERVICE,
                home / ".config" / "omarchy" / "omavless",
                home / ".local" / "state" / "omarchy" / "vless-last", runtime,
            )
            config_dir.mkdir(parents=True)
            with backend.operation_lock(paths):
                child = subprocess.Popen(
                    [str(ROOT / "backend.sh"), "import", "Example"],
                    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                    text=True, env=env,
                )
                assert child.stdin is not None
                child.stdin.write(REALITY_URI)
                child.stdin.close()
                child.stdin = None
                time.sleep(0.15)
                self.assertIsNone(child.poll(), "mutation ignored the cross-process lock")
            stdout, stderr = child.communicate(timeout=5)
            self.assertEqual(child.returncode, 0, stderr or stdout)

    def test_operation_lock_and_child_commands_have_timeouts(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            with mock.patch.object(backend.fcntl, "flock", side_effect=BlockingIOError), \
                 mock.patch.object(backend.time, "monotonic", side_effect=[0.0, 1.0]), \
                 mock.patch.object(backend.time, "sleep"):
                with self.assertRaisesRegex(backend.BackendError, "taking too long"):
                    with backend.operation_lock(paths, timeout=0.5):
                        self.fail("lock unexpectedly acquired")

        expired = subprocess.TimeoutExpired(["mihomo", "-t"], 30)
        with mock.patch.object(backend.subprocess, "run", side_effect=expired):
            with self.assertRaisesRegex(backend.BackendError, "mihomo timed out after 30 seconds"):
                backend.run(["mihomo", "-t"])

    def test_no_privileged_or_crontab_mutation_in_plugin_code(self):
        command_sources = "\n".join(
            (ROOT / name).read_text(encoding="utf-8")
            for name in ("backend.py", "backend.sh", "install.sh", "uninstall.sh", "Service.qml")
        )
        for forbidden in ("NOPASSWD", "/etc/sudoers", "crontab", "pkexec", "sudo "):
            self.assertNotIn(forbidden, command_sources)
        # The one privileged setup step is user-mediated text: the panel may
        # copy it to the clipboard, but neither QML service nor backend can
        # execute it on the user's behalf.
        panel = (ROOT / "Panel.qml").read_text(encoding="utf-8")
        self.assertIn('mihomoCapabilityCommand: "sudo setcap ', panel)
        self.assertIn("onCopyCommand: function(command) { vless.copyText(command) }", panel)
        self.assertNotIn("runControl([root.mihomoCapabilityCommand", panel)

    def test_new_and_migrated_stores_get_safe_setup_defaults(self):
        fresh = backend.empty_store()
        self.assertTrue(fresh["startupConfigured"])
        self.assertFalse(fresh["startup"]["enabled"])
        self.assertFalse(fresh["onboardingComplete"])

        migrated = backend.validate_store({
            "version": 1, "activeId": "", "lastId": "",
            "profiles": [{
                "id": "22222222-2222-4222-8222-222222222222",
                "name": "Example", "uri": REALITY_URI,
            }],
        })
        self.assertFalse(migrated["startupConfigured"])
        self.assertFalse(migrated["startup"]["enabled"])
        self.assertTrue(migrated["onboardingComplete"])

    def test_core_setup_status_requires_all_tun_capabilities(self):
        core = Path("/usr/bin/mihomo")
        completed = subprocess.CompletedProcess(
            [], 0,
            "/usr/bin/mihomo cap_net_admin,cap_net_raw,cap_net_bind_service=ep\n",
            "",
        )
        with mock.patch.object(backend, "find_core", return_value=core), \
             mock.patch.object(backend.shutil, "which", return_value="/usr/bin/getcap"), \
             mock.patch.object(backend, "run", return_value=completed):
            self.assertEqual(backend.core_setup_status(self.paths_for(Path("/tmp"))), {
                "installed": True, "tunReady": True, "path": str(core),
            })
        missing = subprocess.CompletedProcess([], 0, "/usr/bin/mihomo cap_net_admin=ep\n", "")
        with mock.patch.object(backend, "find_core", return_value=core), \
             mock.patch.object(backend.shutil, "which", return_value="/usr/bin/getcap"), \
             mock.patch.object(backend, "run", return_value=missing):
            self.assertFalse(backend.core_setup_status(self.paths_for(Path("/tmp")))["tunReady"])

    def test_configure_startup_validates_and_enables_helper_transactionally(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            store = backend.empty_store()
            store["profiles"] = [{"id": profile_id, "name": "Example", "uri": REALITY_URI}]
            store["routingPreset"] = "roscomvpn-default"
            backend.save_store(paths, store)
            candidate = paths.config_dir / ".startup-candidate.yaml"
            candidate.write_text("candidate", encoding="utf-8")
            ok = subprocess.CompletedProcess([], 0, "", "")
            with mock.patch.object(backend, "service_enabled", side_effect=[True, False]), \
                 mock.patch.object(backend, "core_setup_status", return_value={
                     "installed": True, "tunReady": True, "path": "/usr/bin/mihomo",
                 }), \
                 mock.patch.object(backend, "find_core", return_value=Path("/usr/bin/mihomo")), \
                 mock.patch.object(backend, "ensure_unit"), \
                 mock.patch.object(backend, "ensure_startup_unit"), \
                 mock.patch.object(backend, "test_config", return_value=candidate), \
                 mock.patch.object(backend, "systemctl", return_value=ok) as systemctl:
                backend.configure_startup(paths, True, "profile", profile_id, "rule")
            saved = backend.load_store(paths)
            self.assertEqual(saved["startup"], {
                "enabled": True, "target": "profile", "profileId": profile_id,
                "mode": "rule",
            })
            self.assertFalse(candidate.exists())
            self.assertIn(mock.call("enable", backend.STARTUP_SERVICE, check=False), systemctl.call_args_list)
            self.assertIn(mock.call("disable", backend.SERVICE, check=False), systemctl.call_args_list)

    def test_disabling_startup_is_safe_before_core_or_main_unit_exists(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            backend.save_store(paths, backend.empty_store())
            with mock.patch.object(backend, "service_enabled", return_value=False), \
                 mock.patch.object(backend, "ensure_startup_unit"), \
                 mock.patch.object(backend, "systemctl") as systemctl:
                backend.configure_startup(paths, False, "last", "", "global")
            systemctl.assert_not_called()
            saved = backend.load_store(paths)
            self.assertFalse(saved["startup"]["enabled"])
            self.assertEqual(saved["startup"]["mode"], "global")

    def test_first_profile_is_a_safe_last_used_fallback(self):
        store = backend.empty_store()
        profile_id = "22222222-2222-4222-8222-222222222222"
        store["profiles"] = [{"id": profile_id, "name": "Example", "uri": REALITY_URI}]
        self.assertEqual(
            backend.resolve_startup_profile(store, "last", "")["id"], profile_id
        )

    def test_configure_startup_rejects_routing_without_a_preset(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            store = backend.empty_store()
            store["profiles"] = [{"id": profile_id, "name": "Example", "uri": REALITY_URI}]
            backend.save_store(paths, store)
            with mock.patch.object(backend, "service_enabled", return_value=False):
                with self.assertRaisesRegex(backend.BackendError, "country preset"):
                    backend.configure_startup(paths, True, "profile", profile_id, "rule")

    def test_startup_connect_uses_selected_profile_and_mode(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            store = backend.empty_store()
            store["profiles"] = [{"id": profile_id, "name": "Example", "uri": REALITY_URI}]
            store["startup"] = {
                "enabled": True, "target": "profile", "profileId": profile_id,
                "mode": "global",
            }
            backend.save_store(paths, store)
            candidate = paths.config_dir / ".startup-candidate.yaml"
            candidate.write_text("mode: global\n", encoding="utf-8")
            ok = subprocess.CompletedProcess([], 0, "", "")
            with mock.patch.object(backend, "service_active", side_effect=[False, False, True]), \
                 mock.patch.object(backend, "find_core", return_value=Path("/usr/bin/mihomo")), \
                 mock.patch.object(backend, "ensure_unit"), \
                 mock.patch.object(backend, "render_config_mode", return_value="mode: global\n") as render, \
                 mock.patch.object(backend, "test_config", return_value=candidate), \
                 mock.patch.object(backend, "systemctl", return_value=ok) as systemctl, \
                 mock.patch.object(backend, "mark_active"):
                backend.startup_connect(paths)
            render.assert_called_once_with(paths, mock.ANY, "global", mock.ANY)
            systemctl.assert_called_once_with("start", backend.SERVICE)
            saved = backend.load_store(paths)
            self.assertEqual(saved["activeId"], profile_id)
            self.assertEqual(saved["lastId"], profile_id)
            self.assertEqual(paths.config.read_text(encoding="utf-8"), "mode: global\n")

    def test_explicit_startup_disconnect_stops_without_disabling_login(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            store = backend.empty_store()
            store["activeId"] = profile_id
            store["lastId"] = profile_id
            store["profiles"] = [{"id": profile_id, "name": "Example", "uri": REALITY_URI}]
            backend.save_store(paths, store)
            ok = subprocess.CompletedProcess([], 0, "", "")
            with mock.patch.object(backend, "mark_intent"), \
                 mock.patch.object(backend, "systemctl", return_value=ok) as systemctl:
                backend.stop_service(paths, profile_id)
            systemctl.assert_called_once_with("stop", backend.SERVICE, check=False)
            self.assertEqual(backend.load_store(paths)["activeId"], "")

    def test_bundled_config_enables_rule_tun(self):
        template = (ROOT / "templates" / "default.yaml").read_text(encoding="utf-8")
        for expected in (
            "mode: rule", "tun:\n  enable: true", "auto-route: true",
            "auto-detect-interface: true", "strict-route: true",
        ):
            self.assertIn(expected, template)
        self.assertNotIn("external-controller", template)
        self.assertIn("default-nameserver:", template)

    def test_store_validation_secures_mode_and_rejects_duplicate_ids(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            profile = {"id": profile_id, "name": "Example", "uri": REALITY_URI}
            paths.store.write_text(json.dumps({
                "version": 1, "activeId": "", "lastId": profile_id,
                "profiles": [profile],
            }), encoding="utf-8")
            paths.store.chmod(0o644)
            self.assertEqual(backend.load_store(paths)["profiles"][0]["name"], "Example")
            self.assertEqual(stat.S_IMODE(paths.store.stat().st_mode), 0o600)

            paths.store.write_text(json.dumps({
                "version": 1, "activeId": "", "lastId": "",
                "profiles": [profile, profile],
            }), encoding="utf-8")
            with self.assertRaisesRegex(backend.BackendError, "duplicate id"):
                backend.load_store(paths)

    def test_store_refuses_symlink_and_self_heals_stale_selection(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            external = home / "external.json"
            external.write_text(json.dumps(backend.empty_store()), encoding="utf-8")
            paths.store.symlink_to(external)
            with self.assertRaisesRegex(backend.BackendError, "symlinked"):
                backend.load_store(paths)
            paths.store.unlink()

            data = backend.empty_store()
            data["lastId"] = "33333333-3333-4333-8333-333333333333"
            paths.store.write_text(json.dumps(data), encoding="utf-8")
            self.assertEqual(backend.load_store(paths)["lastId"], "")

    def test_oversized_vless_link_is_rejected_before_parsing(self):
        oversized = REALITY_URI + ("a" * backend.MAX_VLESS_URI_BYTES)
        with self.assertRaisesRegex(backend.BackendError, "too large"):
            backend.parse_vless(oversized)

    def test_vless_rejects_unbounded_query_and_unsupported_crypto(self):
        base, fragment = REALITY_URI.split("#", 1)
        many_fields = base + "&" + "&".join(f"x{i}=1" for i in range(130)) + "#" + fragment
        with self.assertRaisesRegex(backend.BackendError, "Max number of fields exceeded"):
            backend.parse_vless(many_fields)
        with self.assertRaisesRegex(backend.BackendError, "encryption must be none"):
            backend.parse_vless(REALITY_URI.replace("encryption=none", "encryption=aes-128-gcm"))
        with self.assertRaisesRegex(backend.BackendError, "Unsupported VLESS flow"):
            backend.parse_vless(REALITY_URI.replace("xtls-rprx-vision", "made-up-flow"))
        with self.assertRaisesRegex(backend.BackendError, "requires TLS or Reality"):
            backend.parse_vless(REALITY_URI.replace("security=reality", "security=none"))

    def test_stdin_reader_is_utf8_and_byte_bounded(self):
        with mock.patch.object(sys, "stdin", io.TextIOWrapper(io.BytesIO(b"ok"), encoding="utf-8")):
            self.assertEqual(backend.read_stdin_text(2, "input"), "ok")
        with mock.patch.object(sys, "stdin", io.TextIOWrapper(io.BytesIO(b"three"), encoding="utf-8")):
            with self.assertRaisesRegex(backend.BackendError, "too large"):
                backend.read_stdin_text(2, "input")
        with mock.patch.object(sys, "stdin", io.TextIOWrapper(io.BytesIO(b"\xff"), encoding="utf-8")):
            with self.assertRaisesRegex(backend.BackendError, "as UTF-8"):
                backend.read_stdin_text(8, "input")

    def test_editor_output_is_bounded_before_returning_to_qml(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            paths.store.write_text(json.dumps({
                "version": 1, "activeId": "", "lastId": profile_id,
                "profiles": [{"id": profile_id, "name": "Example", "uri": REALITY_URI}],
            }), encoding="utf-8")
            result = subprocess.CompletedProcess([], 0, "x" * (backend.MAX_IMPORT_BYTES + 1), "")
            with mock.patch.object(backend.shutil, "which", return_value="/usr/bin/zenity"), \
                 mock.patch.object(backend, "run", return_value=result):
                with self.assertRaisesRegex(backend.BackendError, "Edited VLESS input is too large"):
                    backend.edit_profile(paths, profile_id, "Example", "")

    def test_running_service_without_profile_is_reported_as_inconsistent(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            paths.store.write_text(json.dumps(backend.empty_store()), encoding="utf-8")
            with mock.patch.object(backend, "service_active", return_value=True):
                with self.assertRaisesRegex(backend.BackendError, "without an active profile"):
                    backend.status_text(paths)

    def test_systemd_unit_refuses_symlink_and_escapes_specifiers(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.unit.parent.mkdir(parents=True)
            target = home / "foreign.service"
            target.write_text("foreign", encoding="utf-8")
            paths.unit.symlink_to(target)
            with self.assertRaisesRegex(backend.BackendError, "symlinked systemd unit"):
                backend.ensure_unit(paths, home / "mihomo")
        self.assertEqual(backend.systemd_quote('/tmp/100%/a"b'), '"/tmp/100%%/a\\"b"')

    def test_startup_unit_is_user_scoped_and_refuses_symlinks(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            helper = backend.startup_unit(paths)
            helper.parent.mkdir(parents=True)
            ok = subprocess.CompletedProcess([], 0, "", "")
            with mock.patch.object(backend, "systemctl", return_value=ok):
                backend.ensure_startup_unit(paths)
            text = helper.read_text(encoding="utf-8")
            self.assertIn("ExecStart=", text)
            self.assertIn(" startup-connect", text)
            self.assertIn("WantedBy=default.target", text)
            self.assertNotIn("User=", text)
            helper.unlink()
            foreign = home / "foreign.service"
            foreign.write_text("foreign", encoding="utf-8")
            helper.symlink_to(foreign)
            with self.assertRaisesRegex(backend.BackendError, "symlinked systemd unit"):
                backend.ensure_startup_unit(paths)

    def test_stop_failure_preserves_active_state_and_clears_intent(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            systemctl = '''#!/bin/sh
case "$*" in
  *"disable --now omavless.service"*) echo "stop refused" >&2; exit 1 ;;
  *"is-active --quiet omavless.service"*) exit 0 ;;
  *) exit 3 ;;
esac
'''
            env, runtime = self.make_env(home, systemctl)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            paths.store.write_text(json.dumps({
                "version": 1, "activeId": profile_id, "lastId": profile_id,
                "profiles": [{"id": profile_id, "name": "Example", "uri": REALITY_URI}],
            }), encoding="utf-8")
            result = subprocess.run(
                [str(ROOT / "backend.sh"), "down-all"], env=env,
                text=True, capture_output=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("stop refused", result.stderr)
            self.assertEqual(json.loads(paths.store.read_text(encoding="utf-8"))["activeId"], profile_id)
            self.assertFalse((runtime / f"omavless.{os.getuid()}.intent").exists())

    def test_stale_row_cannot_disconnect_a_newly_selected_profile(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            active_id = "22222222-2222-4222-8222-222222222222"
            stale_id = "33333333-3333-4333-8333-333333333333"
            paths.store.write_text(json.dumps({
                "version": 1, "activeId": active_id, "lastId": active_id,
                "profiles": [
                    {"id": active_id, "name": "Active", "uri": REALITY_URI},
                    {"id": stale_id, "name": "Stale", "uri": REALITY_URI},
                ],
            }), encoding="utf-8")
            with mock.patch.object(backend, "systemctl") as systemctl:
                with self.assertRaisesRegex(backend.BackendError, "no longer active"):
                    backend.stop_service(paths, stale_id)
            systemctl.assert_not_called()

    def test_delayed_active_observation_cannot_clear_newer_disconnect_intent(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            profile_id = "22222222-2222-4222-8222-222222222222"
            intent = runtime / f"omavless.{os.getuid()}.intent"
            intent.write_text(f"{profile_id} 2000000000000\n", encoding="utf-8")
            intent.chmod(0o600)

            backend.clear_intent(paths, profile_id, 1999999999999)
            self.assertTrue(intent.exists())
            backend.clear_intent(paths, profile_id, 2000000000000)
            self.assertFalse(intent.exists())

    def test_runtime_cleanup_reaps_dead_owner_but_keeps_live_owner(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            live = runtime / f"vless-qr.{os.getpid()}.live.png"
            dead = runtime / "vless-edit.999999999.dead"
            live.write_bytes(b"live")
            dead.write_bytes(b"dead")

            backend.cleanup_runtime(paths)
            self.assertTrue(live.exists())
            self.assertFalse(dead.exists())

    def test_external_drop_is_deduplicated_across_monitors_and_rearmed(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            profile_id = "22222222-2222-4222-8222-222222222222"
            with mock.patch.object(backend.shutil, "which", return_value=None):
                self.assertEqual(backend.notify_drop(paths, profile_id, "Example"), 0)
                self.assertEqual(backend.notify_drop(paths, profile_id, "Example"), 2)
                backend.mark_active(paths, profile_id, time.time_ns() // 1_000_000)
                self.assertEqual(backend.notify_drop(paths, profile_id, "Example"), 0)

    def test_connect_failure_restores_config_and_reports_recovery(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            paths.store.write_text(json.dumps({
                "version": 1, "activeId": "", "lastId": "",
                "profiles": [{"id": profile_id, "name": "Example", "uri": REALITY_URI}],
            }), encoding="utf-8")
            candidate = paths.config_dir / ".candidate.yaml"
            candidate.write_text("candidate", encoding="utf-8")
            ok = subprocess.CompletedProcess([], 0, "", "")
            with mock.patch.object(backend, "find_core", return_value=home / "mihomo"), \
                 mock.patch.object(backend, "ensure_unit"), \
                 mock.patch.object(backend, "test_config", return_value=candidate), \
                 mock.patch.object(backend, "systemctl", return_value=ok), \
                 mock.patch.object(backend, "service_active", side_effect=[False, False, False, False]):
                with self.assertRaises(backend.BackendError) as raised:
                    backend.connect_profile(paths, profile_id)
            self.assertEqual(raised.exception.exit_code, 20)
            self.assertIn("previous OmaVLESS state restored", str(raised.exception))
            self.assertFalse(paths.config.exists())
            self.assertEqual(json.loads(paths.store.read_text(encoding="utf-8"))["activeId"], "")

    def test_connect_failure_reports_incomplete_rollback(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            paths.store.write_text(json.dumps({
                "version": 1, "activeId": "", "lastId": "",
                "profiles": [{"id": profile_id, "name": "Example", "uri": REALITY_URI}],
            }), encoding="utf-8")
            candidate = paths.config_dir / ".candidate.yaml"
            candidate.write_text("candidate", encoding="utf-8")
            ok = subprocess.CompletedProcess([], 0, "", "")
            with mock.patch.object(backend, "find_core", return_value=home / "mihomo"), \
                 mock.patch.object(backend, "ensure_unit"), \
                 mock.patch.object(backend, "test_config", return_value=candidate), \
                 mock.patch.object(backend, "systemctl", return_value=ok), \
                 mock.patch.object(backend, "service_active", side_effect=[False, False, False, True]):
                with self.assertRaises(backend.BackendError) as raised:
                    backend.connect_profile(paths, profile_id)
            self.assertEqual(raised.exception.exit_code, 21)
            self.assertIn("manual recovery", str(raised.exception))

    def test_adopt_template_failure_never_overwrites_current_template(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            env, _ = self.make_env(home)
            config_dir = home / ".config" / "omavless"
            config_dir.mkdir(parents=True, mode=0o700)
            current = config_dir / "route-template.yaml"
            original = "proxies:\n{{OMAVLESS_PROXY}}\nrules:\n- MATCH,PROXY\n"
            current.write_text(original, encoding="utf-8")
            source = home / "routing.yaml"
            source.write_text(
                "proxies: []\nrules:\n- MATCH,PROXY\n# {{OMAVLESS_PROXY}}\n",
                encoding="utf-8",
            )
            result = subprocess.run(
                [str(ROOT / "backend.sh"), "adopt-template", str(source)],
                env=env, text=True, capture_output=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("exactly one", result.stderr)
            self.assertEqual(current.read_text(encoding="utf-8"), original)

    def test_raw_credential_export_command_is_not_exposed(self):
        result = subprocess.run(
            [str(ROOT / "backend.sh"), "export", "unused"],
            text=True, capture_output=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("vless://", result.stdout)

    def test_default_template_ignores_other_clients_config(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            foreign = home / ".config" / "mihomo" / "config.yaml"
            foreign.parent.mkdir(parents=True)
            foreign.write_text("foreign-client-marker: true\nproxies: []\n", encoding="utf-8")
            config_dir = home / ".config" / "omavless"
            paths = backend.Paths(
                home, config_dir, config_dir / "profiles.json",
                config_dir / "route-template.yaml", config_dir / "config.yaml",
                home / ".config" / "systemd" / "user" / backend.SERVICE,
                home / ".config" / "omarchy" / "omavless",
                home / ".local" / "state" / "omarchy" / "vless-last", runtime,
            )
            backend.ensure_private_dir(config_dir)
            backend.ensure_template(paths)
            adopted = paths.template.read_text(encoding="utf-8")
            self.assertNotIn("foreign-client-marker", adopted)
            self.assertEqual(
                adopted,
                (ROOT / "templates" / "default.yaml").read_text(encoding="utf-8"),
            )

    def test_core_discovery_does_not_read_mihoro_config(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            foreign_core = home / "foreign-mihomo"
            foreign_core.write_text("#!/bin/sh\n", encoding="utf-8")
            foreign_core.chmod(0o755)
            mihoro_toml = home / ".config" / "mihoro.toml"
            mihoro_toml.parent.mkdir(parents=True)
            mihoro_toml.write_text(
                f'mihomo_binary_path = "{foreign_core}"\n', encoding="utf-8",
            )
            config_dir = home / ".config" / "omavless"
            paths = backend.Paths(
                home, config_dir, config_dir / "profiles.json",
                config_dir / "route-template.yaml", config_dir / "config.yaml",
                home / ".config" / "systemd" / "user" / backend.SERVICE,
                home / ".config" / "omarchy" / "omavless",
                home / ".local" / "state" / "omarchy" / "vless-last", runtime,
            )
            old_override = os.environ.pop("OMAVLESS_MIHOMO", None)
            try:
                with mock.patch.object(backend.shutil, "which", return_value=None):
                    with self.assertRaises(backend.BackendError):
                        backend.find_core(paths)
            finally:
                if old_override is not None:
                    os.environ["OMAVLESS_MIHOMO"] = old_override

    def test_distribution_has_no_embedded_profile_or_implicit_mihoro_config(self):
        distributed = [
            ROOT / name for name in (
                "Panel.qml", "Service.qml", "NamePrompt.qml", "SubscriptionPrompt.qml",
                "RoutingPresetPrompt.qml", "OnboardingWizard.qml", "StartupPrompt.qml",
                "RoutingToolsPrompt.qml", "ImportPreviewPrompt.qml", "RenameWindow.qml",
                "QrWindow.qml", "backend.py", "backend.sh", "install.sh",
                "uninstall.sh",
                "manifest.json", "README.md", "CHANGELOG.md", "LICENSE", "THIRD_PARTY_NOTICES.md",
                "templates/default.yaml", "templates/china.yaml", "templates/iran.yaml",
            )
        ]
        texts = {path: path.read_text(encoding="utf-8") for path in distributed}
        credential = re.compile(r"vless://[0-9a-fA-F-]{36}@")
        for path, text in texts.items():
            self.assertIsNone(credential.search(text), f"embedded VLESS credential in {path}")
        backend_source = (ROOT / "backend.py").read_text(encoding="utf-8")
        self.assertNotIn('"mihoro.toml"', backend_source)
        self.assertNotIn('"mihomo" / "config.yaml"', backend_source)

    def test_ui_reference_mit_attribution_is_preserved(self):
        license_text = (ROOT / "LICENSE").read_text(encoding="utf-8")
        notice = (ROOT / "THIRD_PARTY_NOTICES.md").read_text(encoding="utf-8")
        self.assertIn("Copyright (c) 2026 Justin Köstinger", license_text)
        self.assertIn("Copyright (c) 2026 OmaVLESS contributors", license_text)
        self.assertIn("https://github.com/jkoestinger/omarchy-vpn", notice)
        self.assertIn("https://omarchyplugins.com/plugin.html?id=jkoestinger.vpn", notice)
        for name in (
            "Panel.qml", "Service.qml", "NamePrompt.qml", "ImportPreviewPrompt.qml",
            "RenameWindow.qml", "QrWindow.qml",
        ):
            source = (ROOT / name).read_text(encoding="utf-8")
            self.assertIn("SPDX-License-Identifier: MIT", source)
            self.assertIn("Adapted from Omarchy VPN", source)
        self.assertIn("THIRD_PARTY_NOTICES.md", (ROOT / "install.sh").read_text(encoding="utf-8"))

    def test_current_mihomo_accepts_generated_vless_configs(self):
        configured = os.environ.get("OMAVLESS_TEST_MIHOMO", "").strip()
        if not configured:
            self.skipTest("set OMAVLESS_TEST_MIHOMO for the opt-in core integration test")
        core = Path(configured)
        template = (ROOT / "templates" / "default.yaml").read_text(encoding="utf-8")
        base, fragment = REALITY_URI.split("#", 1)
        xhttp_extra = urllib.parse.quote(json.dumps({
            "headers": {"X-Trace": "omavless"},
            "xPaddingBytes": "100-1000",
            "xmux": {"maxConcurrency": "16-32", "hKeepAlivePeriod": 0},
        }, separators=(",", ":")))
        xhttp_download_extra = urllib.parse.quote(json.dumps({
            "downloadSettings": {
                "address": "download.example.com",
                "port": 443,
                "network": "xhttp",
                "security": "tls",
                "tlsSettings": {
                    "serverName": "download.example.com",
                    "alpn": ["h2"],
                    "fingerprint": "chrome",
                    "allowInsecure": False,
                },
                "xhttpSettings": {
                    "path": "/down", "host": "download.example.com",
                    "mode": "stream-up",
                },
            }
        }, separators=(",", ":")))
        profiles = {
            "reality-vision": REALITY_URI,
            "reality-vision-udp443-xudp": (
                base.replace(
                    "flow=xtls-rprx-vision", "flow=xtls-rprx-vision-udp443"
                )
                + "&packetEncoding=xudp#" + fragment
            ),
            "xhttp-packet-up": (
                "vless://11111111-1111-4111-8111-111111111111@example.com:443"
                "?type=xhttp&security=tls&sni=example.com&path=%2Fedge"
                "&mode=packet-up&packetEncoding=packetaddr#XHTTP"
            ),
            "xhttp-extra-xmux": (
                "vless://11111111-1111-4111-8111-111111111111@example.com:443"
                "?type=xhttp&security=tls&sni=example.com&path=%2Fedge"
                f"&mode=packet-up&extra={xhttp_extra}#XHTTP-extra"
            ),
            "xhttp-split-download": (
                "vless://11111111-1111-4111-8111-111111111111@example.com:443"
                "?type=xhttp&security=tls&sni=example.com&path=%2Fup"
                f"&mode=stream-up&extra={xhttp_download_extra}#XHTTP-split"
            ),
        }
        for name, uri in profiles.items():
            with self.subTest(profile=name), tempfile.TemporaryDirectory() as temp:
                config = template.replace(
                    backend.PROFILE_MARKER,
                    backend.proxy_yaml({"name": name, "uri": uri}),
                )
                config_path = Path(temp) / "config.yaml"
                config_path.write_text(config, encoding="utf-8")
                result = subprocess.run(
                    [str(core), "-t", "-d", temp, "-f", str(config_path)],
                    text=True, capture_output=True,
                )
                self.assertEqual(result.returncode, 0, result.stderr or result.stdout)

    def test_uninstall_removes_only_runtime_integration_unless_purged(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            fake_bin = home / "bin"
            fake_bin.mkdir()
            systemctl = fake_bin / "systemctl"
            systemctl.write_text(
                '#!/bin/sh\ncase "$*" in *"is-active"*) exit 3;; *) exit 0;; esac\n',
                encoding="utf-8",
            )
            systemctl.chmod(0o755)
            unit = home / ".config/systemd/user/omavless.service"
            startup_unit = home / ".config/systemd/user/omavless-autostart.service"
            data = home / ".config/omavless"
            unit.parent.mkdir(parents=True)
            data.mkdir(parents=True)
            unit.write_text("unit", encoding="utf-8")
            startup_unit.write_text("startup unit", encoding="utf-8")
            (data / "profiles.json").write_text("secret", encoding="utf-8")
            env = os.environ.copy()
            env.update({"HOME": str(home), "PATH": str(fake_bin) + os.pathsep + env["PATH"]})

            result = subprocess.run(
                ["bash", str(ROOT / "uninstall.sh")], env=env, text=True, capture_output=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(unit.exists())
            self.assertFalse(startup_unit.exists())
            self.assertTrue(data.exists())

            result = subprocess.run(
                ["bash", str(ROOT / "uninstall.sh"), "--purge"],
                env=env, text=True, capture_output=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(data.exists())

    def test_uninstall_keeps_unit_when_service_cannot_be_stopped(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            fake_bin = home / "bin"
            fake_bin.mkdir()
            systemctl = fake_bin / "systemctl"
            systemctl.write_text(
                '#!/bin/sh\ncase "$*" in *"disable --now"*) exit 1;; *"is-active"*) exit 0;; *) exit 0;; esac\n',
                encoding="utf-8",
            )
            systemctl.chmod(0o755)
            unit = home / ".config/systemd/user/omavless.service"
            unit.parent.mkdir(parents=True)
            unit.write_text("unit", encoding="utf-8")
            env = os.environ.copy()
            env.update({"HOME": str(home), "PATH": str(fake_bin) + os.pathsep + env["PATH"]})

            result = subprocess.run(
                ["bash", str(ROOT / "uninstall.sh")], env=env, text=True, capture_output=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("still running", result.stderr)
            self.assertTrue(unit.exists())


if __name__ == "__main__":
    unittest.main()
