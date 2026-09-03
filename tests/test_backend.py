import importlib.util
import io
import http.client
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
PLUGIN = ROOT / "plugin"
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
TROJAN_URI = (
    "trojan://s3cr%3At%40value@example.com:443"
    "?type=tcp&security=tls&sni=cdn.example.org&alpn=h2%2Chttp%2F1.1"
    "&fp=chrome#Trojan%20TLS"
)
TROJAN_REALITY_URI = (
    "trojan://reality-password@example.com:443"
    "?type=grpc&security=reality&sni=reality.example.org&fp=firefox"
    "&serviceName=edge&pbk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    "&sid=0123456789abcdef#Trojan%20Reality"
)
HYSTERIA2_PIN = "0123456789abcdef" * 4
HYSTERIA2_URI = (
    "hysteria2://user%3Apass%40secret@example.com:443,5000-6000/"
    "?obfs=gecko&obfs-password=obfs%20secret&sni=hy2.example.org"
    f"&insecure=1&pinSHA256={HYSTERIA2_PIN}&ech=AQIDBA%3D%3D"
    "#Hysteria%202"
)
TUIC_URI = (
    "tuic://22222222-2222-4222-8222-222222222222:pass%3Asecret"
    "@tuic.example.com:10443?congestion_control=bbr&udp_relay_mode=quic"
    "&alpn=h3%2Chq-29&sni=edge.example.com&allow_insecure=0&disable_sni=0"
    "#TUIC%20v5"
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

    def write_ownership_marker(
        self, paths: backend.Paths, phase: str, generation: int = 1
    ) -> Path:
        marker = backend.ownership_marker_path(paths)
        marker.parent.mkdir(parents=True, mode=0o700, exist_ok=True)
        marker.write_text(json.dumps({
            "schemaVersion": 1,
            "generation": generation,
            "phase": phase,
        }), encoding="utf-8")
        marker.chmod(0o600)
        return marker

    def test_manifest_matches_plugin_entrypoint_and_release_version(self):
        manifest = json.loads((ROOT / "manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["schemaVersion"], 1)
        self.assertEqual(manifest["id"], "kdk.omavless")
        self.assertEqual(manifest["name"], "OmaVLESS")
        self.assertEqual(manifest["barWidget"]["displayName"], "OmaVLESS")
        self.assertEqual(manifest["version"], "0.7.0")
        self.assertEqual(backend.PLUGIN_VERSION, manifest["version"])
        self.assertEqual(backend.USER_AGENT, "OmaVLESS/0.7.0")
        self.assertEqual(manifest["entryPoints"]["barWidget"], "plugin/Panel.qml")
        self.assertIn("experimental Trojan, Hysteria2 and TUIC", manifest["description"])
        widget = manifest["barWidget"]
        self.assertEqual(widget["defaults"]["locale"], "system")
        locale_schema = next(item for item in widget["schema"] if item["key"] == "locale")
        self.assertEqual(locale_schema["type"], "enum")
        self.assertEqual(locale_schema["options"], ["system", "en", "ru"])
        self.assertEqual(locale_schema["defaultValue"], "system")
        panel = (PLUGIN / "Panel.qml").read_text(encoding="utf-8")
        self.assertIn('moduleName: "kdk.omavless"', panel)

    def test_cli_file_paths_accept_an_explicit_option_boundary(self):
        parser = backend.build_parser()
        self.assertEqual(
            parser.parse_args(["preview", "--", "--profile"]).file,
            "--profile",
        )
        self.assertEqual(
            parser.parse_args(["import-preview", "--", "--input"]).file,
            "--input",
        )
        subscription_file = parser.parse_args([
            "subscription-save-file", "--", "--provider", "", "--input",
        ])
        self.assertEqual(subscription_file.name, "--provider")
        self.assertEqual(subscription_file.id, "")
        self.assertEqual(subscription_file.file, "--input")
        export = parser.parse_args([
            "export-file", "--", "11111111-1111-4111-8111-111111111111", "--output",
        ])
        self.assertEqual(export.path, "--output")
        self.assertEqual(
            parser.parse_args(["diagnostics-export", "--", "--report"]).path,
            "--report",
        )

    def test_pointer_hover_never_scrolls_profile_lists(self):
        panel = (PLUGIN / "Panel.qml").read_text(encoding="utf-8")
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

    def test_vless_ipv6_and_percent_encoded_label_map_without_downgrade(self):
        uri = (
            "vless://11111111-1111-4111-8111-111111111111@[2001:db8::1]:443"
            "?type=ws&security=tls&sni=ipv6.example.org"
            "&host=cdn.example.org&path=%2Fedge#A%20%26%20B%20%3E%20C"
        )
        parsed = backend.parse_vless(uri)
        self.assertEqual(parsed["server"], "2001:db8::1")
        self.assertEqual(parsed["suggested_name"], "A & B > C")
        rendered = backend.proxy_yaml({"name": "IPv6", "uri": uri})
        self.assertIn('server: "2001:db8::1"', rendered)
        self.assertIn('path: "/edge"', rendered)
        self.assertIn('Host: "cdn.example.org"', rendered)
        preview = backend.preview_vless(uri)
        self.assertEqual(preview["server"], "2001:db8::1")
        self.assertEqual(preview["suggestedName"], "A & B > C")

    def test_trojan_tcp_ws_and_grpc_reality_map_to_mihomo(self):
        tcp = backend.parse_profile(TROJAN_URI)
        self.assertEqual(tcp["protocol"], "trojan")
        self.assertEqual(tcp["password"], "s3cr:t@value")
        self.assertEqual(tcp["alpn"], ["h2", "http/1.1"])
        tcp_yaml = backend.profile_yaml({
            "name": "Trojan TLS", "uri": TROJAN_URI, "protocol": "trojan",
        })
        for expected in (
            "type: trojan", 'password: "s3cr:t@value"', "network: tcp",
            'sni: "cdn.example.org"', 'client-fingerprint: "chrome"',
        ):
            self.assertIn(expected, tcp_yaml)

        ws_uri = (
            "trojan://ws-password@example.com:443?type=ws&security=tls"
            "&sni=ws.example.org&host=edge.example.org&path=%2Fsocket&fp=safari#WS"
        )
        ws_yaml = backend.profile_yaml({
            "name": "Trojan WS", "uri": ws_uri, "protocol": "trojan",
        })
        self.assertIn("network: ws", ws_yaml)
        self.assertIn('path: "/socket"', ws_yaml)
        self.assertIn('Host: "edge.example.org"', ws_yaml)

        reality = backend.parse_profile(TROJAN_REALITY_URI)
        self.assertEqual(reality["network"], "grpc")
        self.assertEqual(reality["security"], "reality")
        reality_yaml = backend.profile_yaml({
            "name": "Trojan Reality", "uri": TROJAN_REALITY_URI,
            "protocol": "trojan",
        })
        self.assertIn("reality-opts:", reality_yaml)
        self.assertIn('grpc-service-name: "edge"', reality_yaml)
        self.assertNotIn("tls: true", reality_yaml)

        reality_base, reality_fragment = TROJAN_REALITY_URI.split("#", 1)
        pq_uri = reality_base + "&supportX25519MLKEM768=true&spx=%2Fprivate#" \
            + reality_fragment
        pq_yaml = backend.profile_yaml({
            "name": "Trojan PQ", "uri": pq_uri, "protocol": "trojan",
        })
        self.assertIn("support-x25519mlkem768: true", pq_yaml)
        self.assertNotIn("spider-x", pq_yaml)
        pq_preview = backend.preview_profile(pq_uri)
        self.assertEqual(pq_preview["experimentalFeatures"], ["Trojan", "REALITY PQ"])
        self.assertIn("spider path", pq_preview["compatibilityNote"])
        self.assertNotIn("private", json.dumps(pq_preview))

    def test_trojan_preview_and_errors_never_expose_the_password(self):
        preview = backend.preview_profile(TROJAN_URI)
        self.assertEqual(preview["protocol"], "trojan")
        self.assertEqual(preview["credentialHint"], "••••")
        self.assertTrue(preview["experimental"])
        self.assertEqual(preview["experimentalFeatures"], ["Trojan"])
        public = json.dumps(preview, ensure_ascii=False)
        self.assertNotIn("s3cr", public)
        self.assertNotIn("trojan://", public)

        secret_uri = TROJAN_URI.replace(
            "&fp=chrome", "&fp=private-secret-fingerprint"
        )
        with self.assertRaises(backend.BackendError) as caught:
            backend.parse_trojan(secret_uri)
        self.assertNotIn("private-secret", str(caught.exception))

    def test_trojan_rejects_ambiguous_or_unrepresentable_share_fields(self):
        tls_base, tls_fragment = TROJAN_URI.split("#", 1)
        reality_ws = TROJAN_REALITY_URI.replace("type=grpc", "type=ws").replace(
            "&serviceName=edge", ""
        )
        cases = (
            ("trojan://@example.com:443", "password"),
            ("trojan://user:password@example.com:443", "authority"),
            (TROJAN_URI.replace("security=tls", "security=none"), "TLS or Reality"),
            (TROJAN_URI.replace("type=tcp", "type=xhttp"), "transport"),
            (tls_base + "&unknown=private-secret#" + tls_fragment, "unsupported fields"),
            (TROJAN_URI.replace("type=tcp", "type=tcp&host=example.org"),
             "transport-only"),
            (tls_base + "&allowInsecure=maybe#" + tls_fragment,
             "true or false"),
            (reality_ws,
             "not supported with WebSocket"),
            (tls_base + "&pbk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA#"
             + tls_fragment,
             "require Reality"),
        )
        for uri, message in cases:
            with self.subTest(message=message), self.assertRaisesRegex(
                    backend.BackendError, message) as caught:
                backend.parse_trojan(uri)
            self.assertNotIn("private-secret", str(caught.exception))

    def test_hysteria2_official_uri_maps_to_current_mihomo_schema(self):
        node = backend.parse_profile(HYSTERIA2_URI)
        self.assertEqual(node["protocol"], "hysteria2")
        self.assertEqual(node["password"], "user:pass@secret")
        self.assertEqual(node["server"], "example.com")
        self.assertEqual(node["port"], 443)
        self.assertEqual(node["ports"], "443,5000-6000")
        self.assertTrue(node["port_hopping"])
        self.assertEqual(node["obfs"], "gecko")
        self.assertEqual(node["obfs_password"], "obfs secret")
        self.assertEqual(node["fingerprint"], HYSTERIA2_PIN)
        self.assertEqual(node["ech"], "AQIDBA==")

        yaml = backend.profile_yaml({
            "name": "Hysteria 2", "uri": HYSTERIA2_URI,
            "protocol": "hysteria2",
        })
        for expected in (
            "type: hysteria2", 'server: "example.com"', "port: 443",
            'ports: "443,5000-6000"', 'password: "user:pass@secret"',
            'obfs: "gecko"', 'obfs-password: "obfs secret"',
            'sni: "hy2.example.org"', "skip-cert-verify: true",
            f'fingerprint: "{HYSTERIA2_PIN}"', "ech-opts:",
            "enable: true", 'config: "AQIDBA=="',
        ):
            self.assertIn(expected, yaml)
        self.assertNotIn("  up:", yaml)
        self.assertNotIn("  down:", yaml)

    def test_hysteria2_preview_and_errors_do_not_expose_auth_or_obfs_secrets(self):
        preview = backend.preview_profile(HYSTERIA2_URI)
        self.assertEqual(preview["protocol"], "hysteria2")
        self.assertEqual(preview["transport"], "quic")
        self.assertEqual(preview["security"], "tls")
        self.assertEqual(preview["credentialHint"], "••••")
        self.assertEqual(preview["experimentalFeatures"], ["Hysteria2"])
        public = json.dumps(preview, ensure_ascii=False)
        for secret in ("user:pass", "obfs secret", "AQIDBA", HYSTERIA2_PIN):
            self.assertNotIn(secret, public)
        self.assertNotIn("hysteria2://", public)

        private_uri = HYSTERIA2_URI.replace(
            "obfs=gecko", "obfs=private-obfuscator"
        )
        with self.assertRaisesRegex(backend.BackendError, "obfuscation type") as caught:
            backend.parse_hysteria2(private_uri)
        self.assertNotIn("private-obfuscator", str(caught.exception))

    def test_hysteria2_accepts_alias_default_port_ipv6_userpass_and_colon_pin(self):
        default = backend.parse_profile("hy2://user:pass@example.com/#Default")
        self.assertEqual(default["password"], "user:pass")
        self.assertEqual(default["port"], 443)
        self.assertFalse(default["port_hopping"])
        self.assertEqual(default["ports"], "443")

        colon_pin = ":".join(
            HYSTERIA2_PIN[index:index + 2]
            for index in range(0, len(HYSTERIA2_PIN), 2)
        )
        ipv6 = backend.parse_profile(
            "hy2://auth@[2001:db8::1]:8443-8445/"
            "?obfs=salamander&obfs-password=secret"
            f"&pinSHA256={colon_pin}#IPv6"
        )
        self.assertEqual(ipv6["server"], "2001:db8::1")
        self.assertEqual(ipv6["ports"], "8443-8445")
        self.assertEqual(ipv6["fingerprint"], HYSTERIA2_PIN)

    def test_hysteria2_rejects_ambiguous_or_unrepresentable_share_fields(self):
        cases = (
            ("hy2://auth@example.com:0", "range"),
            ("hy2://auth@example.com:443,443", "overlapping"),
            ("hy2://auth@example.com:500-400", "range"),
            ("hy2://auth@example.com:443?insecure=true", "0 or 1"),
            ("hy2://auth@example.com:443?obfs=gecko", "requires a password"),
            ("hy2://auth@example.com:443?obfs-password=private-secret", "requires an obfs"),
            ("hy2://auth@example.com:443?pinSHA256=deadbeef", "SHA-256"),
            ("hy2://auth@example.com:443?ech=not-base64", "base64"),
            ("hy2://auth@example.com:443?upmbps=100", "local setting"),
            ("hy2://auth@example.com:443?unknown=private-secret", "unsupported fields"),
            ("hy2://auth@example.com:443/private", "path is not supported"),
        )
        for uri, message in cases:
            with self.subTest(message=message), self.assertRaisesRegex(
                    backend.BackendError, message) as caught:
                backend.parse_hysteria2(uri)
            self.assertNotIn("private-secret", str(caught.exception))
        with self.assertRaisesRegex(backend.BackendError, "not supported"):
            backend.parse_profile(
                "hysteria2+realm://token@realm.example/room?auth=private-secret"
            )

    def test_tuic_v5_interoperable_uri_maps_to_current_mihomo_schema(self):
        node = backend.parse_profile(TUIC_URI)
        self.assertEqual(node["protocol"], "tuic")
        self.assertEqual(node["uuid"], "22222222-2222-4222-8222-222222222222")
        self.assertEqual(node["password"], "pass:secret")
        self.assertEqual(node["server"], "tuic.example.com")
        self.assertEqual(node["port"], 10443)
        self.assertEqual(node["congestion_controller"], "bbr")
        self.assertEqual(node["udp_relay_mode"], "quic")
        self.assertEqual(node["alpn"], ["h3", "hq-29"])

        yaml = backend.profile_yaml({
            "name": "TUIC v5", "uri": TUIC_URI, "protocol": "tuic",
        })
        for expected in (
            "type: tuic", 'server: "tuic.example.com"', "port: 10443",
            'uuid: "22222222-2222-4222-8222-222222222222"',
            'password: "pass:secret"', 'udp-relay-mode: "quic"',
            'congestion-controller: "bbr"', 'alpn: ["h3", "hq-29"]',
            'sni: "edge.example.com"',
        ):
            self.assertIn(expected, yaml)
        self.assertNotIn("reduce-rtt", yaml)
        self.assertNotIn("skip-cert-verify", yaml)

    def test_tuic_preview_and_errors_do_not_expose_uuid_or_password(self):
        preview = backend.preview_profile(TUIC_URI)
        self.assertEqual(preview["protocol"], "tuic")
        self.assertEqual(preview["transport"], "quic")
        self.assertEqual(preview["credentialHint"], "••••")
        self.assertEqual(preview["experimentalFeatures"], ["TUIC v5"])
        public = json.dumps(preview, ensure_ascii=False)
        self.assertNotIn("22222222-2222-4222-8222-222222222222", public)
        self.assertNotIn("pass:secret", public)
        self.assertNotIn("tuic://", public)

        private_uri = TUIC_URI.replace("bbr", "private-controller")
        with self.assertRaisesRegex(backend.BackendError, "congestion controller") as caught:
            backend.parse_tuic(private_uri)
        self.assertNotIn("private-controller", str(caught.exception))

    def test_tuic_accepts_v2rayn_encoded_userinfo_and_safe_defaults(self):
        encoded_userinfo = urllib.parse.quote(
            "33333333-3333-4333-8333-333333333333:p@ss:word", safe=""
        )
        node = backend.parse_profile(
            f"tuic://{encoded_userinfo}@[2001:db8::2]:443/#Encoded"
        )
        self.assertEqual(node["uuid"], "33333333-3333-4333-8333-333333333333")
        self.assertEqual(node["password"], "p@ss:word")
        self.assertEqual(node["server"], "2001:db8::2")
        self.assertEqual(node["congestion_controller"], "cubic")
        self.assertEqual(node["udp_relay_mode"], "native")
        yaml = backend.tuic_yaml({"name": "Default", "uri": node["uri"]})
        self.assertIn('congestion-controller: "cubic"', yaml)
        self.assertIn('udp-relay-mode: "native"', yaml)
        self.assertNotIn("reduce-rtt", yaml)

    def test_tuic_disable_sni_explains_mihomo_certificate_semantics(self):
        uri = (
            "tuic://44444444-4444-4444-8444-444444444444:password"
            "@example.com:443?disable_sni=1"
        )
        node = backend.parse_tuic(uri)
        self.assertTrue(node["disable_sni"])
        preview = backend.preview_tuic(uri)
        self.assertTrue(preview["insecure"])
        self.assertEqual(
            preview["compatibilityNote"], backend.TUIC_DISABLE_SNI_COMPATIBILITY_NOTE
        )
        self.assertIn("disable-sni: true", backend.tuic_yaml({
            "name": "No SNI", "uri": uri,
        }))

    def test_tuic_rejects_v4_and_unrepresentable_or_ambiguous_fields(self):
        base = "tuic://22222222-2222-4222-8222-222222222222:private-secret@example.com:443"
        cases = (
            ("tuic://v4-token@example.com:443", "requires a UUID and password"),
            (base.replace("22222222-2222-4222-8222-222222222222", "not-a-uuid"),
             "valid UUID"),
            (base.replace(":private-secret@", ":@"), "password"),
            (base + "?congestion_control=reno", "congestion controller"),
            (base + "?udp_relay_mode=lossy", "UDP relay mode"),
            (base + "?allow_insecure=true", "0 or 1"),
            (base + "?disable_sni=1&sni=example.com", "cannot set both"),
            (base + "?zero_rtt_handshake=1", "unsupported fields"),
            (base + "?token=private-secret", "unsupported fields"),
            (base + "?sni=a&sni=b", "duplicate fields"),
            (base + "/private", "invalid authority"),
        )
        for uri, message in cases:
            with self.subTest(message=message), self.assertRaisesRegex(
                    backend.BackendError, message) as caught:
                backend.parse_tuic(uri)
            self.assertNotIn("private-secret", str(caught.exception))

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

    def test_experimental_vless_encryption_is_bounded_and_redacted(self):
        client_key = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8"
        variants = (
            f"mlkem768x25519plus.native.1rtt.{client_key}",
            f"mlkem768x25519plus.xorpub.0rtt.{client_key}",
            (
                "mlkem768x25519plus.random.1rtt."
                f"100-111-1111.75-0-111.50-0-3333.{client_key}"
            ),
        )
        for encryption in variants:
            with self.subTest(encryption=encryption.split(".")[1:3]):
                uri = REALITY_URI.replace(
                    "encryption=none", "encryption=" + urllib.parse.quote(encryption)
                )
                node = backend.parse_vless(uri)
                self.assertEqual(node["encryption"], encryption)
                self.assertIn(
                    f"encryption: {json.dumps(encryption)}",
                    backend.proxy_yaml({"name": "Encrypted", "uri": uri}),
                )
                preview = backend.preview_vless(uri)
                self.assertTrue(preview["experimental"])
                self.assertEqual(preview["experimentalFeatures"], ["VLESS Encryption"])
                public = json.dumps(preview, ensure_ascii=False)
                self.assertNotIn(client_key, public)
                self.assertNotIn("mlkem768x25519plus", public)

    def test_vless_encryption_rejects_values_mihomo_cannot_parse(self):
        client_key = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8"
        cases = (
            ("aes-128-gcm", "not supported"),
            (f"mlkem768x25519plus.bad.1rtt.{client_key}", "not supported"),
            (f"mlkem768x25519plus.native.bad.{client_key}", "not supported"),
            ("mlkem768x25519plus.native.1rtt.100-111-1111", "client key"),
            ("mlkem768x25519plus.native.1rtt.bad", "padding"),
            (f"mlkem768x25519plus.native.1rtt.50-1-2.{client_key}", "too small"),
            (f"mlkem768x25519plus.native.1rtt.100-35-70000.{client_key}",
             "outside the supported range"),
            ("mlkem768x25519plus.native.1rtt." + "A" * 44, "key"),
        )
        for encryption, error in cases:
            uri = REALITY_URI.replace(
                "encryption=none", "encryption=" + urllib.parse.quote(encryption)
            )
            with self.subTest(error=error), self.assertRaisesRegex(backend.BackendError, error):
                backend.parse_vless(uri)
        private_value = "mlkem768x25519plus.native.1rtt.private-secret"
        with self.assertRaises(backend.BackendError) as failure:
            backend.validate_vless_encryption(private_value)
        self.assertNotIn("private-secret", str(failure.exception))

    def test_reality_pq_flag_is_explicit_experimental_metadata(self):
        base, fragment = REALITY_URI.split("#", 1)
        uri = base + "&supportX25519MLKEM768=true#" + fragment
        node = backend.parse_vless(uri)
        self.assertTrue(node["support_x25519mlkem768"])
        yaml = backend.proxy_yaml({"name": "PQ", "uri": uri})
        self.assertIn("support-x25519mlkem768: true", yaml)
        preview = backend.preview_vless(uri)
        self.assertTrue(preview["experimental"])
        self.assertEqual(preview["experimentalFeatures"], ["REALITY PQ"])
        self.assertIn("fingerprint", preview["compatibilityNote"])
        public = json.dumps(preview, ensure_ascii=False)
        self.assertNotIn("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", public)

        disabled = backend.parse_vless(
            base + "&support-x25519mlkem768=false#" + fragment
        )
        self.assertFalse(disabled["support_x25519mlkem768"])
        with self.assertRaisesRegex(backend.BackendError, "must be true or false"):
            backend.parse_vless(base + "&supportX25519MLKEM768=maybe#" + fragment)
        with self.assertRaisesRegex(backend.BackendError, "conflicting field aliases"):
            backend.parse_vless(
                base + "&supportX25519MLKEM768=true"
                "&support-x25519mlkem768=false#" + fragment
            )
        with self.assertRaisesRegex(backend.BackendError, "requires Reality"):
            backend.parse_vless(
                uri.replace("security=reality", "security=tls")
            )

    def test_vless_query_rejects_duplicates_aliases_and_unknown_fields(self):
        base, fragment = REALITY_URI.split("#", 1)
        cases = (
            (base + "&security=tls#" + fragment, "duplicate fields"),
            (base + "&network=grpc#" + fragment, "conflicting field aliases"),
            (base + "&unknown=private-query-value#" + fragment, "unsupported fields"),
            (base + "&allowInsecure=maybe#" + fragment, "must be true or false"),
        )
        for uri, message in cases:
            with self.subTest(message=message), self.assertRaisesRegex(
                    backend.BackendError, message) as caught:
                backend.parse_vless(uri)
            self.assertNotIn("private-query-value", str(caught.exception))

        malformed_port = REALITY_URI.replace(":443", ":private-port")
        with self.assertRaisesRegex(backend.BackendError, "Invalid VLESS link") as caught:
            backend.parse_vless(malformed_port)
        self.assertNotIn("private-port", str(caught.exception))

    def test_vless_provider_metadata_is_explicit_but_never_mapped(self):
        base, fragment = REALITY_URI.split("#", 1)
        uri = (
            base + "&concurrency=4&x-durev-block=provider-value"
            "&x-durev-prio=2#" + fragment
        )
        node = backend.parse_vless(uri)
        self.assertIn(
            backend.VLESS_PROVIDER_METADATA_COMPATIBILITY_NOTE,
            node["compatibility_note"],
        )
        yaml = backend.proxy_yaml({"name": "Provider", "uri": uri})
        self.assertNotIn("concurrency", yaml)
        self.assertNotIn("x-durev", yaml)
        preview = backend.preview_vless(uri)
        public = json.dumps(preview, ensure_ascii=False)
        self.assertIn("Provider-only VLESS metadata", public)
        self.assertNotIn("provider-value", public)

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
                    "supportX25519MLKEM768": True,
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
        self.assertIn("support-x25519mlkem768: true", yaml)
        self.assertNotIn("spider-x", yaml)
        preview = backend.preview_vless(uri)
        self.assertEqual(
            preview["compatibilityNote"], backend.REALITY_SPX_COMPATIBILITY_NOTE
            + " " + backend.REALITY_PQ_COMPATIBILITY_NOTE
        )
        self.assertTrue(preview["experimental"])
        self.assertEqual(preview["experimentalFeatures"], ["REALITY PQ"])
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
            for pid, name in (
                ("10010", "python3"), ("10011", "v2rayN"), ("10012", "bash"),
                ("10013", "mihomo"), ("10014", "xray"),
            ):
                (proc / pid).mkdir()
                (proc / pid / "comm").write_text(name + "\n", encoding="utf-8")
                children = proc / pid / "task" / pid
                children.mkdir(parents=True)
                (children / "children").write_text("", encoding="ascii")
            (proc / "10010" / "task" / "10010" / "children").write_text(
                "10013\n", encoding="ascii"
            )
            (proc / "10013" / "task" / "10013" / "children").write_text(
                "10014\n", encoding="ascii"
            )
            family = backend.process_family_pids(10010, proc)
            self.assertEqual(family, {10010, 10013, 10014})
            self.assertEqual(backend.vpn_process_labels(proc, own_pids=family), ["V2RayN"])

            (proc / "10010" / "task" / "10010" / "children").write_text(
                " ".join(str(pid) for pid in range(20000, 20100)), encoding="ascii"
            )
            self.assertEqual(len(backend.process_family_pids(10010, proc, limit=4)), 4)

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
            # Status serialization must not depend on whether the developer's
            # machine happens to have a Mihomo binary on PATH. Core discovery
            # has its own focused tests; keep this fixture host-independent.
            with mock.patch.object(backend, "service_active", return_value=False), \
                 mock.patch.object(backend, "core_setup_status", return_value={
                     "installed": False, "tunReady": False, "path": "",
                 }), \
                 mock.patch.object(backend, "file_picker_status", return_value={
                     "available": False, "provider": "",
                 }), \
                 mock.patch.object(backend, "desktop_helper_status", return_value={
                     "configEditorAvailable": False,
                     "qrEncoderAvailable": False,
                 }):
                payload = json.loads(backend.status_text(paths))
            self.assertEqual(payload["routing"], {
                "mode": "rule", "source": "custom", "preset": "",
                "configured": False, "ruleCount": 1, "providerCount": 0,
                "customRuleCount": 0, "rulesUpdatedAt": 0,
                "ruleUpdateAvailable": False,
            })
            self.assertEqual(payload["capabilities"]["core"], "mihomo")
            self.assertEqual(
                payload["capabilities"]["protocols"],
                ["vless", "trojan", "hysteria2", "tuic"],
            )
            self.assertEqual(payload["coreSetup"], {
                "installed": False, "tunReady": False, "path": "",
            })
            self.assertEqual(payload["filePicker"], {
                "available": False, "provider": "",
            })
            self.assertEqual(payload["desktopHelpers"], {
                "configEditorAvailable": False,
                "qrEncoderAvailable": False,
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
            store["profiles"] = [{
                "id": profile_id, "name": "Example", "uri": REALITY_URI,
                "protocol": "vless",
            }]
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

    def test_remote_rule_refresh_keeps_all_or_nothing_timestamp_and_private_errors(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            store = backend.empty_store()
            store["rulesUpdatedAt"] = 1234
            backend.save_store(paths, store)
            backend.controller_socket(paths).touch()
            providers = {"providers": {
                "private-provider-one": {"vehicleType": "HTTP"},
                "private-provider-two": {"vehicleType": "HTTP"},
            }}
            with mock.patch.object(backend, "service_active", return_value=True), \
                 mock.patch.object(backend, "controller_json", return_value=(200, providers)), \
                 mock.patch.object(
                     backend, "controller_request",
                     side_effect=[(204, {}), backend.BackendError("private-provider-two")],
                 ), self.assertRaises(backend.BackendError) as raised:
                backend.refresh_rule_providers(paths)
            self.assertEqual(backend.load_store(paths)["rulesUpdatedAt"], 1234)
            message = str(raised.exception)
            self.assertNotIn("private-provider-one", message)
            self.assertNotIn("private-provider-two", message)

    def test_loaded_rules_are_bounded_and_hide_internal_proxy_targets(self):
        payload = {"rules": [
            {"type": "DOMAIN-SUFFIX", "payload": "example.com", "proxy": "PROXY"},
            {"type": "IP-CIDR", "payload": "10.0.0.0/8", "proxy": "DIRECT"},
            {"type": "DOMAIN", "payload": "ads.example", "proxy": "REJECT-DROP"},
            {"type": "RULE-SET", "payload": "x" * 2000, "proxy": "Secret group"},
        ]}
        result = backend.loaded_rules_payload(payload)
        self.assertEqual(result["total"], 4)
        self.assertEqual(
            [item["target"] for item in result["items"]],
            ["VPN", "DIRECT", "REJECT", "VPN"],
        )
        encoded = json.dumps(result, ensure_ascii=False)
        self.assertNotIn("PROXY", encoded)
        self.assertNotIn("Secret group", encoded)
        self.assertLessEqual(
            len(result["items"][-1]["payload"].encode("utf-8")),
            backend.MAX_DIAGNOSTIC_RULE_PAYLOAD_BYTES,
        )
        many = backend.loaded_rules_payload({"rules": [
            {"type": "MATCH", "payload": "", "proxy": "PROXY"}
            for _ in range(backend.MAX_LOADED_RULES + 17)
        ]})
        self.assertEqual(many["shown"], backend.MAX_LOADED_RULES)
        self.assertTrue(many["truncated"])
        self.assertLessEqual(
            len(json.dumps(many, separators=(",", ":")).encode("utf-8")),
            280 * 1024,
        )

    def test_loaded_rule_parsing_rejects_malformed_metadata(self):
        for payload in (
            [], {"rules": "not-a-list"}, {"rules": ["not-an-object"]},
            {"rules": [{"type": [], "payload": "x", "proxy": "DIRECT"}]},
        ):
            with self.subTest(payload=payload), self.assertRaises(backend.BackendError):
                backend.loaded_rules_payload(payload)

    def test_controller_rejects_malformed_and_oversized_diagnostic_json(self):
        connection = mock.Mock()
        response = connection.getresponse.return_value
        response.status = 200
        with mock.patch.object(backend, "UnixHTTPConnection", return_value=connection):
            response.read.return_value = b"{broken"
            with self.assertRaisesRegex(backend.BackendError, "invalid JSON"):
                backend.controller_json(
                    Path("/tmp/private.sock"), "/rules", 1,
                    max_response_bytes=128,
                )
            response.read.return_value = b"x" * 129
            with self.assertRaisesRegex(backend.BackendError, "too large"):
                backend.controller_json(
                    Path("/tmp/private.sock"), "/rules", 1,
                    max_response_bytes=128,
                )

    def test_loaded_rule_provider_status_is_bounded_and_derived(self):
        result = backend.loaded_rule_providers_payload({"providers": {
            "remote-main": {
                "vehicleType": "HTTP", "behavior": "Domain",
                "ruleCount": 321, "updatedAt": "2026-08-25T10:00:00Z",
            },
            "empty-local": {
                "vehicleType": "File", "behavior": "Classical",
                "ruleCount": 0, "updatedAt": "",
            },
        }})
        self.assertEqual(result["total"], 2)
        self.assertEqual(result["items"][0], {
            "name": "remote-main", "behavior": "Domain", "ruleCount": 321,
            "updatedAt": "2026-08-25T10:00:00Z", "status": "loaded",
            "refreshable": True,
        })
        self.assertEqual(result["items"][1]["status"], "empty")
        self.assertNotIn("vehicleType", json.dumps(result))

    def test_rule_provider_refresh_names_come_only_from_valid_loaded_list(self):
        payload = {"providers": {
            "remote-main": {"vehicleType": "HTTP"},
            "local-copy": {"vehicleType": "File"},
        }}
        self.assertEqual(
            backend.refreshable_rule_provider_names(payload), ["remote-main"]
        )
        for name in ("../escape", "provider/name", "provider?query", "bad\nname"):
            with self.subTest(name=name), self.assertRaises(backend.BackendError):
                backend.refreshable_rule_provider_names({
                    "providers": {name: {"vehicleType": "HTTP"}}
                })

    def test_advanced_diagnostics_redacts_local_credentials_names_and_urls(self):
        profile_name = "Private profile name"
        subscription_name = "Private subscription"
        subscription_url = "https://user:token@example.com/feed?key=secret"
        fragments = (profile_name, subscription_name, subscription_url, REALITY_URI)
        result = {
            "rules": backend.loaded_rules_payload({"rules": [{
                "type": "RULE-SET", "payload": profile_name,
                "proxy": "11111111-1111-4111-8111-111111111111",
            }, {
                "type": "DOMAIN", "payload": subscription_url, "proxy": "PROXY",
            }]}, fragments),
            "providers": backend.loaded_rule_providers_payload({"providers": {
                subscription_name: {
                    "behavior": "Domain", "ruleCount": 1,
                    "updatedAt": "2026-08-25T10:00:00Z",
                },
            }}, fragments),
        }
        encoded = json.dumps(result, ensure_ascii=False)
        for secret in (
            profile_name, subscription_name, subscription_url, REALITY_URI,
            "11111111-1111-4111-8111-111111111111", "vless://",
        ):
            self.assertNotIn(secret, encoded)

    def test_advanced_diagnostics_reads_only_private_controller_endpoints(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            backend.save_store(paths, backend.empty_store())
            backend.controller_socket(paths).touch()
            responses = [
                (200, {"rules": [{
                    "type": "MATCH", "payload": "", "proxy": "PROXY",
                }]}),
                (200, {"providers": {}}),
            ]
            with mock.patch.object(backend, "service_active", return_value=True), \
                 mock.patch.object(backend, "controller_json", side_effect=responses) as request:
                result = backend.advanced_diagnostics_payload(paths)
            self.assertEqual(result["version"], 1)
            self.assertEqual(result["rules"]["items"][0]["target"], "VPN")
            self.assertEqual(
                [call.args[1] for call in request.call_args_list],
                ["/rules", "/providers/rules"],
            )
            for call in request.call_args_list:
                self.assertEqual(call.args[0], backend.controller_socket(paths))
                self.assertEqual(
                    call.kwargs["max_response_bytes"],
                    backend.ADVANCED_DIAGNOSTICS_RESPONSE_BYTES,
                )

    def test_advanced_diagnostics_controller_errors_are_credential_free(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            backend.save_store(paths, backend.empty_store())
            backend.controller_socket(paths).touch()
            private_error = "vless://private-uuid@private-endpoint controller.sock"
            with mock.patch.object(backend, "service_active", return_value=True), \
                 mock.patch.object(backend, "controller_json", side_effect=OSError(private_error)), \
                 self.assertRaises(backend.BackendError) as raised:
                backend.advanced_diagnostics_payload(paths)
            self.assertEqual(str(raised.exception), "Could not read live Mihomo diagnostics")
            self.assertNotIn(private_error, str(raised.exception))

    def test_full_vpn_selects_and_verifies_the_active_profile_privately(self):
        socket_path = Path("/run/user/1000/omavless/controller.sock")
        with mock.patch.object(
            backend, "wait_private_controller", return_value=socket_path
        ), mock.patch.object(
            backend, "controller_request", return_value=(204, {})
        ) as request, mock.patch.object(
            backend, "controller_json", side_effect=[
                (200, {"now": "Example"}), (200, {"now": "PROXY"}),
            ]
        ) as readback, mock.patch.object(backend.time, "sleep") as sleep:
            backend.select_global_proxy(mock.Mock(), "Example")
        self.assertEqual(request.call_args_list, [
            mock.call(
                socket_path, "PUT", "/proxies/PROXY", mock.ANY, {"name": "Example"}
            ),
            mock.call(
                socket_path, "PUT", "/proxies/GLOBAL", mock.ANY, {"name": "PROXY"}
            ),
        ])
        self.assertEqual(readback.call_args_list, [
            mock.call(socket_path, "/proxies/PROXY", mock.ANY),
            mock.call(socket_path, "/proxies/GLOBAL", mock.ANY),
        ])
        sleep.assert_not_called()

    def test_full_vpn_retries_selector_not_found_during_startup(self):
        socket_path = Path("/run/user/1000/omavless/controller.sock")
        with mock.patch.object(
            backend, "wait_private_controller", return_value=socket_path
        ), mock.patch.object(
            backend, "controller_request", side_effect=[
                (404, {}), (204, {}), (204, {}),
            ]
        ) as request, mock.patch.object(
            backend, "controller_json", side_effect=[
                (200, {"now": "Example"}), (200, {"now": "PROXY"}),
            ]
        ), mock.patch.object(backend.time, "sleep") as sleep:
            backend.select_global_proxy(mock.Mock(), "Example")
        self.assertEqual(request.call_count, 3)
        sleep.assert_called_once_with(backend.SELECTOR_READY_INITIAL_DELAY_SECONDS)

    def test_full_vpn_retries_temporary_controller_transport_failure(self):
        socket_path = Path("/run/user/1000/omavless/controller.sock")
        with mock.patch.object(
            backend, "wait_private_controller", return_value=socket_path
        ), mock.patch.object(
            backend, "controller_request", side_effect=[
                OSError("private controller startup"), (204, {}), (204, {}),
            ]
        ), mock.patch.object(
            backend, "controller_json", side_effect=[
                (200, {"now": "Example"}), (200, {"now": "PROXY"}),
            ]
        ), mock.patch.object(backend.time, "sleep") as sleep:
            backend.select_global_proxy(mock.Mock(), "Example")
        sleep.assert_called_once_with(backend.SELECTOR_READY_INITIAL_DELAY_SECONDS)

    def test_full_vpn_retries_temporary_controller_protocol_failure(self):
        socket_path = Path("/run/user/1000/omavless/controller.sock")
        with mock.patch.object(
            backend, "wait_private_controller", return_value=socket_path
        ), mock.patch.object(
            backend, "controller_request", side_effect=[
                http.client.BadStatusLine("startup"), (204, {}), (204, {}),
            ]
        ), mock.patch.object(
            backend, "controller_json", side_effect=[
                (200, {"now": "Example"}), (200, {"now": "PROXY"}),
            ]
        ), mock.patch.object(backend.time, "sleep") as sleep:
            backend.select_global_proxy(mock.Mock(), "Example")
        sleep.assert_called_once_with(backend.SELECTOR_READY_INITIAL_DELAY_SECONDS)

    def test_full_vpn_retries_temporary_readback_protocol_failure(self):
        socket_path = Path("/run/user/1000/omavless/controller.sock")
        with mock.patch.object(
            backend, "wait_private_controller", return_value=socket_path
        ), mock.patch.object(
            backend, "controller_request", return_value=(204, {})
        ) as request, mock.patch.object(
            backend, "controller_json", side_effect=[
                http.client.BadStatusLine("startup"),
                (200, {"now": "Example"}), (200, {"now": "PROXY"}),
            ]
        ), mock.patch.object(backend.time, "sleep") as sleep:
            backend.select_global_proxy(mock.Mock(), "Example")
        self.assertEqual(request.call_count, 2)
        sleep.assert_called_once_with(backend.SELECTOR_READY_INITIAL_DELAY_SECONDS)

    def test_full_vpn_retries_temporary_server_error(self):
        socket_path = Path("/run/user/1000/omavless/controller.sock")
        with mock.patch.object(
            backend, "wait_private_controller", return_value=socket_path
        ), mock.patch.object(
            backend, "controller_request", side_effect=[
                (503, {}), (204, {}), (204, {}),
            ]
        ), mock.patch.object(
            backend, "controller_json", side_effect=[
                (200, {"now": "Example"}), (200, {"now": "PROXY"}),
            ]
        ), mock.patch.object(backend.time, "sleep") as sleep:
            backend.select_global_proxy(mock.Mock(), "Example")
        sleep.assert_called_once_with(backend.SELECTOR_READY_INITIAL_DELAY_SECONDS)

    def test_full_vpn_retries_temporary_readback_failure(self):
        socket_path = Path("/run/user/1000/omavless/controller.sock")
        with mock.patch.object(
            backend, "wait_private_controller", return_value=socket_path
        ), mock.patch.object(
            backend, "controller_request", return_value=(204, {})
        ) as request, mock.patch.object(
            backend, "controller_json", side_effect=[
                (404, {}), (200, {"now": "DIRECT"}),
                (200, {"now": "Example"}), (200, {"now": "PROXY"}),
            ]
        ), mock.patch.object(backend.time, "sleep") as sleep:
            backend.select_global_proxy(mock.Mock(), "Example")
        self.assertEqual(request.call_count, 2)
        self.assertEqual(sleep.call_args_list, [mock.call(0.05), mock.call(0.1)])

    def test_full_vpn_does_not_retry_permanent_selector_errors(self):
        socket_path = Path("/run/user/1000/omavless/controller.sock")
        for status_code in (400, 401, 403):
            with self.subTest(status_code=status_code), mock.patch.object(
                backend, "wait_private_controller", return_value=socket_path
            ), mock.patch.object(
                backend, "controller_request", return_value=(status_code, {})
            ) as request, mock.patch.object(backend.time, "sleep") as sleep, \
                 self.assertRaisesRegex(backend.BackendError, "refused"):
                backend.select_global_proxy(mock.Mock(), "Example")
            request.assert_called_once()
            sleep.assert_not_called()

    def test_full_vpn_selector_retry_deadline_is_bounded(self):
        socket_path = Path("/run/user/1000/omavless/controller.sock")
        now = 0.0

        def monotonic():
            return now

        def sleep(delay):
            nonlocal now
            now += delay

        with mock.patch.object(
            backend, "wait_private_controller", return_value=socket_path
        ), mock.patch.object(
            backend, "controller_request", return_value=(404, {})
        ) as request, mock.patch.object(
            backend.time, "monotonic", side_effect=monotonic
        ), mock.patch.object(
            backend.time, "sleep", side_effect=sleep
        ), self.assertRaisesRegex(backend.BackendError, "refused"):
            backend.select_global_proxy(mock.Mock(), "Example")
        self.assertEqual(now, backend.SELECTOR_READY_TIMEOUT_SECONDS)
        self.assertLessEqual(request.call_count, 8)

    def test_full_vpn_rejects_a_selection_that_mihomo_did_not_retain(self):
        socket_path = Path("/run/user/1000/omavless/controller.sock")
        now = 0.0

        def monotonic():
            return now

        def sleep(delay):
            nonlocal now
            now += delay

        with mock.patch.object(
            backend, "wait_private_controller", return_value=socket_path
        ), mock.patch.object(
            backend, "controller_request", return_value=(204, {})
        ), mock.patch.object(
            backend, "controller_json", return_value=(200, {"now": "DIRECT"})
        ), mock.patch.object(
            backend.time, "monotonic", side_effect=monotonic
        ), mock.patch.object(
            backend.time, "sleep", side_effect=sleep
        ), self.assertRaisesRegex(backend.BackendError, "did not retain"):
            backend.select_global_proxy(mock.Mock(), "Example")
        self.assertEqual(now, backend.SELECTOR_READY_TIMEOUT_SECONDS)

    def test_full_vpn_rejects_nested_global_selector_failure(self):
        socket_path = Path("/run/user/1000/omavless/controller.sock")
        with mock.patch.object(
            backend, "wait_private_controller", return_value=socket_path
        ), mock.patch.object(
            backend, "controller_request", side_effect=[(204, {}), (400, {})]
        ) as request, mock.patch.object(
            backend, "controller_json", return_value=(200, {"now": "Example"})
        ), self.assertRaisesRegex(backend.BackendError, "refused"):
            backend.select_global_proxy(mock.Mock(), "Example")
        self.assertEqual(request.call_args_list[-1], mock.call(
            socket_path, "PUT", "/proxies/GLOBAL", mock.ANY, {"name": "PROXY"}
        ))

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
        self.assertIn("include-all-proxies: true", text)
        self.assertIn("- name: GLOBAL", text)
        self.assertIn("default-selected: PROXY", text)
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
                self.assertIn("include-all-proxies: true", text)
                self.assertIn("- name: GLOBAL", text)
                self.assertIn("default-selected: PROXY", text)
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

    def test_trojan_cli_import_keeps_password_out_of_public_status(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            env, _runtime = self.make_env(home)
            imported = subprocess.run(
                [str(ROOT / "backend.sh"), "import", "Trojan"],
                input=TROJAN_URI, text=True, env=env, capture_output=True,
            )
            self.assertEqual(imported.returncode, 0, imported.stderr)
            stored = json.loads(
                (home / ".config" / "omavless" / "profiles.json").read_text(
                    encoding="utf-8"
                )
            )
            self.assertEqual(stored["profiles"][0]["protocol"], "trojan")
            with mock.patch.object(backend, "service_active", return_value=False):
                public = backend.status_text(self.paths_for(home, home / "runtime"))
            self.assertNotIn("s3cr", public)
            self.assertNotIn("trojan://", public)
            self.assertEqual(json.loads(public)["profiles"][0]["protocol"], "trojan")

    def test_hysteria2_cli_import_keeps_auth_out_of_public_status(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            env, _runtime = self.make_env(home)
            imported = subprocess.run(
                [str(ROOT / "backend.sh"), "import", "Hysteria 2"],
                input=HYSTERIA2_URI, text=True, env=env, capture_output=True,
            )
            self.assertEqual(imported.returncode, 0, imported.stderr)
            stored = json.loads(
                (home / ".config" / "omavless" / "profiles.json").read_text(
                    encoding="utf-8"
                )
            )
            self.assertEqual(stored["profiles"][0]["protocol"], "hysteria2")
            with mock.patch.object(backend, "service_active", return_value=False):
                public = backend.status_text(self.paths_for(home, home / "runtime"))
            for secret in ("user%3Apass", "obfs%20secret", "hysteria2://"):
                self.assertNotIn(secret, public)
            self.assertEqual(
                json.loads(public)["profiles"][0]["protocol"], "hysteria2"
            )

    def test_tuic_cli_import_keeps_uuid_and_password_out_of_public_status(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            env, _runtime = self.make_env(home)
            imported = subprocess.run(
                [str(ROOT / "backend.sh"), "import", "TUIC"],
                input=TUIC_URI, text=True, env=env, capture_output=True,
            )
            self.assertEqual(imported.returncode, 0, imported.stderr)
            stored = json.loads(
                (home / ".config" / "omavless" / "profiles.json").read_text(
                    encoding="utf-8"
                )
            )
            self.assertEqual(stored["profiles"][0]["protocol"], "tuic")
            with mock.patch.object(backend, "service_active", return_value=False):
                public = backend.status_text(self.paths_for(home, home / "runtime"))
            self.assertNotIn("22222222-2222-4222-8222-222222222222", public)
            self.assertNotIn("pass%3Asecret", public)
            self.assertNotIn("tuic://", public)
            self.assertEqual(json.loads(public)["profiles"][0]["protocol"], "tuic")

    def test_v1_store_migrates_in_memory_to_protocol_aware_v3(self):
        profile_id = "22222222-2222-4222-8222-222222222222"
        migrated = backend.validate_store({
            "version": 1, "activeId": "", "lastId": profile_id,
            "profiles": [{"id": profile_id, "name": "Example", "uri": REALITY_URI}],
        })
        self.assertEqual(migrated["version"], 3)
        self.assertEqual(migrated["profiles"][0]["protocol"], "vless")
        self.assertEqual(migrated["routingPreset"], "roscomvpn-default")
        self.assertEqual(migrated["subscriptions"], [])
        self.assertNotIn("subscriptionId", migrated["profiles"][0])

    def test_v2_store_migration_preserves_profile_relationships_and_startup(self):
        profile_id = "22222222-2222-4222-8222-222222222222"
        subscription_id = "33333333-3333-4333-8333-333333333333"
        key = backend.subscription_key(REALITY_URI)
        migrated = backend.validate_store({
            "version": 2,
            "activeId": profile_id,
            "lastId": profile_id,
            "profiles": [{
                "id": profile_id,
                "name": "Example",
                "uri": REALITY_URI,
                "subscriptionId": subscription_id,
                "subscriptionKey": key,
                "missing": False,
                "favorite": True,
            }],
            "subscriptions": [{
                "id": subscription_id,
                "name": "Provider",
                "url": "https://provider.example/subscription",
                "updatedAt": 123,
            }],
            "routingPreset": "roscomvpn-default",
            "customRules": [],
            "rulesUpdatedAt": 456,
            "startupConfigured": True,
            "startup": {
                "enabled": True,
                "target": "profile",
                "profileId": profile_id,
                "mode": "rule",
            },
            "onboardingComplete": True,
        })
        self.assertEqual(migrated["version"], 3)
        self.assertEqual(migrated["profiles"][0]["protocol"], "vless")
        self.assertEqual(migrated["profiles"][0]["id"], profile_id)
        self.assertTrue(migrated["profiles"][0]["favorite"])
        self.assertEqual(migrated["profiles"][0]["subscriptionId"], subscription_id)
        self.assertEqual(migrated["activeId"], profile_id)
        self.assertEqual(migrated["lastId"], profile_id)
        self.assertEqual(migrated["startup"]["profileId"], profile_id)
        self.assertTrue(migrated["startup"]["enabled"])

    def test_v3_store_requires_a_matching_supported_protocol(self):
        profile_id = "22222222-2222-4222-8222-222222222222"
        base = {
            "version": 3,
            "activeId": "",
            "lastId": profile_id,
            "profiles": [{"id": profile_id, "name": "Example", "uri": REALITY_URI}],
        }
        with self.assertRaisesRegex(backend.BackendError, "missing its protocol"):
            backend.validate_store(json.loads(json.dumps(base)))
        mismatch = json.loads(json.dumps(base))
        mismatch["profiles"][0]["protocol"] = "trojan"
        with self.assertRaisesRegex(backend.BackendError, "does not match"):
            backend.validate_store(mismatch)
        base["profiles"][0]["protocol"] = "wireguard"
        with self.assertRaisesRegex(backend.BackendError, "unsupported protocol"):
            backend.validate_store(base)

    def test_profile_adapter_preserves_vless_parse_preview_yaml_and_identity(self):
        profile = {"name": "Example", "uri": REALITY_URI, "protocol": "vless"}
        self.assertEqual(backend.profile_protocol(REALITY_URI), "vless")
        self.assertEqual(
            backend.extract_profile_uri("https://docs.example/help\n" + REALITY_URI),
            REALITY_URI,
        )
        self.assertEqual(backend.parse_profile(REALITY_URI), backend.parse_vless(REALITY_URI))
        self.assertEqual(backend.preview_profile(REALITY_URI), backend.preview_vless(REALITY_URI))
        self.assertEqual(backend.profile_yaml(profile), backend.proxy_yaml(profile))
        self.assertEqual(backend.profile_endpoint(profile), "example.com")
        self.assertEqual(
            backend.profile_subscription_key(REALITY_URI),
            backend._vless_subscription_key(REALITY_URI),
        )
        with self.assertRaisesRegex(backend.BackendError, "not supported") as caught:
            backend.profile_protocol("unknown://a-secret@example.com:443")
        self.assertNotIn("a-secret", str(caught.exception))

    def test_unified_import_classifier_routes_one_profile_or_subscription(self):
        store = backend.empty_store()
        profile = backend.classify_import(REALITY_URI, store)
        self.assertEqual(profile["version"], 1)
        self.assertEqual(profile["kind"], "profile")
        self.assertEqual(profile["profile"], backend.preview_profile(REALITY_URI))

        url = "https://subscription.example/feed?token=private-token"
        with mock.patch.object(backend, "fetch_subscription") as fetch_mock:
            subscription = backend.classify_import(url, store)
        self.assertEqual(subscription, {
            "version": 1, "kind": "subscription",
            "suggestedName": "Subscription", "duplicate": False,
        })
        fetch_mock.assert_not_called()
        public = json.dumps(subscription)
        self.assertNotIn(url, public)
        self.assertNotIn("private-token", public)
        self.assertNotIn("subscription.example", public)

    def test_unified_import_classifier_rejects_invalid_or_ambiguous_input_safely(self):
        store = backend.empty_store()
        with self.assertRaisesRegex(backend.BackendError, "one profile link"):
            backend.classify_import(REALITY_URI + "\n" + TROJAN_URI, store)
        private = "https://user:private-password@example.com/feed"
        with self.assertRaises(backend.BackendError) as caught:
            backend.classify_import(private, store)
        self.assertEqual(
            str(caught.exception),
            "Input is not a supported profile link or valid subscription URL",
        )
        self.assertNotIn("private-password", str(caught.exception))
        for invalid in (
            "http://remote.example/feed",
            "https://provider.example/feed#fragment",
            "https://provider.example/feed bad",
        ):
            with self.subTest(invalid=invalid), self.assertRaises(backend.BackendError):
                backend.classify_import(invalid, store)

    def test_unified_import_classifier_flags_duplicate_without_fetching(self):
        url = "https://subscription.example/feed?token=private-token"
        store = backend.empty_store()
        store["subscriptions"].append({
            "id": "22222222-2222-4222-8222-222222222222",
            "name": "Existing", "url": url, "updatedAt": 0,
        })
        with mock.patch.object(backend, "fetch_subscription") as fetch_mock:
            result = backend.classify_import(url, store)
        self.assertTrue(result["duplicate"])
        fetch_mock.assert_not_called()

    def test_import_preview_cli_classifies_clipboard_and_file_consistently(self):
        url = "https://subscription.example/feed?token=private-token"
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            env, _runtime = self.make_env(home)
            source = home / "subscription.url"
            source.write_text(url, encoding="utf-8")
            stdin_result = subprocess.run(
                [str(ROOT / "backend.sh"), "import-preview"],
                input=url, capture_output=True, text=True, env=env, check=False,
            )
            file_result = subprocess.run(
                [str(ROOT / "backend.sh"), "import-preview", "--", str(source)],
                capture_output=True, text=True, env=env, check=False,
            )
        self.assertEqual(stdin_result.returncode, 0, stdin_result.stderr)
        self.assertEqual(file_result.returncode, 0, file_result.stderr)
        self.assertEqual(json.loads(stdin_result.stdout), json.loads(file_result.stdout))
        self.assertNotIn(url, stdin_result.stdout)
        self.assertNotIn("private-token", file_result.stdout)

    def test_subscription_url_file_is_bounded_validated_and_private(self):
        url = "https://subscription.example/feed?token=private-token"
        with tempfile.TemporaryDirectory() as temp:
            source = Path(temp) / "subscription.url"
            source.write_text(url + "\n", encoding="utf-8")
            self.assertEqual(backend.read_subscription_url_file(source), url)
            source.write_text(
                "https://user:private-password@example.com/feed", encoding="utf-8"
            )
            with self.assertRaises(backend.BackendError) as caught:
                backend.read_subscription_url_file(source)
        self.assertNotIn("private-password", str(caught.exception))

    def test_fresh_store_defers_routing_preset_choice(self):
        fresh = backend.validate_store(backend.empty_store())
        self.assertEqual(fresh["routingPreset"], "")

    def test_subscription_parser_accepts_raw_and_urlsafe_base64_lists(self):
        second = REALITY_URI.replace("example.com:443", "two.example:8443").replace(
            "#Example", "#Second"
        )
        raw = REALITY_URI + "\nss://ignored\n" + second + "\n"
        profiles, skipped = backend.parse_subscription(raw)
        self.assertEqual(len(profiles), 2)
        self.assertEqual(skipped, 0)
        encoded = backend.base64.urlsafe_b64encode(raw.encode()).decode().rstrip("=")
        profiles64, skipped64 = backend.parse_subscription(encoded)
        self.assertEqual([item["key"] for item in profiles64], [item["key"] for item in profiles])
        self.assertEqual(skipped64, 0)

    def test_subscription_parser_and_store_supports_all_profile_adapters(self):
        entries, skipped = backend.parse_subscription("\n".join(
            (REALITY_URI, TROJAN_URI, HYSTERIA2_URI, TUIC_URI)
        ))
        self.assertEqual(skipped, 0)
        self.assertEqual(
            [entry["node"]["protocol"] for entry in entries],
            ["vless", "trojan", "hysteria2", "tuic"],
        )
        subscription_id = "33333333-3333-4333-8333-333333333333"
        subscription = {
            "id": subscription_id, "name": "Mixed provider",
            "url": "https://provider.example/mixed", "updatedAt": 0,
        }
        store = backend.empty_store()
        store["subscriptions"].append(subscription)
        backend.sync_subscription_store(store, subscription, entries, 123)
        self.assertEqual(
            [profile["protocol"] for profile in store["profiles"]],
            ["vless", "trojan", "hysteria2", "tuic"],
        )
        migrated = backend.validate_store(store)
        self.assertEqual(migrated["version"], 3)

    def test_trojan_subscription_identity_ignores_label_and_query_order(self):
        base, _label = TROJAN_URI.split("#", 1)
        parsed = urllib.parse.urlsplit(base)
        reordered = urllib.parse.urlunsplit((
            parsed.scheme, parsed.netloc, parsed.path,
            urllib.parse.urlencode(list(reversed(urllib.parse.parse_qsl(parsed.query)))),
            "Provider rename",
        ))
        self.assertEqual(
            backend.profile_subscription_key(TROJAN_URI),
            backend.profile_subscription_key(reordered),
        )

    def test_hysteria2_subscription_identity_normalizes_alias_label_and_query_order(self):
        parsed = urllib.parse.urlsplit(HYSTERIA2_URI)
        reordered = urllib.parse.urlunsplit((
            "hy2", parsed.netloc, parsed.path,
            urllib.parse.urlencode(list(reversed(urllib.parse.parse_qsl(parsed.query)))),
            "Provider rename",
        ))
        self.assertEqual(
            backend.profile_subscription_key(HYSTERIA2_URI),
            backend.profile_subscription_key(reordered),
        )

    def test_tuic_subscription_identity_ignores_userinfo_encoding_label_and_query_order(self):
        parsed = urllib.parse.urlsplit(TUIC_URI)
        userinfo, endpoint = parsed.netloc.rsplit("@", 1)
        encoded_userinfo = urllib.parse.quote(
            urllib.parse.unquote(userinfo), safe=""
        )
        reordered = urllib.parse.urlunsplit((
            parsed.scheme, encoded_userinfo + "@" + endpoint, parsed.path,
            urllib.parse.urlencode(list(reversed(urllib.parse.parse_qsl(parsed.query)))),
            "Provider rename",
        ))
        self.assertEqual(
            backend.profile_subscription_key(TUIC_URI),
            backend.profile_subscription_key(reordered),
        )

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

    def test_vless_subscription_identity_ignores_unmapped_provider_metadata(self):
        base, fragment = REALITY_URI.split("#", 1)
        first = (
            base + "&concurrency=4&x-durev-block=first&x-durev-prio=0#"
            + fragment
        )
        second = (
            base.replace("type=tcp", "network=tcp")
            + "&concurrency=8&x-durev-block=second&x-durev-prio=4#Renamed"
        )
        self.assertEqual(
            backend.profile_subscription_key(first),
            backend.profile_subscription_key(second),
        )
        self.assertNotEqual(
            backend.profile_subscription_key(first),
            backend.profile_subscription_key(
                second.replace("sni=example.org", "sni=other.example")
            ),
        )

    def test_subscription_refresh_migrates_identity_without_replacing_profile(self):
        subscription_id = "33333333-3333-4333-8333-333333333333"
        profile_id = "44444444-4444-4444-8444-444444444444"
        base, fragment = REALITY_URI.split("#", 1)
        old_uri = base + "&x-durev-prio=0#" + fragment
        new_uri = base + "&x-durev-prio=4#Provider rename"
        store = backend.empty_store()
        store["subscriptions"] = [{
            "id": subscription_id, "name": "Provider",
            "url": "https://provider.example/sub", "updatedAt": 0,
        }]
        store["profiles"] = [{
            "id": profile_id, "name": "Old", "uri": old_uri,
            "protocol": "vless", "subscriptionId": subscription_id,
            # Simulate the raw-query identity persisted by pre-0.7 builds.
            "subscriptionKey": "f" * 64,
            "missing": False, "favorite": True,
        }]
        store["activeId"] = profile_id
        store["lastId"] = profile_id
        node = backend.parse_vless(new_uri)
        key = backend.profile_subscription_key(new_uri)
        result = backend.sync_subscription_store(
            store, store["subscriptions"][0],
            [{"key": key, "uri": new_uri, "node": node}], 123,
        )
        self.assertEqual(result, {"added": 0, "removed": 0, "stale": 0, "total": 1})
        self.assertEqual(store["profiles"][0]["id"], profile_id)
        self.assertTrue(store["profiles"][0]["favorite"])
        self.assertEqual(store["profiles"][0]["subscriptionKey"], key)
        self.assertEqual(store["activeId"], profile_id)
        self.assertEqual(store["lastId"], profile_id)

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
            self.assertEqual(stored["version"], 3)
            self.assertEqual(stored["subscriptions"][0]["url"], secret_url)
            with mock.patch.object(backend, "service_active", return_value=False):
                public = backend.status_text(paths)
            self.assertNotIn(secret_url, public)
            self.assertNotIn("do-not-leak", public)
            payload = json.loads(public)
            self.assertEqual(payload["subscriptions"][0]["name"], "My provider")
            self.assertEqual(payload["profiles"][0]["sourceName"], "My provider")
            self.assertEqual(payload["profiles"][0]["protocol"], "vless")

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
        service = (PLUGIN / "Service.qml").read_text(encoding="utf-8")
        backend_source = (ROOT / "backend.py").read_text(encoding="utf-8")
        self.assertIn('runControl(["subscription-save"', service)
        self.assertIn('read_stdin_text(MAX_SUBSCRIPTION_URL_BYTES, "subscription URL")', backend_source)

    def test_profile_credential_is_absent_from_live_process_argv_and_environment(self):
        if not Path("/proc/self/cmdline").is_file():
            self.skipTest("process inspection requires procfs")
        try:
            proc_self_pid = int(os.readlink("/proc/self"))
        except (OSError, ValueError):
            self.skipTest("process inspection requires a visible procfs PID namespace")
        if proc_self_pid != os.getpid():
            self.skipTest("procfs belongs to a different PID namespace")
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            env, _ = self.make_env(home)
            process = subprocess.Popen(
                [str(ROOT / "backend.sh"), "preview"],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                stderr=subprocess.PIPE, text=True, env=env,
            )
            try:
                proc = Path("/proc") / str(process.pid)
                if not proc.is_dir():
                    self.skipTest("procfs does not expose child PIDs in this namespace")
                command = b""
                for _ in range(100):
                    try:
                        command = (proc / "cmdline").read_bytes()
                    except FileNotFoundError:
                        # Some nested PID namespaces publish the child in
                        # procfs a moment after Popen returns.
                        time.sleep(0.01)
                        continue
                    if b"backend.py" in command:
                        break
                    time.sleep(0.01)
                self.assertTrue(command, "child process never became visible in procfs")
                environment = (proc / "environ").read_bytes()
                for private in (
                    b"vless://",
                    b"11111111-1111-4111-8111-111111111111",
                    b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                ):
                    self.assertNotIn(private, command)
                    self.assertNotIn(private, environment)
                stdout, stderr = process.communicate(REALITY_URI, timeout=10)
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=5)
            self.assertEqual(process.returncode, 0, stderr)
            self.assertNotIn(REALITY_URI, stdout)
            self.assertNotIn("11111111-1111-4111-8111-111111111111", stdout)
            self.assertNotIn("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", stdout)

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

    def test_python_reads_exact_private_rust_ownership_marker_schema(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            self.assertEqual(backend.read_ownership_marker(paths), backend.OwnershipMarker())
            marker = backend.ownership_marker_path(paths)
            marker.parent.mkdir(parents=True, mode=0o700)
            for generation, phase in enumerate(sorted(backend.OWNERSHIP_PHASES), start=1):
                marker.write_text(json.dumps({
                    "schemaVersion": 1, "generation": generation, "phase": phase,
                }), encoding="utf-8")
                marker.chmod(0o600)
                self.assertEqual(
                    backend.read_ownership_marker(paths),
                    backend.OwnershipMarker(1, generation, phase),
                )

    def test_python_ownership_marker_rejects_unsafe_or_invalid_state(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            marker = backend.ownership_marker_path(paths)
            marker.parent.mkdir(parents=True, mode=0o700)
            invalid = (
                b'{"schemaVersion":1,"generation":0,"phase":"legacy","phase":"rust"}',
                b'{"schemaVersion":1,"generation":true,"phase":"legacy"}',
                b'{"schemaVersion":1,"generation":0,"phase":"unknown"}',
                b'{"schemaVersion":2,"generation":0,"phase":"legacy"}',
                b'{"schemaVersion":1,"generation":0,"phase":"legacy","extra":1}',
            )
            for payload in invalid:
                marker.write_bytes(payload)
                marker.chmod(0o600)
                with self.assertRaisesRegex(backend.BackendError, "ownership state is invalid"):
                    backend.read_ownership_marker(paths)
            marker.write_bytes(b"x" * (backend.MAX_OWNERSHIP_MARKER_BYTES + 1))
            marker.chmod(0o600)
            with self.assertRaisesRegex(backend.BackendError, "ownership state is too large"):
                backend.read_ownership_marker(paths)
            marker.write_text('{"schemaVersion":1,"generation":0,"phase":"legacy"}')
            marker.chmod(0o644)
            with self.assertRaisesRegex(backend.BackendError, "ownership state is unsafe"):
                backend.read_ownership_marker(paths)
            marker.chmod(0o600)
            marker.parent.chmod(0o755)
            with self.assertRaisesRegex(backend.BackendError, "ownership state is unsafe"):
                backend.read_ownership_marker(paths)

    def test_python_ownership_marker_symlink_and_errors_do_not_leak_paths(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            marker = backend.ownership_marker_path(paths)
            marker.parent.mkdir(parents=True, mode=0o700)
            private = home / "private.example-password"
            private.write_text("unchanged", encoding="utf-8")
            marker.symlink_to(private)
            with self.assertRaises(backend.BackendError) as raised:
                backend.read_ownership_marker(paths)
            public = str(raised.exception)
            self.assertNotIn(str(private), public)
            self.assertNotIn("private.example", public)
            self.assertNotIn("password", public)
            self.assertEqual(private.read_text(encoding="utf-8"), "unchanged")

    def test_legacy_mutation_lock_allows_only_legacy_ownership(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)

            entered: list[str] = []
            with backend.legacy_mutation_lock(paths) as marker:
                entered.append(marker.phase)
            self.assertEqual(entered, ["legacy"])

            for generation, phase in enumerate(
                ("cutoverPreparing", "rust", "rollbackPreparing"), start=1
            ):
                with self.subTest(phase=phase):
                    self.write_ownership_marker(paths, phase, generation)
                    with self.assertRaises(backend.BackendError) as raised:
                        with backend.legacy_mutation_lock(paths):
                            entered.append(phase)
                    self.assertEqual(
                        str(raised.exception),
                        "OmaVLESS native runtime ownership blocks this legacy operation",
                    )
            self.assertEqual(entered, ["legacy"])

    def test_legacy_mutation_lock_checks_marker_after_shared_lock_entry(self):
        paths = mock.Mock()
        events: list[str] = []

        @backend.contextlib.contextmanager
        def recorded_lock(_paths, _timeout):
            events.append("lock-enter")
            try:
                yield
            finally:
                events.append("lock-exit")

        def marker_reader(_paths):
            events.append("marker-read")
            return backend.OwnershipMarker()

        with mock.patch.object(backend, "operation_lock", recorded_lock), \
             mock.patch.object(backend, "read_ownership_marker", marker_reader):
            with backend.legacy_mutation_lock(paths, timeout=0.25):
                events.append("body")
        self.assertEqual(
            events, ["lock-enter", "marker-read", "body", "lock-exit"]
        )

    def test_legacy_mutation_lock_fails_closed_before_body_on_invalid_marker(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            marker = backend.ownership_marker_path(paths)
            marker.parent.mkdir(parents=True, mode=0o700)
            marker.write_text('{"phase":"rust"}', encoding="utf-8")
            marker.chmod(0o600)
            entered = False
            with self.assertRaisesRegex(backend.BackendError, "ownership state is invalid"):
                with backend.legacy_mutation_lock(paths):
                    entered = True
            self.assertFalse(entered)

    def test_rust_ownership_blocks_legacy_import_without_reading_private_input(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            env, runtime = self.make_env(home)
            env["XDG_STATE_HOME"] = str(home / ".local" / "state")
            paths = self.paths_for(home, runtime)
            self.write_ownership_marker(paths, "rust", 9)
            private_input = REALITY_URI.replace("Example", "private-password-fragment")
            result = subprocess.run(
                [str(ROOT / "backend.sh"), "import", "Example"],
                input=private_input,
                capture_output=True,
                text=True,
                timeout=5,
                env=env,
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
            self.assertEqual(
                result.stderr.strip(),
                "OmaVLESS native runtime ownership blocks this legacy operation",
            )
            self.assertNotIn("private-password-fragment", result.stderr)
            self.assertFalse(paths.store.exists())

    def test_rust_ownership_keeps_read_only_status_available_without_migration(self):
        args = mock.Mock(command="status")
        parser = mock.Mock()
        parser.parse_args.return_value = args
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            self.write_ownership_marker(paths, "rust", 3)
            with mock.patch.object(backend, "build_parser", return_value=parser), \
                 mock.patch.object(backend.Paths, "current", return_value=paths), \
                 mock.patch.object(backend, "migrate_legacy_data") as migrate, \
                 mock.patch.object(backend, "status") as status:
                self.assertEqual(backend.main(), 0)
            migrate.assert_not_called()
            status.assert_called_once_with(paths)

    def test_every_declared_legacy_mutation_is_rejected_at_admission(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            self.write_ownership_marker(paths, "cutoverPreparing", 2)
            for command in sorted(backend.LEGACY_MUTATION_COMMANDS):
                args = mock.Mock(command=command)
                parser = mock.Mock()
                parser.parse_args.return_value = args
                with self.subTest(command=command), \
                     mock.patch.object(backend, "build_parser", return_value=parser), \
                     mock.patch.object(backend.Paths, "current", return_value=paths), \
                     mock.patch.object(backend, "migrate_legacy_data") as migrate:
                    with self.assertRaisesRegex(
                        backend.BackendError, "native runtime ownership blocks"
                    ):
                        backend.main()
                    migrate.assert_not_called()

    def test_run_core_dispatch_bypasses_user_mutation_lock_and_migration(self):
        args = mock.Mock(command="run-core", core="/usr/bin/mihomo")
        paths = mock.Mock()
        parser = mock.Mock()
        parser.parse_args.return_value = args
        with mock.patch.object(backend, "build_parser", return_value=parser), \
             mock.patch.object(backend.Paths, "current", return_value=paths), \
             mock.patch.object(backend, "ensure_private_dir") as ensure_private, \
             mock.patch.object(backend, "operation_lock") as operation_lock, \
             mock.patch.object(backend, "read_ownership_marker") as read_marker, \
             mock.patch.object(backend, "migrate_legacy_data") as migrate, \
             mock.patch.object(backend, "run_core_supervisor", return_value=7) as run_core:
            self.assertEqual(backend.main(), 7)
        ensure_private.assert_called_once_with(paths.config_dir)
        operation_lock.assert_not_called()
        read_marker.assert_not_called()
        migrate.assert_not_called()
        run_core.assert_called_once_with(paths, Path("/usr/bin/mihomo"))

    def test_run_core_process_starts_while_user_mutation_lock_is_held(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            env, runtime = self.make_env(home)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True)
            paths.config.write_text("mode: rule\n", encoding="utf-8")
            core = home / "mihomo"
            core.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            core.chmod(0o755)
            with backend.operation_lock(paths):
                child = subprocess.Popen(
                    [str(ROOT / "backend.sh"), "run-core", str(core)],
                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env,
                )
                try:
                    stdout, stderr = child.communicate(timeout=2)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.communicate()
                    self.fail("run-core waited for the user mutation lock")
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
            path.read_text(encoding="utf-8")
            for path in (
                ROOT / "backend.py", ROOT / "backend.sh", ROOT / "install.sh",
                ROOT / "uninstall.sh", PLUGIN / "Service.qml",
            )
        )
        for forbidden in ("NOPASSWD", "/etc/sudoers", "crontab", "pkexec", "sudo "):
            self.assertNotIn(forbidden, command_sources)
        # The one privileged setup step is user-mediated text: the panel may
        # copy it to the clipboard, but neither QML service nor backend can
        # execute it on the user's behalf.
        panel = (PLUGIN / "Panel.qml").read_text(encoding="utf-8")
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

    def test_file_picker_discovery_is_bounded_and_deterministic(self):
        available = {
            "zenity": "/usr/bin/zenity",
            "kdialog": "/usr/bin/kdialog",
            "yad": "/usr/bin/yad",
        }
        with mock.patch.object(backend.shutil, "which", side_effect=available.get):
            self.assertEqual(
                backend.discover_file_picker(), ("zenity", "/usr/bin/zenity")
            )
            self.assertEqual(
                backend.file_picker_status(),
                {"available": True, "provider": "zenity"},
            )
        available["zenity"] = None
        with mock.patch.object(backend.shutil, "which", side_effect=available.get):
            self.assertEqual(
                backend.discover_file_picker(), ("kdialog", "/usr/bin/kdialog")
            )

    def test_desktop_helper_status_exposes_only_readiness_booleans(self):
        available = {
            "zenity": "/private/bin/zenity",
            "qrencode": "/private/bin/qrencode",
        }
        with mock.patch.object(backend.shutil, "which", side_effect=available.get):
            payload = backend.desktop_helper_status()
        self.assertEqual(payload, {
            "configEditorAvailable": True,
            "qrEncoderAvailable": True,
        })
        self.assertNotIn("/private", json.dumps(payload))

        with mock.patch.object(backend.shutil, "which", return_value=None):
            self.assertEqual(backend.desktop_helper_status(), {
                "configEditorAvailable": False,
                "qrEncoderAvailable": False,
            })

    def test_missing_file_picker_has_actionable_public_error(self):
        with mock.patch.object(backend.shutil, "which", return_value=None), \
             mock.patch.object(backend, "gtk4_file_picker_available", return_value=False):
            self.assertEqual(
                backend.file_picker_status(), {"available": False, "provider": ""}
            )
            with self.assertRaises(backend.BackendError) as raised:
                backend.pick_import_file()
        self.assertEqual(raised.exception.exit_code, 2)
        self.assertIn("File import unavailable — file picker missing", str(raised.exception))
        self.assertIn("omarchy pkg add zenity", str(raised.exception))

    def test_standard_omarchy_gtk4_picker_is_the_final_safe_fallback(self):
        selected = "/tmp/profile;still-data.conf"
        with mock.patch.object(backend.shutil, "which", return_value=None), \
             mock.patch.object(backend, "gtk4_file_picker_available", return_value=True), \
             mock.patch.object(backend, "pick_import_file_gtk4", return_value=selected), \
             mock.patch.object(backend, "run") as run_mock, \
             mock.patch("sys.stdout", new_callable=io.StringIO) as output:
            self.assertEqual(backend.discover_file_picker(), ("gtk4", ""))
            self.assertEqual(
                backend.file_picker_status(),
                {"available": True, "provider": "gtk4"},
            )
            self.assertEqual(backend.pick_import_file(), 0)
            self.assertEqual(output.getvalue(), selected + "\n")
            run_mock.assert_not_called()

    def test_supported_file_pickers_use_safe_argv_and_never_execute_the_path(self):
        selected = "/tmp/profile;touch-never-executed.conf"
        expected = {
            "zenity": ["/usr/bin/zenity", "--file-selection"],
            "kdialog": ["/usr/bin/kdialog", "--getopenfilename"],
            "yad": ["/usr/bin/yad", "--file"],
        }
        for provider, prefix in expected.items():
            with self.subTest(provider=provider), \
                 mock.patch.object(
                     backend.shutil, "which",
                     side_effect=lambda name, chosen=provider: (
                         f"/usr/bin/{name}" if name == chosen else None
                     ),
                 ), \
                 mock.patch.object(
                     backend, "run",
                     return_value=subprocess.CompletedProcess([], 0, selected + "\n", ""),
                 ) as run_mock, \
                 mock.patch("sys.stdout", new_callable=io.StringIO) as output:
                self.assertEqual(backend.pick_import_file(), 0)
                command = run_mock.call_args.args[0]
                self.assertEqual(command[:2], prefix)
                self.assertNotIn("bash", command)
                self.assertEqual(output.getvalue(), selected + "\n")

    def test_configure_startup_validates_and_enables_helper_transactionally(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            store = backend.empty_store()
            store["profiles"] = [{
                "id": profile_id, "name": "Example", "uri": REALITY_URI,
                "protocol": "vless",
            }]
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
        store["profiles"] = [{
            "id": profile_id, "name": "Example", "uri": REALITY_URI,
            "protocol": "vless",
        }]
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
            store["profiles"] = [{
                "id": profile_id, "name": "Example", "uri": REALITY_URI,
                "protocol": "vless",
            }]
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
            store["profiles"] = [{
                "id": profile_id, "name": "Example", "uri": REALITY_URI,
                "protocol": "vless",
            }]
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
                 mock.patch.object(backend, "select_global_proxy") as select_global, \
                 mock.patch.object(backend, "mark_active"):
                backend.startup_connect(paths)
            render.assert_called_once_with(paths, mock.ANY, "global", mock.ANY)
            systemctl.assert_called_once_with("start", backend.SERVICE)
            select_global.assert_called_once_with(paths, "Example")
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
            store["profiles"] = [{
                "id": profile_id, "name": "Example", "uri": REALITY_URI,
                "protocol": "vless",
            }]
            backend.save_store(paths, store)
            ok = subprocess.CompletedProcess([], 0, "", "")
            with mock.patch.object(backend, "mark_intent"), \
                 mock.patch.object(backend, "systemctl", return_value=ok) as systemctl:
                backend.stop_service(paths, profile_id)
            systemctl.assert_called_once_with("stop", backend.SERVICE, check=False)
            self.assertEqual(backend.load_store(paths)["activeId"], "")

    def test_interactive_full_vpn_selects_the_active_profile_after_start(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            profile_id = "22222222-2222-4222-8222-222222222222"
            store = backend.empty_store()
            store["profiles"] = [{
                "id": profile_id, "name": "Example", "uri": REALITY_URI,
                "protocol": "vless",
            }]
            backend.save_store(paths, store)
            paths.template.write_text(
                "mode: global\nproxies:\n{{OMAVLESS_PROXY}}\nrules:\n  - MATCH,PROXY\n",
                encoding="utf-8",
            )
            candidate = paths.config_dir / ".candidate.yaml"
            candidate.write_text("mode: global\n", encoding="utf-8")
            ok = subprocess.CompletedProcess([], 0, "", "")
            with mock.patch.object(
                backend, "service_active", side_effect=[False, False, True]
            ), mock.patch.object(
                backend, "find_core", return_value=Path("/usr/bin/mihomo")
            ), mock.patch.object(backend, "ensure_unit"), mock.patch.object(
                backend, "render_config", return_value="mode: global\n"
            ), mock.patch.object(
                backend, "test_config", return_value=candidate
            ), mock.patch.object(
                backend, "systemctl", return_value=ok
            ) as systemctl, mock.patch.object(
                backend, "select_global_proxy"
            ) as select_global, mock.patch.object(backend, "mark_active"):
                backend.connect_profile(paths, profile_id)
            systemctl.assert_called_once_with("start", backend.SERVICE)
            select_global.assert_called_once_with(paths, "Example")
            self.assertEqual(backend.load_store(paths)["activeId"], profile_id)

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
        with self.assertRaisesRegex(backend.BackendError, "Invalid VLESS query"):
            backend.parse_vless(many_fields)
        with self.assertRaisesRegex(backend.BackendError, "not supported by Mihomo"):
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
                with self.assertRaisesRegex(backend.BackendError, "Edited profile input is too large"):
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
        self.assertEqual(
            backend.systemd_condition_path('/tmp/100%/a "é'),
            '/tmp/100%%/a\\x20\\x22\\xc3\\xa9',
        )
        with self.assertRaisesRegex(ValueError, "must be absolute"):
            backend.systemd_condition_path("relative/path")

    def test_systemd_unit_runs_the_plugin_supervisor_without_credentials(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.unit.parent.mkdir(parents=True)
            core = home / "mihomo"
            core.write_text("#!/bin/sh\n", encoding="utf-8")
            core.chmod(0o755)
            private = "credential-that-must-not-reach-the-unit"
            paths.config_dir.mkdir(parents=True)
            paths.config.write_text(f'password: "{private}"\n', encoding="utf-8")
            ok = subprocess.CompletedProcess([], 0, "", "")
            with mock.patch.object(backend, "systemctl", return_value=ok):
                backend.ensure_unit(paths, core)
            text = paths.unit.read_text(encoding="utf-8")
            condition = backend.systemd_condition_path(
                str(backend.PLUGIN_DIR / "manifest.json")
            )
            self.assertIn(f"ConditionPathExists={condition}", text)
            self.assertNotIn('ConditionPathExists="', text)
            self.assertIn(" run-core ", text)
            self.assertIn(str(backend.PLUGIN_DIR / "backend.sh"), text)
            self.assertIn(str(core), text)
            self.assertNotIn(f"ExecStart={backend.systemd_quote(str(core))}", text)
            self.assertNotIn(str(paths.config), text)
            self.assertNotIn(private, text)

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
            condition = backend.systemd_condition_path(
                str(backend.PLUGIN_DIR / "manifest.json")
            )
            self.assertIn(f"ConditionPathExists={condition}", text)
            self.assertNotIn('ConditionPathExists="', text)
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
                self.assertEqual(backend.notify_drop(paths, profile_id), 0)
                self.assertEqual(backend.notify_drop(paths, profile_id), 2)
                backend.mark_active(paths, profile_id, time.time_ns() // 1_000_000)
                self.assertEqual(backend.notify_drop(paths, profile_id), 0)

    def test_external_drop_notification_has_a_fixed_metadata_free_body(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            profile_id = "22222222-2222-4222-8222-222222222222"
            with mock.patch.object(
                backend.shutil, "which", return_value="/usr/bin/notify-send"
            ), mock.patch.object(backend, "run") as runner:
                self.assertEqual(backend.notify_drop(paths, profile_id), 0)
            command = runner.call_args.args[0]
            self.assertEqual(command[-1], "A VPN profile was deactivated")

    def test_argparse_boundary_preserves_profile_and_subscription_names_starting_with_dash(self):
        parser = backend.build_parser()
        profile_id = "22222222-2222-4222-8222-222222222222"
        cases = (
            (["rename", profile_id, "--", "--fast"], "--fast"),
            (["edit", profile_id, "--", "--fast"], "--fast"),
            (["import", "--", "--fast", "", ""], "--fast"),
            (["subscription-save", "--", "--provider", ""], "--provider"),
        )
        for arguments, expected_name in cases:
            with self.subTest(arguments=arguments):
                self.assertEqual(parser.parse_args(arguments).name, expected_name)

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
                "plugin/Panel.qml", "plugin/Service.qml", "plugin/NamePrompt.qml",
                "plugin/SubscriptionPrompt.qml", "plugin/RoutingPresetPrompt.qml",
                "plugin/OnboardingWizard.qml", "plugin/StartupPrompt.qml",
                "plugin/RoutingToolsPrompt.qml", "plugin/ImportPreviewPrompt.qml",
                "plugin/RenameWindow.qml", "plugin/QrWindow.qml",
                "backend.py", "backend.sh", "install.sh",
                "uninstall.sh",
                "manifest.json", "README.md", "CHANGELOG.md", "LICENSE", "THIRD_PARTY_NOTICES.md",
                "templates/default.yaml", "templates/china.yaml", "templates/iran.yaml",
            )
        ]
        distributed.extend(sorted((ROOT / "docs" / "user").glob("*.md")))
        texts = {path: path.read_text(encoding="utf-8") for path in distributed}
        credential = re.compile(
            r"(?:vless://[0-9a-fA-F-]{36}|trojan://[^\s/@]+|"
            r"(?:hysteria2|hy2)://[^\s/@]+|tuic://[^\s/@]+)@"
        )
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
            source = (PLUGIN / name).read_text(encoding="utf-8")
            self.assertIn("SPDX-License-Identifier: MIT", source)
            self.assertIn("Adapted from Omarchy VPN", source)
        self.assertIn("THIRD_PARTY_NOTICES.md", (ROOT / "install.sh").read_text(encoding="utf-8"))

    def test_current_mihomo_accepts_generated_profile_configs(self):
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
            "vless-ipv6-ws": (
                "vless://11111111-1111-4111-8111-111111111111@[2001:db8::1]:443"
                "?type=ws&security=tls&sni=ipv6.example.org"
                "&host=cdn.example.org&path=%2Fedge#IPv6"
            ),
            "reality-pq-flag": base + "&supportX25519MLKEM768=true#" + fragment,
            "vless-encryption": REALITY_URI.replace(
                "encryption=none",
                "encryption=mlkem768x25519plus.native.1rtt."
                "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8",
            ),
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
            "trojan-tls": TROJAN_URI,
            "trojan-reality-grpc": TROJAN_REALITY_URI,
            "hysteria2-gecko-port-hopping": HYSTERIA2_URI,
            "tuic-v5": TUIC_URI,
        }
        for name, uri in profiles.items():
            with self.subTest(profile=name), tempfile.TemporaryDirectory() as temp:
                config = template.replace(
                    backend.PROFILE_MARKER,
                    backend.profile_yaml({
                        "name": name,
                        "uri": uri,
                        "protocol": backend.profile_protocol(uri),
                    }),
                )
                config_path = Path(temp) / "config.yaml"
                config_path.write_text(config, encoding="utf-8")
                result = subprocess.run(
                    [str(core), "-t", "-d", temp, "-f", str(config_path)],
                    text=True, capture_output=True,
                )
                self.assertEqual(result.returncode, 0, result.stderr or result.stdout)

    def test_current_mihomo_accepts_every_bundled_global_selector(self):
        configured = os.environ.get("OMAVLESS_TEST_MIHOMO", "").strip()
        if not configured:
            self.skipTest("set OMAVLESS_TEST_MIHOMO for the opt-in core integration test")
        core = Path(configured)
        rendered_profile = backend.profile_yaml({
            "name": "Example", "uri": REALITY_URI, "protocol": "vless",
        })
        for template_name in ("default.yaml", "china.yaml", "iran.yaml"):
            with self.subTest(template=template_name), tempfile.TemporaryDirectory() as temp:
                template = (ROOT / "templates" / template_name).read_text(encoding="utf-8")
                config = template.replace(backend.PROFILE_MARKER, rendered_profile)
                config_path = Path(temp) / "config.yaml"
                config_path.write_text(config, encoding="utf-8")
                result = subprocess.run(
                    [str(core), "-t", "-d", temp, "-f", str(config_path)],
                    text=True, capture_output=True,
                )
                self.assertEqual(result.returncode, 0, result.stderr or result.stdout)

    def test_core_validation_error_never_forwards_credential_bearing_output(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(mode=0o700, parents=True)
            private = "private-password-and-key"
            with mock.patch.object(backend, "run", return_value=subprocess.CompletedProcess(
                args=["mihomo", "-t"], returncode=1,
                stdout="", stderr=f"invalid credential {private}",
            )):
                with self.assertRaises(backend.BackendError) as caught:
                    backend.test_config(
                        paths, Path("/usr/bin/mihomo"),
                        f'proxies:\n  - name: test\n    password: "{private}"\n',
                    )
            self.assertEqual(
                str(caught.exception), "Mihomo rejected the generated configuration"
            )
            self.assertNotIn(private, str(caught.exception))
            self.assertEqual(list(paths.config_dir.glob(".candidate.*.yaml")), [])

    def test_plugin_enabled_state_is_strict_and_bounded(self):
        command = "/usr/bin/omarchy"
        enabled = subprocess.CompletedProcess(
            [], 0, json.dumps([{"id": "kdk.omavless", "enabled": True}]), ""
        )
        disabled = subprocess.CompletedProcess(
            [], 0, json.dumps([{"id": "kdk.omavless", "enabled": False}]), ""
        )
        with mock.patch.object(backend.shutil, "which", return_value=command), \
             mock.patch.object(backend, "run", side_effect=[enabled, disabled]) as run:
            self.assertIs(backend.plugin_enabled_state(), True)
            self.assertIs(backend.plugin_enabled_state(), False)
        self.assertEqual(
            run.call_args_list[0],
            mock.call(
                [command, "plugin", "list", "--json"], check=False,
                timeout=backend.PLUGIN_LIST_TIMEOUT_SECONDS,
            ),
        )

        invalid_results = (
            subprocess.CompletedProcess([], 1, "", "unavailable"),
            subprocess.CompletedProcess([], 0, "not-json", ""),
            subprocess.CompletedProcess([], 0, json.dumps({"enabled": False}), ""),
            subprocess.CompletedProcess(
                [], 0, json.dumps([{"id": "kdk.omavless", "enabled": "false"}]), ""
            ),
            subprocess.CompletedProcess([], 0, "[" * 2000 + "]" * 2000, ""),
            subprocess.CompletedProcess([], 0, "x" * (backend.MAX_PLUGIN_LIST_BYTES + 1), ""),
        )
        for result in invalid_results:
            with self.subTest(stdout=result.stdout[:16], returncode=result.returncode), \
                 mock.patch.object(backend.shutil, "which", return_value=command), \
                 mock.patch.object(backend, "run", return_value=result):
                self.assertIsNone(backend.plugin_enabled_state())
        with mock.patch.object(backend.shutil, "which", return_value=None):
            self.assertIsNone(backend.plugin_enabled_state())

    def test_removal_guard_ignores_hot_reload_and_unavailable_shell(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            for state in (True, None):
                with self.subTest(enabled=state), \
                     mock.patch.dict(os.environ, {"OMAVLESS_REMOVAL_GRACE_SECONDS": "0"}), \
                     mock.patch.object(backend, "plugin_manifest_present", return_value=True), \
                     mock.patch.object(backend, "plugin_enabled_state", return_value=state), \
                     mock.patch.object(backend, "disable_runtime_integration") as cleanup:
                    self.assertEqual(backend.watch_plugin_removal(paths), 0)
                    cleanup.assert_not_called()

    def test_removal_guard_dispatch_does_not_create_private_data(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            with mock.patch.object(sys, "argv", ["backend.sh", "watch-plugin-removal"]), \
                 mock.patch.object(backend.Paths, "current", return_value=paths), \
                 mock.patch.object(backend, "watch_plugin_removal", return_value=0) as guard, \
                 mock.patch.object(backend, "ensure_private_dir") as ensure:
                self.assertEqual(backend.main(), 0)
            guard.assert_called_once_with(paths)
            ensure.assert_not_called()
            self.assertFalse(paths.config_dir.exists())

    def test_removal_guard_cleans_after_disable_or_checkout_deletion(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            cases = ((True, False), (False, None))
            for manifest_present, enabled in cases:
                with self.subTest(manifest=manifest_present, enabled=enabled), \
                     mock.patch.dict(os.environ, {"OMAVLESS_REMOVAL_GRACE_SECONDS": "0"}), \
                     mock.patch.object(
                         backend, "plugin_manifest_present", return_value=manifest_present
                     ), \
                     mock.patch.object(
                         backend, "plugin_enabled_state", return_value=enabled
                     ) as state, \
                     mock.patch.object(
                         backend, "disable_runtime_integration", return_value=True
                     ) as cleanup:
                    self.assertEqual(backend.watch_plugin_removal(paths), 0)
                    cleanup.assert_called_once_with(paths, stop_main=True)
                    if manifest_present:
                        state.assert_called_once_with()
                    else:
                        state.assert_not_called()

    def test_removal_guard_cannot_stop_native_owned_runtime(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            self.write_ownership_marker(paths, "rust", 4)
            with mock.patch.dict(
                os.environ, {"OMAVLESS_REMOVAL_GRACE_SECONDS": "0"}
            ), mock.patch.object(
                backend, "plugin_manifest_present", return_value=False
            ), mock.patch.object(
                backend, "disable_runtime_integration"
            ) as cleanup:
                with self.assertRaisesRegex(
                    backend.BackendError, "native runtime ownership blocks"
                ):
                    backend.watch_plugin_removal(paths)
            cleanup.assert_not_called()

    def test_disabled_plugin_cleanup_runs_end_to_end_without_deleting_private_data(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.unit.parent.mkdir(parents=True)
            paths.config_dir.mkdir(parents=True, mode=0o700)
            helper = backend.startup_unit(paths)
            paths.unit.write_text("main unit", encoding="utf-8")
            helper.write_text("startup unit", encoding="utf-8")
            private = "private-profile-that-must-remain"
            paths.store.write_text(private, encoding="utf-8")
            paths.store.chmod(0o600)

            fake_bin = home / "bin"
            fake_bin.mkdir()
            omarchy = fake_bin / "omarchy"
            omarchy.write_text(
                '#!/bin/sh\nprintf \'%s\\n\' \'[{"id":"kdk.omavless","enabled":false}]\'\n',
                encoding="utf-8",
            )
            omarchy.chmod(0o755)
            systemctl = fake_bin / "systemctl"
            systemctl.write_text(
                '#!/bin/sh\ncase "$*" in *"is-active"*|*"is-enabled"*) exit 3;; *) exit 0;; esac\n',
                encoding="utf-8",
            )
            systemctl.chmod(0o755)
            env = os.environ.copy()
            env.update({
                "OMAVLESS_HOME": str(home),
                "XDG_RUNTIME_DIR": str(runtime),
                "OMAVLESS_SYSTEMCTL": str(systemctl),
                "OMAVLESS_REMOVAL_GRACE_SECONDS": "0",
                "PATH": str(fake_bin) + os.pathsep + env["PATH"],
            })
            result = subprocess.run(
                [str(ROOT / "backend.sh"), "watch-plugin-removal"],
                env=env, text=True, capture_output=True, timeout=10,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(paths.unit.exists())
            self.assertFalse(helper.exists())
            self.assertEqual(paths.store.read_text(encoding="utf-8"), private)
            self.assertNotIn(private, result.stdout + result.stderr)

    def test_runtime_cleanup_removes_units_only_after_the_core_stops(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.unit.parent.mkdir(parents=True)
            helper = backend.startup_unit(paths)
            paths.unit.write_text("main", encoding="utf-8")
            helper.write_text("startup", encoding="utf-8")
            ok = subprocess.CompletedProcess([], 0, "", "")
            with mock.patch.object(backend, "systemctl", return_value=ok) as systemctl, \
                 mock.patch.object(backend, "service_active", return_value=False), \
                 mock.patch.object(backend, "service_enabled", return_value=False):
                self.assertTrue(backend.disable_runtime_integration(paths, stop_main=True))
            self.assertFalse(paths.unit.exists())
            self.assertFalse(helper.exists())
            self.assertEqual(systemctl.call_args_list, [
                mock.call("disable", "--now", backend.STARTUP_SERVICE, check=False),
                mock.call("disable", "--now", backend.SERVICE, check=False),
                mock.call("daemon-reload", check=False),
            ])

            paths.unit.write_text("main", encoding="utf-8")
            helper.write_text("startup", encoding="utf-8")
            with mock.patch.object(backend, "systemctl", return_value=ok), \
                 mock.patch.object(backend, "service_active", return_value=True), \
                 mock.patch.object(backend, "SERVICE_STOP_GRACE_SECONDS", 0):
                self.assertFalse(backend.disable_runtime_integration(paths, stop_main=True))
            self.assertTrue(paths.unit.exists())
            self.assertTrue(helper.exists())

            with mock.patch.object(backend, "systemctl", return_value=ok), \
                 mock.patch.object(backend, "service_active", return_value=False), \
                 mock.patch.object(backend, "service_enabled", return_value=True):
                self.assertFalse(backend.disable_runtime_integration(paths, stop_main=True))
            self.assertTrue(paths.unit.exists())
            self.assertTrue(helper.exists())

    def test_core_supervisor_stops_and_unlinks_after_plugin_removal(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True)
            private = "private-profile-credential"
            paths.config.write_text(f'password: "{private}"\n', encoding="utf-8")
            core = home / "mihomo"
            core.write_text("#!/bin/sh\n", encoding="utf-8")
            core.chmod(0o755)
            process = mock.Mock()
            process.poll.side_effect = [None, None]
            process.returncode = None
            with mock.patch.object(backend.subprocess, "Popen", return_value=process) as popen, \
                 mock.patch.object(backend, "plugin_manifest_present", return_value=False), \
                 mock.patch.object(backend, "PLUGIN_REMOVAL_GRACE_SECONDS", 0), \
                 mock.patch.object(backend.time, "sleep"), \
                 mock.patch.object(backend, "stop_probe_core") as stop, \
                 mock.patch.object(
                     backend, "disable_runtime_integration", return_value=True
                 ) as cleanup:
                self.assertEqual(backend.run_core_supervisor(paths, core), 0)
            argv = popen.call_args.args[0]
            self.assertEqual(argv, [
                str(core.resolve()), "-d", str(paths.config_dir), "-f", str(paths.config),
            ])
            self.assertNotIn(private, " ".join(argv))
            self.assertEqual(popen.call_args.kwargs, {"stdin": subprocess.DEVNULL})
            stop.assert_called_once_with(process)
            cleanup.assert_called_once_with(paths, stop_main=False)

    def test_core_supervisor_preserves_child_failure_for_systemd_restart(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp)
            runtime = home / "runtime"
            runtime.mkdir(mode=0o700)
            paths = self.paths_for(home, runtime)
            paths.config_dir.mkdir(parents=True)
            paths.config.write_text("mode: rule\n", encoding="utf-8")
            core = home / "mihomo"
            core.write_text("#!/bin/sh\n", encoding="utf-8")
            core.chmod(0o755)
            process = mock.Mock()
            process.poll.side_effect = [None, 7]
            process.returncode = 7
            with mock.patch.object(backend.subprocess, "Popen", return_value=process), \
                 mock.patch.object(backend, "plugin_manifest_present", return_value=True), \
                 mock.patch.object(backend.time, "sleep"), \
                 mock.patch.object(backend, "disable_runtime_integration") as cleanup:
                self.assertEqual(backend.run_core_supervisor(paths, core), 7)
            cleanup.assert_not_called()
            self.assertEqual(backend.core_exit_code(-15), 143)

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
