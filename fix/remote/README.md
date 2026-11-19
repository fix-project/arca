# Remote runtimes

## SSH clone and distributed evaluation

On **both machines**, build Fix and bootstrap from the same seed. Each machine
needs an Ed25519 key loaded in its own `ssh-agent`:

```sh
bazel build //fix/cli:fix //bootstrap:seed
fix="$PWD/bazel-bin/fix/cli/fix"
seed="$PWD/$(bazel cquery //bootstrap:seed --output=files)"
"$fix" trust add-agent
"$fix" bootstrap "$seed"
```

Exchange the Fix signing public keys. On the **server**, trust the client's key;
on the **client**, trust the server's key:

```sh
"$fix" trust add-key /path/to/peer-signing-key.pub
```

The client also needs SSH login access to the server. The server's
noninteractive SSH session must find `fix` on its command path and have access
to its signing `ssh-agent`. Fix trust is separate from SSH login authorization.
Check the command path with `ssh user@server 'command -v fix'`.

On the **server**, create a repository and give it a label:

```sh
mkdir -p ~/fix-repo
cd ~/fix-repo
"$fix" init
"$fix" -l example '42u64'
```

On the **client**, build the distributed scheduler, clone the server repository,
then label the compiled scheduler. Run the build command from the Arca checkout:

```sh
bazel build //programs/fix/schedulers:distributed_module
module="$PWD/bazel-bin/programs/fix/schedulers/distributed.wasm"
mkdir -p ~/work
cd ~/work
"$fix" clone user@server:fix-repo project
cd project
"$fix" remote labels origin
"$fix" pull origin example
"$fix" -l distributed "@compile(\"$module\")"
"$fix" --scheduler distributed eval 1u64
```

The clone stores `origin`'s label names and canonical values, then `pull`
creates a shallow local label by default; its backing remains remote.
`distributed` delegates the Eval request to the first reachable remote, here `origin`; a failed delegation is
an error. The remote starts `fix serve --stdio` under its own bootstrapped
programs to accept a proposed transfer of the selected scheduler and
collector. It then re-executes `serve` with those programs on the same SSH connection,
subject to the remote machine's trust settings.
A requested collector must be trusted as a tagger on the worker; use
`fix trust add-tagger HANDLE` there if needed. With no execution remotes
of its own, Distributed runs the delegated request locally. The local parser
runs before delegation. To use this policy by default in the cloned repository,
run `"$fix" config scheduler distributed`.
Run `"$fix" pull origin` when the server's labels change, then inspect them
with `"$fix" remote labels origin`.

## Remote configuration

`fix init` adds a `bootstrap` file remote pointing to the user bootstrap store.
Run `fix pull bootstrap` to fetch its label refs. File remotes provide storage,
not evaluation workers. Repository remotes live in `.fix/remotes`. `file:///absolute/path/to/.fix`
uses a local repository. Network addresses can be `tcp://host:port`,
`ssh://user@host/absolute/repository/path`, `user@host:repository/path`
(relative to the remote login directory), or
`user@host:/absolute/repository/path`. SSH starts `fix -C PATH serve --stdio`
on demand; TCP requires a running server. SSH authentication and host-key
checks use the user's SSH configuration. Fix separately verifies Tag and
coupon certificates against its trust settings.

```sh
fix remote add worker user@worker.example:/srv/fix/repository
fix remote set-scheduler worker scheduler-label
fix remote set-collector worker collector-label
fix remote add other tcp://worker.example:9000
fix remote set-url worker tcp://other.example:9000
fix remote get-url worker
fix remote rename worker backup
fix remote remove backup
fix remote list
```

Repeat `--remote NAME=URL` to add remotes for one command. A command-line remote overrides the repository remote with the same name, without saving the override.

For SSH remotes, the active scheduler and collector are transferred and selected
on demand. `set-scheduler` and `set-collector` instead select programs already
available in the remote repository; a command can override either with
`--remote-scheduler REMOTE=FIX_NAME` or `--remote-collector REMOTE=FIX_NAME`.
TCP connections use the scheduler and collector already running on their
server. Compiler and tagger trust always comes from the machine accepting the
work.

## Remote refs

`fix clone URL DIR` initializes `.fix` in DIR, configures `origin`, and copies
its label names. Existing files outside `.fix` are untouched. Object names are
stored as Ref shadow labels such as `origin:program`; Thunk and Encode names
retain their kinds. These are ordinary labels: `@label("origin:program")`
resolves locally without fetching backing data. Evaluation and loading belong
to the selected scheduler. Local defaults come from the user's bootstrap.

Both commands use `REMOTE [SOURCE [DESTINATION]]`:

```sh
fix pull origin
fix pull origin program local-program --strict
fix push origin local-program program --exact
```

Omitting SOURCE selects all labels. Bare `fix pull` selects every configured
remote. Pull's default destination is `REMOTE:SOURCE`; push's is SOURCE.
Selected destinations are updated. Push supports SSH and file remotes; TCP
servers reject label updates.

`pull` batches the selected labels into one scheduler evaluation. Label snapshots
also use proposal/accept; repeating a pull reuses unchanged snapshots and memoized
results. `-j0` with Hint, OnePass, or Distributed runs applications and collector
calls remotely while keeping the scheduler local. Add `--summarize` for the
per-machine resource table. Coupon transfers about labeled inputs or results
include their claim, for example `Load Eval(origin:program, result-name)`.
Storage supplies friendly names using labels, Fix notation (such as `*'name`),
and unambiguous canonical prefixes with their hexadecimal type byte (`abcd1234.b5`).
Label transformations use sigils: `&name`, `*'name`, `#name`, and `[value index]`.
When no faithful sigil form is available, the canonical prefix retains the type byte.
Coupon descriptions use only resident metadata, without loading referenced data.

Both commands accept mutually exclusive `--shallow`, `--strict`, and `--exact`:

| Mode | Result |
| --- | --- |
| shallow (default) | Preserve a Ref, lower an Object to its Ref, or evaluate a shallow Encode for a Thunk. |
| strict | Evaluate a strict Encode and store its resulting Object. |
| exact | Load the original value's backing without evaluating it; preserve its name and kind. |

Strict encoding of Objects and Refs uses identification Thunks. Shallow
encoding preserves their Ref descriptors without loading backing; strict
evaluation loads it. Existing Encodes use the requested encoding mode. Push also loads and transfers backing and
certificates so the destination can obtain the contents without connecting back
to the sender. Pull can obtain data from its configured source remote through
the scheduler. Tag trust is checked when data is imported, not when a remote
label's name is read.

`fix config --global pull.mode strict` sets the user default;
`fix config pull.mode exact` overrides it in this repository. `push.mode` works
the same way. Command-line mode flags take precedence over both settings.

A home repository is optional and can be configured as an ordinary file remote.

## User trust and identities

Trust and default programs live in `$XDG_CONFIG_HOME/fix` (default `~/.config/fix`):

- `authorized_keys`: OpenSSH Ed25519 public-key lines, one per line.
- `trusted_compilers`: canonical Fix compiler author handles, one per line.
- `trusted_taggers`: canonical callable identities trusted for local privileged tag construction, one per line.
- `config`: TOML containing signing identities and program selections edited by `fix trust` and `fix config --global`.

`FIX_CONFIG_DIR` selects another user configuration directory. `XDG_DATA_HOME` selects the bootstrap store's parent directory. Project repository archives never install user configuration.

```sh
fix trust add-agent
fix trust add-key ~/.ssh/id_ed25519.pub
fix trust add-identity ~/.ssh/id_ed25519.pub
fix trust add-key /path/to/worker.pub
fix trust add-compiler COMPILER_HANDLE
fix trust add-tagger COLLECTOR_HANDLE
fix trust list
```

`add-agent` adds every Ed25519 key loaded in ssh-agent to both signing identities and trusted keys. `remove-key`, `remove-identity`, `remove-compiler`, and `remove-tagger` remove entries. Compiler and tagger operands can also name repository labels, such as `trusted-compiler`, or synced remote refs, such as `origin:trusted-compiler`. Remote refs resolve without downloading backing data; run `fix pull origin` to refresh them. Selecting a signing identity does not automatically authorize its signatures; add its public key to `authorized_keys` too. Repeat `--identity KEY` to replace the configured signing identities for one command. KEY can be a public-key file or a complete OpenSSH public-key line.

Signing uses the Linux host's `SSH_AUTH_SOCK`. Load the chosen keys with `ssh-add`; private keys stay in ssh-agent and are never passed to the guest. Newly persisted Tags and explicit `fix sign` operations use every selected identity. Network transfers select a certificate after the receiver advertises its trusted keys. `fix export` adds the selected identities to archived Tag certificates without changing the source Tags. Existing signatures are preserved. Exports containing no Tags, or reusing an existing certificate, need no signing agent. Using a stored Tag requires a valid signature by at least one authorized key. Reading signed data requires no agent.

Each authorized key is trusted as a complete remote runtime. It can attest to Tags by any author, including kernel coupons. Tags retain their original Fix authors and canonical names. Compiler trust separately controls which Runnable authors may execute. Callable-tagger trust controls privileged tag construction inside the local runtime, including the coupon collector. This list is stored alongside SSH-key trust but is not advertised as network certificate authority.

## Stored certificates and transfer

Tag and Tag Ref object records point to their backing data like other objects. `.fix/tags/signatures/TAG/SIGNER` indexes certificate Blob names separately for each signing key. Each certificate contains one signature; updating one key's certificate does not overwrite another's. Certificates are ordinary content-addressed Blobs, stored and transferred like other Blobs. Their signatures bind the canonical Tag name in the `fix-tag-v1` SSHSIG namespace. Tag backing data remains separate under `.fix/data`. Reloading checks certificate trust, backing hashes, and certificates of nested Tags. Revoking a key causes subsequent reloads of certificates signed only by that key to fail.

`fixremote` exports sets of canonical root names, object records, optional backing data, and optional TODO metadata. A prepared Tag transfer retains its available certificates separately from the backing data. Import verifies object hashes, certificate signatures, and required dependencies before publishing objects. A scheduler can send a thin Ref with arbitrary TODO metadata and transfer backing later; Fix neither interprets that metadata nor fetches data automatically.

`fixhandle::minrepo::visit` visits required Objects in post-order using the normal
call stack. Its reader can iterate borrowed children without heap allocation. Refs, Thunks, and Encodes are opaque;
callers explicitly include coupon claims or execution definitions when needed.
Tag fields follow the same backing rules as Tree fields.
The repository adapter uses this traversal for transfer closure. Complete
computation transfers opt into deferred definitions explicitly; attached coupons
still use their minimum repositories.

The scheduler SDK exposes `remote::propose`, `remote::propose_full`, `remote::import`, `remote::missing`, `remote::certify`, `remote::trusted_keys`, and `remote::configured`. The Linux SDK provides `Client::export`, `Client::import`, `Client::missing`, `Client::certify`, and `Client::remote_config`. Local descriptors and continuations are never wire identities. Data transfer and trust checks do not depend on a networking protocol.

## Example wire protocol

When updating the protocol, rebuild both `fix` and the bootstrap seed, then
rerun `fix bootstrap SEED` on each machine. Explicitly configured scheduler
programs must also use the matching protocol version.

The `fixprotocol` crate follows the original Fix protocol's message structure. Serde/Postcard messages carry a protocol version, with an eight-byte big-endian frame length supplied by `fixtransport`. The codecs work over `embedded_io`, independently of TCP or a particular scheduler. The receiver checks the version before decoding the message body. Decode failures retain the codec error and frame size; server diagnostics identify the serving protocol version.

- `RequestTransfer` requests data by Fix name; the sender replies through the normal proposal/accept exchange. Plain transfers carry no evaluation task. Backing is read only after the receiver selects its missing objects.
- `Run` identifies a task by coupon operation, collector, and input. `Result` identifies that task and its returned coupon.
- `RequestProgress` obtains current runtime measurements independently of evaluation requests, so connected unused nodes remain visible.
- `EvaluationProgress` reports availability, live task operations and locations, completed steps, measured counts, CPU, memory, and transport counters. Each evaluation connection tracks its completion events until delivered; nested remotes retain their routes. Runtime and completion-event identities prevent aliases and returning progress reports from duplicating measurements. The receiving CLI chooses the presentation.
- `RequestInfo` and `Info` exchange parallelism, available memory, link speed, and inventory hints.
- Scheduler connections exchange `Hello` and `HelloAccepted`, then carry `Multiplex` frames. Both endpoints can open independent request channels. Even channel numbers belong to the initiator; odd numbers belong to the receiver. Each channel permits one unconsumed chunk of at most 64 KiB; consuming it returns a credit. Closing a channel leaves other requests and the physical connection active.
- Topology advertisements carry a runtime identity, an owner revision, capacity, inventory, and named links. A link is usable when both endpoints advertise it. Multiple links between the same runtimes remain distinct, while worker capacity is counted once per runtime. Nodes exchange advertisements over existing connections and delegate requests to immediate neighbors. A receiving scheduler chooses placement again.
- Blob, Tree, and shallow Tree requests and data messages support separate object transfer. Load messages express prefetch hints.
- `ProposeTransfer` lists object names, backing availability, and TODO metadata, without certificate names. `AcceptTransfer` selects the missing records and, when Tags are missing, advertises the receiver's trusted SSH keys. Data messages supply those records before `Run` or `Result` completes the exchange.

For a requested Tag, the sender chooses a certificate signed by a key the receiver trusts and sends its Blob alongside the Tag backing. If no suitable certificate exists and the receiver trusts a configured signing identity, the sender signs the local Tag on demand through ssh-agent and retains that certificate for reuse. This also works for unsigned local Tags; exporting them endorses their contents. Existing Tags need no certificate transfer. A certificate must verify even when its Blob hash and the Tag's backing hash are correct. Only Tags and coupons are signed; task, inventory, and transfer-control messages are untrusted scheduler communication. A returned coupon must match the expected author, operation, and input before a scheduler accepts its result.

The shared `send_transfer` and `receive_transfer` helpers implement sequential proposal/accept exchanges over a persistent stream. The TCP example uses these helpers, buffers each transfer, and limits each frame and the received backing bytes to 512 MiB. The lower-level message codecs let schedulers implement other exchange policies, object requests, and prefetching. Schedulers own transport authentication, confidentiality, retries, resource limits, and acquisition policy.

## Unsigned seeds

Bootstrap and precompilation artifacts use an explicit unsigned staging mode. Archive import stores unsigned or untrusted Tags, but reading and evaluating them still requires a trusted signature. After configuring and authorizing your signing identities, you can sign imported Tags explicitly:

```sh
fix trust add-agent
fix bootstrap /path/to/seed.tar
fix init
```

`fix sign TAG` accepts a Tag name or repository label. `fix sign --all` signs every stored Tag. Both add signatures from all selected identities while preserving existing signatures and rejecting backing data that does not match its name. Signing vouches for the Tag's contents; select only Tags you intend to endorse.

`fix import --sign ARCHIVE` imports and signs only the Tags in that archive. `fix bootstrap SEED` also trusts the seed compiler and compiled collector in user configuration.

Build tools use `--unsigned` or `FIX_UNSIGNED=true` for staging repositories. Their unsigned output still requires explicit trust and signing when imported into a normal repository. Staging mode does not authorize unsigned Tags received from the network.

## Sharing a lazy compilation

The source can label a compilation Thunk without running it:

```sh
fix init
fix pull bootstrap
fix -l addblob "#(\"\" *'@label(\"bootstrap:compile\") @file(\"addblob.wasm\"))"
```

After cloning on the destination:

```sh
fix clone user@server:/path/to/source project
cd project
fix '*#(536870912u64 *@label("origin:addblob") 20u64 22u64)'
```

The function position forces the compilation Thunk before applying its result.
The scheduler may compile locally, delegate compilation, or delegate the whole
request. Evaluation rows identify their execution location; remote task titles,
counts, and runtime statistics appear in the same display. Pull instead
emphasizes the backing bytes transferred. Clone and init both configure the
local bootstrap store as a storage remote.
