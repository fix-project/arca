# Remote runtimes

Repository remotes live in `.fix/remotes`. Addresses are opaque to Fix; the example scheduler uses `tcp://host:port` addresses.

```sh
fix remote add worker tcp://worker.example:9000
fix remote set-url worker tcp://other.example:9000
fix remote get-url worker
fix remote rename worker backup
fix remote remove backup
fix remote list
```

Repeat `--remote NAME=URL` to add remotes for one command. A command-line remote overrides the repository remote with the same name, without saving the override.

## User trust and identities

Trust lives under `~/.fix/config`, independently of repositories:

- `authorized_keys`: OpenSSH Ed25519 public-key lines, one per line.
- `trusted_compilers`: canonical Fix compiler author handles, one per line.
- `trusted_taggers`: canonical callable identities trusted for local privileged tag construction, one per line.
- `config`: JSON containing an `identities` array of OpenSSH public-key lines.

`FIX_CONFIG_DIR` selects another user configuration directory, including for isolated tests and build tools. Repository archives never install user configuration.

```sh
fix trust add-key ~/.ssh/id_ed25519.pub
fix trust add-identity ~/.ssh/id_ed25519.pub
fix trust add-key /path/to/worker.pub
fix trust add-compiler COMPILER_HANDLE
fix trust add-tagger COLLECTOR_HANDLE
fix trust list
```

`remove-key`, `remove-identity`, `remove-compiler`, and `remove-tagger` remove entries. Compiler and tagger operands can also name repository labels, such as `trusted-compiler`. Selecting a signing identity does not automatically authorize its signatures; add its public key to `authorized_keys` too. Repeat `--identity KEY` to replace the configured signing identities for one command. KEY can be a public-key file or a complete OpenSSH public-key line.

Signing uses the Linux host's `SSH_AUTH_SOCK`. Load the chosen keys with `ssh-add`; private keys stay in ssh-agent and are never passed to the guest. Newly persisted Tags and explicit `fix sign` operations use every selected identity. Network transfers select a certificate after the receiver advertises its trusted keys. `fix export` adds the selected identities to archived Tag certificates without changing the source Tags. Existing signatures are preserved. Exports containing no Tags, or reusing an existing certificate, need no signing agent. Using a stored Tag requires a valid signature by at least one authorized key. Reading signed data requires no agent.

Each authorized key is trusted as a complete remote runtime. It can attest to Tags by any author, including kernel coupons. Tags retain their original Fix authors and canonical names. Compiler trust separately controls which Runnable authors may execute. Callable-tagger trust controls privileged tag construction inside the local runtime, including the coupon collector. This list is stored alongside SSH-key trust but is not advertised as network certificate authority.

## Stored certificates and transfer

Tag and Tag Ref object records point to their backing data like other objects. `.fix/certificates/TAG/SIGNER` indexes certificate Blob names separately for each signing key. Each certificate contains one signature; updating one key's certificate does not overwrite another's. Certificates are ordinary content-addressed Blobs, stored and transferred like other Blobs. Their signatures bind the canonical Tag name in the `fix-tag-v1` SSHSIG namespace. Tag backing data remains separate under `.fix/data`. Reloading checks certificate trust, backing hashes, and certificates of nested Tags. Revoking a key causes subsequent reloads of certificates signed only by that key to fail.

`fixremote` exports sets of canonical root names, object records, optional backing data, and optional TODO metadata. A prepared Tag transfer retains its available certificates separately from the backing data. Import verifies object hashes, certificate signatures, and required dependencies before publishing objects. A scheduler can send a thin Ref with arbitrary TODO metadata and transfer backing later; Fix neither interprets that metadata nor fetches data automatically.

The scheduler SDK exposes `remote::export`, `remote::import`, `remote::missing`, `remote::certify`, `remote::trusted_keys`, and `remote::configured`. The Linux SDK provides `Client::export`, `Client::import`, `Client::missing`, `Client::certify`, and `Client::remote_config`. Local descriptors and continuations are never wire identities. Data transfer and trust checks do not depend on a networking protocol.

## Example wire protocol

The `fixprotocol` crate follows the original Fix protocol's message structure. Serde/Postcard messages carry a protocol version, with an eight-byte big-endian frame length supplied by `fixtransport`. The codecs work over `embedded_io`, independently of TCP or a particular scheduler.

- `Run` identifies a task by coupon operation, collector, and input. `Result` identifies that task and its returned coupon.
- `RequestInfo` and `Info` exchange parallelism, link speed, and inventory hints.
- Blob, Tree, and shallow Tree requests and data messages support separate object transfer. Load messages express prefetch hints.
- `ProposeTransfer` lists object names, backing availability, and TODO metadata, without certificate names. `AcceptTransfer` selects the missing records and, when Tags are missing, advertises the receiver's trusted SSH keys. Data messages supply those records before `Run` or `Result` completes the exchange.

For a requested Tag, the sender chooses a certificate signed by a key the receiver trusts and sends its Blob alongside the Tag backing. If no suitable certificate exists and the receiver trusts a configured signing identity, the sender signs the local Tag on demand through ssh-agent and retains that certificate for reuse. This also works for unsigned local Tags; exporting them endorses their contents. Existing Tags need no certificate transfer. A certificate must verify even when its Blob hash and the Tag's backing hash are correct. Only Tags and coupons are signed; task, inventory, and transfer-control messages are untrusted scheduler communication. A returned coupon must match the expected author, operation, and input before a scheduler accepts its result.

The shared `send_transfer` and `receive_transfer` helpers implement sequential proposal/accept exchanges over a persistent stream. The TCP example uses these helpers, buffers each transfer, and limits each frame and the received backing bytes to 512 MiB. The lower-level message codecs let schedulers implement other exchange policies, object requests, and prefetching. Schedulers own transport authentication, confidentiality, retries, resource limits, and acquisition policy.

## Unsigned seeds

Bootstrap and precompilation artifacts use an explicit unsigned staging mode. Archive import stores unsigned or untrusted Tags, but reading and evaluating them still requires a trusted signature. After configuring and authorizing your signing identities, you can sign imported Tags explicitly:

```sh
fix init /path/to/seed.tar --sign
fix trust add-compiler trusted-compiler
```

`fix sign TAG` accepts a Tag name or repository label. `fix sign --all` signs every stored Tag. Both add signatures from all selected identities while preserving existing signatures and rejecting backing data that does not match its name. Signing vouches for the Tag's contents; select only Tags you intend to endorse.

`fix import --sign ARCHIVE` imports and signs only the Tags in that archive. `fix init /path/to/seed.tar --sign` does the same when initializing a repository. Neither command adds a compiler to user trust automatically.

Build tools use `--unsigned` or `FIX_UNSIGNED=true` for staging repositories. Their unsigned output still requires explicit trust and signing when imported into a normal repository. Staging mode does not authorize unsigned Tags received from the network.
