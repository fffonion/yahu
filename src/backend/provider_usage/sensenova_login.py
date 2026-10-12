"""SenseNova's official password -> OAuth authorization-code exchange.

Invoked privately by Yahu. Tokens on stdout are consumed by the parent,
never exposed as a public API result or diagnostic output.
"""

import base64
import hashlib
import json
import os
import secrets
import sys
from urllib.parse import parse_qs, urlencode, urljoin, urlparse

import requests


class LoginFailure(Exception):
    pass


def b64url(value):
    return base64.urlsafe_b64encode(value).rstrip(b"=").decode("ascii")


def origin(url):
    parsed = urlparse(url)
    if parsed.username or parsed.password or not parsed.hostname:
        raise LoginFailure("invalid authentication origin")
    if parsed.scheme != "https" and not (
        parsed.scheme == "http" and parsed.hostname in {"127.0.0.1", "localhost", "::1"}
    ):
        raise LoginFailure("authentication requires HTTPS")
    return parsed.scheme, parsed.netloc


def json_body(response):
    try:
        if not 200 <= response.status_code < 300:
            raise LoginFailure("authentication request rejected")
        raw = response.raw.read(65537, decode_content=True)
        if len(raw) > 65536:
            raise LoginFailure("authentication response too large")
        value = json.loads(raw)
        if not isinstance(value, dict):
            raise LoginFailure("invalid authentication response")
        return value
    finally:
        response.close()


def encrypted_password(keys, password):
    key = next((key for key in keys if key.get("kid") == "public:hydra.openid.id-token"), None)
    if key is None:
        if not keys:
            return password, False
        raise LoginFailure("password encryption key unavailable")
    from cryptography.hazmat.primitives import hashes
    from cryptography.hazmat.primitives.asymmetric import padding, rsa
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM

    def number(value):
        return int.from_bytes(base64.urlsafe_b64decode(value + "=" * (-len(value) % 4)), "big")

    public_key = rsa.RSAPublicNumbers(number(key["e"]), number(key["n"])).public_key()
    protected = b64url(b'{"alg":"RSA-OAEP","enc":"A256GCM"}')
    content_key, iv = secrets.token_bytes(32), secrets.token_bytes(12)
    wrapped = public_key.encrypt(content_key, padding.OAEP(
        mgf=padding.MGF1(hashes.SHA1()), algorithm=hashes.SHA1(), label=None
    ))
    encrypted = AESGCM(content_key).encrypt(iv, password.encode("utf-8"), protected.encode("ascii"))
    return ".".join([protected, b64url(wrapped), b64url(iv), b64url(encrypted[:-16]), b64url(encrypted[-16:])]), True


def login(username, password, oauth_base, iam_base, jwks_url):
    oauth_base = oauth_base.rstrip("/")
    allowed = {origin(oauth_base), origin(jwks_url), origin(iam_base)}
    verifier, state = secrets.token_urlsafe(48), secrets.token_urlsafe(24)
    # SHA-256 is the protocol-required PKCE challenge, not a file fingerprint.
    challenge = b64url(hashlib.sha256(verifier.encode("ascii")).digest())
    authorization = oauth_base + "/oauth2/auth?" + urlencode({
        "client_id": "nova", "response_type": "code", "redirect_uri": oauth_base,
        "scope": "openid offline offline_access", "code_challenge": challenge,
        "code_challenge_method": "S256", "state": state,
    })
    with requests.Session() as session:
        session.headers.update({"Accept": "application/json", "Origin": oauth_base,
                                "Referer": oauth_base + "/login"})

        def request(method, url, **kwargs):
            return session.request(method, url, timeout=15, allow_redirects=False, stream=True, **kwargs)

        def walk(url, looking_for):
            for _ in range(8):
                if origin(url) not in allowed:
                    raise LoginFailure("unexpected authentication redirect origin")
                query = parse_qs(urlparse(url).query)
                if looking_for in query:
                    if looking_for == "code" and (
                        origin(url) != origin(oauth_base)
                        or urlparse(url).path not in ("", "/")
                        or query.get("state") != [state]
                    ):
                        raise LoginFailure("OAuth callback state mismatch")
                    value = query[looking_for][0]
                    if not value:
                        raise LoginFailure("empty authorization value")
                    return value
                response = request("GET", url)
                location = response.headers.get("Location")
                valid_redirect = response.status_code in (301, 302, 303, 307, 308)
                response.close()
                if not valid_redirect or not location:
                    raise LoginFailure("authentication interaction required")
                url = urljoin(url, location)
            raise LoginFailure("authentication redirect limit reached")

        login_challenge = walk(authorization, "login_challenge")
        keys = json_body(request("GET", jwks_url)).get("keys")
        if not isinstance(keys, list):
            raise LoginFailure("invalid password encryption keys")
        protected_password, is_encrypt = encrypted_password(keys, password)
        result = json_body(request("POST", iam_base.rstrip("/") + "/v1/auth/nova/login", json={
            "username": username, "password": protected_password,
            "challenge": login_challenge, "is_encrypt": is_encrypt,
        }))
        result = result.get("data", result)
        if not isinstance(result, dict):
            raise LoginFailure("invalid login result")
        redirect = next((result.get(key) for key in ("redirect_uri", "redirect_to", "redirect")
                         if isinstance(result.get(key), str) and result[key]), None)
        if not redirect:
            raise LoginFailure("login did not return an authorization redirect")
        code = walk(urljoin(oauth_base, redirect), "code")
        tokens = json_body(request("POST", oauth_base + "/oauth2/token", data={
            "grant_type": "authorization_code", "client_id": "nova", "code": code,
            "redirect_uri": oauth_base, "code_verifier": verifier, "state": state,
        }))
        for key in ("access_token", "refresh_token"):
            value = tokens.get(key)
            if not isinstance(value, str) or not value or value.strip() != value or "\n" in value or "\r" in value:
                raise LoginFailure("login returned incomplete credentials")
        return {key: tokens[key] for key in ("access_token", "refresh_token", "expires_in") if key in tokens}


if __name__ == "__main__":
    try:
        endpoints = json.loads(os.environ["SENSENOVA_AUTH_ENDPOINTS"])
        tokens = login(os.environ["SENSENOVA_USERNAME"], os.environ["SENSENOVA_PASSWORD"], **endpoints)
        print(json.dumps(tokens))
    except Exception as error:
        # LoginFailure messages are fixed literals; all other exceptions remain opaque.
        reason = str(error) if isinstance(error, LoginFailure) else type(error).__name__
        print(json.dumps({"error": "SenseNova 密码登录失败或需要交互验证", "safe_reason": reason}))
        sys.exit(1)
