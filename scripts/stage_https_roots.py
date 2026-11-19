"""Validate and stage an approved public HTTPS CA bundle as a separate file."""

import argparse
import hashlib
import os
from pathlib import Path
import re
import ssl
import tempfile

from sign_cmddb import sign_payload_rsa

SIGNATURE_DOMAIN = b"PurpleOS/HTTPS-ROOTS/v1\0"


def stage_file(destination, data):
    with tempfile.NamedTemporaryFile(dir=destination.parent, delete=False) as output:
        pending = Path(output.name)
        try:
            output.write(data)
            output.close()
            os.replace(pending, destination)
        finally:
            output.close()
            pending.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path, help="Approved PEM certificate bundle")
    parser.add_argument("destination", type=Path, help="Output file in the staging directory")
    parser.add_argument("--sha256", required=True, help="Expected digest from the release manifest")
    parser.add_argument("--key", type=Path, help="Creator RSA private key for the guest signature")
    args = parser.parse_args()

    if not re.fullmatch(r"[0-9a-fA-F]{64}", args.sha256):
        parser.error("--sha256 must contain 64 hexadecimal digits")
    try:
        data = args.bundle.read_bytes()
        if hashlib.sha256(data).hexdigest() != args.sha256.lower():
            parser.error("bundle digest does not match the release manifest")

        certificates = re.findall(
            rb"-----BEGIN CERTIFICATE-----\s+[A-Za-z0-9+/=\s]+?-----END CERTIFICATE-----",
            data,
        )
        if not certificates or data.count(b"-----BEGIN ") != len(certificates) or (
            data.count(b"-----END ") != len(certificates)
        ):
            parser.error("bundle must contain complete PEM certificate blocks only")
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        context.load_verify_locations(cadata=b"\n".join(certificates).decode("ascii"))
        if context.cert_store_stats()["x509_ca"] != len(certificates):
            parser.error("bundle contains duplicate entries or certificates that are not CAs")

        signature = None
        if args.key is not None:
            signature = sign_payload_rsa(SIGNATURE_DOMAIN + data, str(args.key))
        stage_file(args.destination, data)
        if signature is not None:
            stage_file(args.destination.with_name(args.destination.name + ".sig"), signature.encode("ascii"))
    except (OSError, RuntimeError, UnicodeError) as error:
        parser.error(str(error))

    print(f"Staged {len(certificates)} CA certificates in {args.destination}")


if __name__ == "__main__":
    main()
