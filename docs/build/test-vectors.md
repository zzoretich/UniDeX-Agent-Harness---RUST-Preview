# Test vectors and the scanners

Two scanners guard this public repository: the privacy gate (`cargo xtask privacy`, run by the commit hooks, the
pre-push hook, the `privacy` job and the `privacy (base)` job) and the pinned secret scanner (gitleaks, in CI). The gate
also carries its own secret rules (`xtask/src/privacy/secrets.rs`). A test that needs a value one of them would flag
must not weaken them. This page states the convention every task follows, as the M1 plan's Global Constraints give it,
with one worked example for each kind. Every example on this page is itself checked by both scanners, so each one is
written the way the convention asks.

## The convention

Neither the privacy gate nor the secret scanner is ever loosened for a test.

- No key-shaped value is written whole in any committed file, tests and fixtures included. A test builds it at run
  time from parts, each too short to match any secret rule (a prefix constant split in two, a body generated at run
  time). Fixtures and golden files use low-entropy invented values (`sig-0000`, `msg_0001`) that match no rule.
- Addresses: the documentation ranges (192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24, 2001:db8::/32) and loopback,
  which the privacy rules never flag. A private, shared, link-local or metadata-service address is written only as a
  numeric constructor (`Ipv4Addr::new(..)`) or an octet array, its dotted spelling built at run time.
- Home folders: only the placeholder names the privacy rules allow (`/home/user`, `/Users/example`, `C:\Users\user`);
  any other user name is built at run time.
- Hosts: `example.com`, `example.org`, `example.net`, `.test`, `.example`, `.invalid`. A `.local` or `.lan` name, a
  hardware address or a `user@host` prompt is built at run time. Emails use the example domains.
- The privacy gate's own tests (`xtask/tests/`) keep their path-scoped allow entries, because they test the rules.
- The allow files (`privacy-allow.toml`, `.gitleaks.toml`) are frozen after T2a. A task whose vector cannot follow the
  convention stops and reports.

## Worked examples

### A key-shaped value

Split the provider prefix in two and generate the body, so no literal in the file is long enough to match a rule. The
same holds for a private key block: its header is assembled too.

```rust
/// A key in the shape of an API key, made at run time.
fn key_vector() -> String {
    let body: String = (0..48).map(|i| char::from(b'a' + (i * 7 % 26) as u8)).collect();
    format!("{}{}{}", "sk-", "ant-", body)
}

/// A private key block's header, made at run time.
fn key_block_header() -> String {
    format!("{}{}", "-----BEGIN ", "RSA PRIVATE KEY-----")
}
```

### A fixture or golden value

A value that only has to look like an identifier is low in entropy and matches no rule:

```json
{ "message_id": "msg_0001", "signature": "sig-0000", "request": "req-0002" }
```

The rules have no entropy test, so "low-entropy" alone is not enough: a plain value that follows a provider prefix or
the word `Bearer` is a finding once it is long enough. Keep such a value shorter than the rule's body minimum (20
characters after `Bearer`, 32 after an `sk-` prefix, 36 after a hosting-token prefix; a cloud key prefix needs exactly
16 more capitals and digits, so use fewer), or make the whole body one repeated character, which the rules treat as a
placeholder. The bearer rule also reads prose. The word "bearer" (any letter case) is a finding when it is followed by
spaces, tabs or `%20` with at most one line break (or one written escape such as `\n`) among them, and then by 20 or
more letters, digits, `-`, `.`, `_`, `~`, `+` or `/`. So a sentence that ends a line with that word and starts the next
line with a file path is a finding; a blank line between them is not. Do not write a path straight after that word.

### An address

Documentation ranges and loopback are written as they are. A private, shared, link-local or metadata-service address is
a constructor or an octet array, and its dotted spelling is made at run time:

```rust
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

let public_example: IpAddr = "198.51.100.7".parse().unwrap();
let v6_example: IpAddr = "2001:db8::1".parse().unwrap();
let loopback = Ipv4Addr::LOCALHOST;
let private = Ipv4Addr::new(10, 0, 0, 1);
let metadata = [169u8, 254, 169, 254];
let dotted = private.to_string(); // built at run time, never written out
let v6_private = Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1);
```

### A home folder

Use a placeholder name the rules allow (`/home/user/project`, `/Users/example/notes`, `C:\Users\user\AppData`). Any
other name is assembled:

```rust
let invented = format!("/{}/{}/notes.md", "home", "wrenfield");
```

### A host, an email and a prompt

Example domains and reserved suffixes are written as they are (`build-01.example.com`, `printer.test`,
`dev@example.com`). A local network name, a hardware address and a shell prompt are assembled:

```rust
let local_name = format!("{}.{}", "printer", "local");
let hardware = [0x02u8, 0x00, 0x5e, 0x10, 0x00, 0x01]
    .map(|b| format!("{b:02x}"))
    .join(":");
let prompt = format!("{}@{}:~$ ", "dev", "buildbox");
```

### The gate's own tests

A test of the gate's rules may need the very text a rule flags. It keeps a path-scoped entry in `privacy-allow.toml`,
limited to `xtask/tests/**`, one rule and the exact matched strings, with a reason:

```toml
[[allow]]
path = "xtask/tests/**"
rule = "Email"
values = ["<each exact matched string>"]
reason = "invented test vectors"
```

The secret rules can never be exempted this way: a test of them builds every key-shaped vector at run time, as
`xtask/tests/common/mod.rs` does.

### What the scanners are not

Both scanners guard against accidents and against a change that loosens the rules. Neither is a data-loss control: a
value split across strings or lines, an encoded value (the gate does not decode base64), or one put where neither
scans is not found. The `privacy (base)` job is the one check a branch cannot steer.

### When a vector cannot follow the convention

After T2a the allow files do not change. A task whose vector cannot be written this way stops and reports to the
controller instead of adding an exemption.
